//! 存储层模块
//!
//! 提供 Memexia 知识图谱的持久化存储功能
//!
//! ## 模块结构
//!
//! - [`node`](node::Node) - 节点数据模型
//! - [`edge`](edge::Edge) - 边（关系）数据模型
//! - [`graph`](graph::GraphStorage) - 图存储抽象 trait
//! - [`oxigraph`](oxigraph::OxigraphStorage) - Oxigraph 图数据库实现
//! - [`nquads`](nquads) - N-Quads 序列化/反序列化

pub mod node;
pub mod edge;
pub mod graph;
pub mod oxigraph;
pub mod nquads;

pub use node::{Node, NodeType};
pub use edge::{Edge, EdgeFilter, EdgeSource, RelationType};
pub use graph::{GraphStorage, GraphStats, QueryResult, EdgeDirection};
pub use oxigraph::OxigraphStorage;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Memexia 仓库元数据
///
/// 存储在 `.memexia/meta.json` 文件中
#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryMeta {
    /// 当前版本
    pub version: String,
    /// 创建时间 (RFC3339 格式)
    pub created_at: String,
    /// 最后更新时间 (RFC3339 格式)
    pub updated_at: String,
    /// 仓库名称
    pub name: String,
}

impl Default for RepositoryMeta {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: "0.1.0".to_string(),
            created_at: now.clone(),
            updated_at: now,
            name: "Untitled Repository".to_string(),
        }
    }
}

/// Memexia 存储管理器
///
/// 提供存储层的统一入口，封装图存储和文件操作
#[derive(Debug, Clone)]
pub struct Storage {
    /// 仓库根路径
    root: PathBuf,
    /// Oxigraph 图数据库
    graph_storage: OxigraphStorage,
}

impl Storage {
    /// 打开已有仓库
    ///
    /// # Arguments
    ///
    /// * `root` - 仓库根目录路径
    ///
    /// # Returns
    ///
    /// `Storage` 实例
    pub fn open(root: &Path) -> Result<Self> {
        let memexia_path = root.join(".memexia");
        if !memexia_path.exists() {
            anyhow::bail!("Not a valid Memexia repository: missing .memexia");
        }

        let meta_path = memexia_path.join("meta.json");
        if !meta_path.exists() {
            anyhow::bail!("Not a valid Memexia repository: missing .memexia/meta.json");
        }

        let graph_path = memexia_path.join("graph");
        if !graph_path.exists() {
            anyhow::bail!("Not a valid Memexia repository: missing .memexia/graph");
        }

        let graph_storage = OxigraphStorage::open(&graph_path)
            .with_context(|| format!("Failed to open graph store at {:?}", graph_path))?;

        Ok(Self {
            root: root.to_path_buf(),
            graph_storage,
        })
    }

    /// 初始化新仓库
    ///
    /// # Arguments
    ///
    /// * `root` - 仓库根目录路径
    ///
    /// # Returns
    ///
    /// `Storage` 实例
    pub fn init(root: &Path) -> Result<Self> {
        let memexia_dir = root.join(".memexia");

        // 检查是否已存在
        if memexia_dir.exists() {
            anyhow::bail!("Memexia repository already exists at {:?}", root);
        }

        // 创建目录结构
        std::fs::create_dir_all(&memexia_dir)?;
        std::fs::create_dir_all(memexia_dir.join("objects"))?;
        std::fs::create_dir_all(memexia_dir.join("config"))?;
        std::fs::create_dir_all(memexia_dir.join("index"))?;
        std::fs::create_dir_all(memexia_dir.join("graph"))?;

        // 创建 notes 目录
        std::fs::create_dir_all(root.join("notes"))?;

        // 创建 meta.json 元数据文件
        let meta = RepositoryMeta::default();
        let meta_path = memexia_dir.join("meta.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("Failed to write {:?}", meta_path))?;

        // 创建空的 N-Quads 图文件 (符合验收标准)
        let nq_path = memexia_dir.join("graph.nq");
        std::fs::write(&nq_path, "").with_context(|| format!("Failed to create {:?}", nq_path))?;

        // 初始化 Oxigraph 存储
        let graph_storage = OxigraphStorage::create(&memexia_dir.join("graph"))
            .with_context(|| format!("Failed to create OxigraphStorage"))?;

        Ok(Self {
            root: root.to_path_buf(),
            graph_storage,
        })
    }

    /// 获取图存储抽象
    ///
    /// # Returns
    ///
    /// 实现了 `GraphStorage` trait 的引用
    pub fn graph(&self) -> &dyn GraphStorage {
        &self.graph_storage
    }

    /// 获取可变的图存储抽象
    ///
    /// # Returns
    ///
    /// 实现了 `GraphStorage` trait 的可变引用
    pub fn graph_mut(&mut self) -> &mut dyn GraphStorage {
        &mut self.graph_storage
    }

    /// 获取仓库根路径
    ///
    /// # Returns
    ///
    /// 根路径引用
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 获取仓库元数据
    ///
    /// # Returns
    ///
    /// 元数据或 None（如果文件不存在）
    pub fn get_meta(&self) -> Result<Option<RepositoryMeta>> {
        let meta_path = self.root.join(".memexia/meta.json");
        if !meta_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&meta_path)
            .with_context(|| format!("Failed to read {:?}", meta_path))?;

        let meta: RepositoryMeta = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", meta_path))?;

        Ok(Some(meta))
    }

