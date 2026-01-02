//! 索引模块
//!
//! 将文件解析并索引到知识图谱中
//!
//! ## 功能
//!
//! - 单文件索引
//! - 全量索引
//! - 增量更新
//! - 变更检测

use crate::core::parser::parse_markdown;
use crate::core::watch_config::WatchConfig;
use crate::storage::{GraphStorage, Node, NodeType, Storage};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// 索引器
///
/// 负责将文件系统中的 Markdown 文件解析并索引到知识图谱
pub struct Indexer {
    /// 存储后端
    storage: Storage,
    /// 文件监听配置
    config: WatchConfig,
}

impl Indexer {
    /// 创建新的索引器
    ///
    /// # Arguments
    ///
    /// * `storage` - 存储后端
    ///
    /// # Returns
    ///
    /// 索引器实例
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            config: WatchConfig::new(),
        }
    }

    /// 创建带配置的索引器
    ///
    /// # Arguments
    ///
    /// * `storage` - 存储后端
    /// * `config` - 监听配置
    ///
    /// # Returns
    ///
    /// 索引器实例
    pub fn with_config(storage: Storage, config: WatchConfig) -> Self {
        Self { storage, config }
    }

    /// 更新配置
    ///
    /// # Arguments
    ///
    /// * `config` - 新的配置
    pub fn set_config(&mut self, config: WatchConfig) {
        self.config = config;
    }

    /// 获取当前配置
    ///
    /// # Returns
    ///
    /// 当前配置引用
    pub fn config(&self) -> &WatchConfig {
        &self.config
    }

    /// 索引单个文件
    ///
    /// # Arguments
    ///
    /// * `path` - 文件路径
    ///
    /// # Returns
    ///
    /// 索引结果
    pub fn index_file(&self, path: &Path) -> anyhow::Result<IndexResult> {
        // 检查文件是否被允许
        if !self.config.is_allowed(path) {
            return Ok(IndexResult::Skipped);
        }

        // 检查是否为 Markdown 文件
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            return Ok(IndexResult::Skipped);
        }

        // 读取文件内容
        let content = fs::read_to_string(path)?;
        let relative_path = self.get_relative_path(path)?;

        // 解析文档
        let doc = parse_markdown(&content, &relative_path);

        // 创建节点
        let node = doc.to_node();
        self.storage.graph().add_node(&node)?;

        // 创建边
        for link in &doc.wiki_links {
            let target_id = self.get_target_id(&link.target);

            // 确保目标节点存在
            if !self.storage.graph().node_exists(&target_id)? {
                let target_node = Node::new(&target_id, NodeType::Concept, &link.target);
                self.storage.graph().add_node(&target_node)?;
            }

            let edge = link.to_edge(&node.id);
            self.storage.graph().add_edge(&edge)?;
        }

        Ok(IndexResult::Indexed {
            path: relative_path,
            node_count: 1,
            edge_count: doc.wiki_links.len(),
        })
    }

    /// 索引单个文件（异步）
    ///
    /// # Arguments
    ///
    /// * `path` - 文件路径
    ///
    /// # Returns
    ///
    /// 索引结果
    pub async fn index_file_async(&self, path: &Path) -> anyhow::Result<IndexResult> {
        self.index_file(path)
    }

    /// 全量索引目录
    ///
    /// 遍历目录中的所有文件并索引
    ///
    /// # Arguments
    ///
    /// * `root` - 根目录路径
    ///
    /// # Returns
    ///
    /// 索引汇总结果
    pub fn index_all(&self, root: &Path) -> anyhow::Result<IndexSummary> {
        let mut summary = IndexSummary::default();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            match self.index_file(path) {
                Ok(result) => summary.add(result),
                Err(e) => {
                    summary.errors.push((path.to_string_lossy().into_owned(), e.to_string()));
                }
            }
        }

        Ok(summary)
    }

    /// 重新索引（删除后重建）
    ///
    /// 先删除图谱中对应目录的所有节点和边，然后重新索引
    ///
    /// # Arguments
    ///
    /// * `root` - 根目录路径
    ///
    /// # Returns
    ///
    /// 索引汇总结果
    pub fn reindex_all(&self, root: &Path) -> anyhow::Result<IndexSummary> {
        // 获取所有现有节点
        let existing_nodes = self.storage.graph().list_nodes()?;

        // 删除属于该目录的节点
        let root_prefix = format!("urn:memexia:file:");
        for node in existing_nodes {
            if node.id.starts_with(&root_prefix) {
                self.storage.graph().delete_node(&node.id)?;
            }
        }

        // 重新索引
        self.index_all(root)
    }

    /// 处理文件变更事件
    ///
    /// # Arguments
    ///
    /// * `event` - 文件事件
    ///
    /// # Returns
    ///
    /// 处理结果
    pub fn handle_event(&self, event: &super::watcher::FileEvent) -> anyhow::Result<IndexResult> {
        match event {
            super::watcher::FileEvent::Created(path) => {
                let path = Path::new(path);
                self.index_file(path)
            }
            super::watcher::FileEvent::Modified(path) => {
                let path = Path::new(path);
                self.index_file(path)
            }
            super::watcher::FileEvent::Deleted(path) => {
                // 删除节点
                let node_id = self.path_to_id_string(path);
                if self.storage.graph().node_exists(&node_id)? {
                    self.storage.graph().delete_node(&node_id)?;
                }
                Ok(IndexResult::Deleted(node_id))
            }
            super::watcher::FileEvent::Renamed(from, to) => {
                // 先删除旧节点，再索引新文件
                let from_id = self.path_to_id_string(from);
                if self.storage.graph().node_exists(&from_id)? {
                    self.storage.graph().delete_node(&from_id)?;
                }
                let path = Path::new(to);
                self.index_file(path)
            }
        }
    }

    /// 获取相对路径
    fn get_relative_path(&self, path: &Path) -> anyhow::Result<String> {
        // 获取相对于仓库根目录的路径
        let root = self.storage.root();

        if let Ok(rel_path) = path.strip_prefix(root) {
            Ok(rel_path.to_string_lossy().replace('\\', "/"))
        } else {
            // 如果不在根目录下，使用完整路径
            Ok(path.to_string_lossy().replace('\\', "/"))
        }
    }

    /// 将文件路径转换为节点 ID（带 URL 编码）
    fn path_to_id(&self, path: &Path) -> String {
        let rel_path = self.get_relative_path(path).unwrap_or_else(|_| {
            path.to_string_lossy().replace('\\', "/")
        });
        let encoded = encode_iri_component(&rel_path);
        format!("urn:memexia:file:{}", encoded)
    }

    /// 将字符串路径转换为节点 ID（带 URL 编码）
    fn path_to_id_string(&self, path_str: &str) -> String {
        let encoded = encode_iri_component(path_str);
        format!("urn:memexia:file:{}", encoded)
    }

    /// 获取目标节点 ID
    fn get_target_id(&self, target: &str) -> String {
        // 如果 target 已经是 URN 格式，直接使用
        if target.starts_with("urn:memexia:") {
            return target.to_string();
        }

        // 否则作为文件名处理（需要编码）
        let encoded = encode_iri_component(target);
        format!("urn:memexia:file:{}", encoded)
    }
}

/// 对 IRI 路径组件进行 percent 编码
fn encode_iri_component(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        // 只对 ASCII 字母数字和安全的符号不编码
        if c.is_ascii_alphanumeric() || "-_.~!$&'()*+,;=:@/".contains(c) {
            result.push(c);
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            for byte in encoded.as_bytes() {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}

/// 索引结果
#[derive(Debug, Clone, PartialEq)]
pub enum IndexResult {
    /// 文件被成功索引
    Indexed {
        /// 文件相对路径
        path: String,
        /// 创建的节点数
        node_count: usize,
        /// 创建的边数
        edge_count: usize,
    },
    /// 文件被跳过（不符合条件）
    Skipped,
    /// 文件被删除
    Deleted(String),
}

/// 索引汇总
#[derive(Debug, Default)]
pub struct IndexSummary {
    /// 索引的文件数
    pub files_indexed: usize,
    /// 创建的节点总数
    pub nodes_created: usize,
    /// 创建的边总数
    pub edges_created: usize,
    /// 跳过的文件数
    pub files_skipped: usize,
    /// 删除的文件数
    pub files_deleted: usize,
    /// 错误列表
    pub errors: Vec<(String, String)>,
}

impl IndexSummary {
    /// 添加索引结果
    pub fn add(&mut self, result: IndexResult) {
        match result {
            IndexResult::Indexed { path: _, node_count, edge_count } => {
                self.files_indexed += 1;
                self.nodes_created += node_count;
                self.edges_created += edge_count;
            }
            IndexResult::Skipped => {
                self.files_skipped += 1;
            }
            IndexResult::Deleted(_) => {
                self.files_deleted += 1;
            }
        }
    }

    /// 检查是否有错误
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_indexer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();
        let indexer = Indexer::new(storage);

        assert_eq!(indexer.config().whitelist.len(), 1);
    }

    #[test]
    fn test_index_file() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();
        let indexer = Indexer::new(storage.clone());

        // 创建测试文件
        let test_file = temp_dir.path().join("test.md");
        let content = r#"---
title: 测试文档
type: Concept
tags: [test]
---

# 测试

这是一个测试文档 [[目标|Contradicts]]。

#tag1"#;
        std::fs::write(&test_file, content).unwrap();

        // 索引文件
        let result = indexer.index_file(&test_file).unwrap();

        match result {
            IndexResult::Indexed { path, node_count, edge_count } => {
                assert!(path.contains("test.md"));
                assert_eq!(node_count, 1);
                assert_eq!(edge_count, 1);
            }
            _ => panic!("Expected Indexed result"),
        }

        // 验证节点创建
        let nodes = storage.graph().list_nodes().unwrap();
        assert_eq!(nodes.len(), 2); // 测试文档 + 目标节点

        // 验证边创建
        let edges = storage.graph().list_edges().unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_index_file_blacklisted() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        let mut config = WatchConfig::new();
        config.clear_whitelist();
        config.add_whitelist("*.txt"); // 只允许 txt 文件

        let indexer = Indexer::with_config(storage, config);

        // 创建测试文件
        let test_file = temp_dir.path().join("test.md");
        std::fs::write(&test_file, "# 测试").unwrap();

        // 索引文件（应该被跳过）
        let result = indexer.index_file(&test_file).unwrap();
        assert_eq!(result, IndexResult::Skipped);
    }

    #[test]
    fn test_index_all() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();
        let indexer = Indexer::new(storage);

        // 创建多个测试文件
        for i in 1..=3 {
            let test_file = temp_dir.path().join(format!("test{}.md", i));
            std::fs::write(&test_file, format!("# 测试 {}", i)).unwrap();
        }

        // 全量索引
        let summary = indexer.index_all(temp_dir.path()).unwrap();

        assert_eq!(summary.files_indexed, 3);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn test_handle_event_created() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();
        let indexer = Indexer::new(storage.clone());

        // 创建测试文件
        let test_file = temp_dir.path().join("event_test.md");
        std::fs::write(&test_file, "# 事件测试").unwrap();

        // 处理创建事件
        let event = crate::core::FileEvent::Created(test_file.to_string_lossy().into_owned());
        let result = indexer.handle_event(&event).unwrap();

        assert!(matches!(result, IndexResult::Indexed { .. }));
    }

    #[test]
    fn test_index_summary() {
        let mut summary = IndexSummary::default();

        summary.add(IndexResult::Indexed {
            path: "a.md".to_string(),
            node_count: 1,
            edge_count: 2,
        });
        summary.add(IndexResult::Indexed {
            path: "b.md".to_string(),
            node_count: 1,
            edge_count: 0,
        });
        summary.add(IndexResult::Skipped);

        assert_eq!(summary.files_indexed, 2);
        assert_eq!(summary.nodes_created, 2);
        assert_eq!(summary.edges_created, 2);
        assert_eq!(summary.files_skipped, 1);
    }
}