    /// 更新仓库元数据
    ///
    /// # Arguments
    ///
    /// * `meta` - 新的元数据
    ///
    /// # Returns
    ///
    /// 操作结果
    pub fn update_meta(&self, meta: &RepositoryMeta) -> Result<()> {
        let meta_path = self.root.join(".memexia/meta.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(meta)?)
            .with_context(|| format!("Failed to write {:?}", meta_path))?;

        Ok(())
    }

    /// 导出图为 N-Quads 格式
    ///
    /// # Arguments
    ///
    /// * `path` - 输出文件路径
    ///
    /// # Returns
    ///
    /// 操作结果
    pub fn export_nquads(&self, path: &Path) -> Result<()> {
        nquads::export_nquads(&self.graph_storage, path)?;
        Ok(())
    }

    /// 从 N-Quads 格式导入
    ///
    /// # Arguments
    ///
    /// * `path` - 输入文件路径
    ///
    /// # Returns
    ///
    /// 操作结果
    pub fn import_nquads(&self, path: &Path) -> Result<()> {
        nquads::import_nquads(&self.graph_storage, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_init() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        assert!(temp_dir.path().join(".memexia").exists());
        assert!(temp_dir.path().join(".memexia/graph").exists());
        assert!(temp_dir.path().join(".memexia/meta.json").exists());
        assert!(temp_dir.path().join(".memexia/graph.nq").exists());
        assert!(temp_dir.path().join("notes").exists());

        let meta = storage.get_meta().unwrap().unwrap();
        assert_eq!(meta.version, "0.1.0");
        assert_eq!(meta.name, "Untitled Repository");
    }

    #[test]
    fn test_storage_open() {
        let temp_dir = TempDir::new().unwrap();

        // 初始化
        let _ = Storage::init(temp_dir.path()).unwrap();

        // 重新打开
        let storage = Storage::open(temp_dir.path()).unwrap();
        assert_eq!(storage.root(), temp_dir.path());
    }

    #[test]
    fn test_storage_open_invalid() {
        let temp_dir = TempDir::new().unwrap();

        // 尝试打开不存在的仓库
        let result = Storage::open(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_init_already_exists() {
        let temp_dir = TempDir::new().unwrap();

        // 初始化
        let _ = Storage::init(temp_dir.path()).unwrap();

        // 再次初始化应该失败
        let result = Storage::init(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_storage_node_crud() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        // 添加节点
        let node = Node::new("urn:memexia:file:test.md", NodeType::Concept, "Test");
        storage.graph().add_node(&node).unwrap();

        // 检查存在
        assert!(storage.graph().node_exists("urn:memexia:file:test.md").unwrap());

        // 获取节点
        let retrieved = storage.graph().get_node("urn:memexia:file:test.md").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test");
    }

    #[test]
    fn test_storage_edge_crud() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        // 添加节点
        let node1 = Node::new("urn:memexia:file:a.md", NodeType::Concept, "A");
        let node2 = Node::new("urn:memexia:file:b.md", NodeType::Question, "B");
        storage.graph().add_node(&node1).unwrap();
        storage.graph().add_node(&node2).unwrap();

        // 添加边
        let edge = Edge::new(
            "edge-1",
            "urn:memexia:file:a.md",
            "urn:memexia:file:b.md",
            RelationType::Contradicts,
        );
        storage.graph().add_edge(&edge).unwrap();

        // 获取边
        let edges = storage
            .graph()
            .get_edges_for_node("urn:memexia:file:a.md", EdgeDirection::Outgoing)
            .unwrap();

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, RelationType::Contradicts);
    }

    #[test]
    fn test_storage_stats() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Storage::init(temp_dir.path()).unwrap();

        // 添加测试数据
        let node1 = Node::new("urn:memexia:file:a.md", NodeType::Concept, "A");
        let node2 = Node::new("urn:memexia:file:b.md", NodeType::Question, "B");
        storage.graph().add_node(&node1).unwrap();
        storage.graph().add_node(&node2).unwrap();

        let edge = Edge::new(
            "edge-1",
            "urn:memexia:file:a.md",
            "urn:memexia:file:b.md",
            RelationType::Supports,
        );
        storage.graph().add_edge(&edge).unwrap();

        // 获取统计
        let stats = storage.graph().get_stats().unwrap();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
    }
}
