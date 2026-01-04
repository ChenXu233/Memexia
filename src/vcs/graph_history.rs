//! 图历史追踪模块
//!
//! 记录图数据的变化历史，支持快照存储和差异计算
//! 包含节点快照和推导链追踪功能

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::storage::Storage;
use crate::core::object::hash_content;

/// 将节点ID转换为安全的目录名
/// 替换 Windows 不允许的字符（如冒号）为空格下划线
fn sanitize_node_id_for_path(node_id: &str) -> String {
    node_id.replace(':', "_")
}

/// 图历史追踪器
pub struct GraphHistory {
    /// 历史目录
    root: PathBuf,
    /// 完整图快照目录
    snapshots_dir: PathBuf,
    /// 节点历史目录
    nodes_dir: PathBuf,
    /// 推导链目录
    derivations_dir: PathBuf,
}

impl GraphHistory {
    /// 初始化新的图历史
    pub fn init(root: &Path) -> Result<Self> {
        let history_dir = root.join(".memexia/history");
        let snapshots_dir = history_dir.join("snapshots");
        let nodes_dir = history_dir.join("nodes");
        let derivations_dir = history_dir.join("derivations");

        fs::create_dir_all(&snapshots_dir)
            .with_context(|| format!("Failed to create snapshots dir: {:?}", snapshots_dir))?;
        fs::create_dir_all(&nodes_dir)
            .with_context(|| format!("Failed to create nodes dir: {:?}", nodes_dir))?;
        fs::create_dir_all(&derivations_dir)
            .with_context(|| format!("Failed to create derivations dir: {:?}", derivations_dir))?;

        Ok(Self {
            root: history_dir,
            snapshots_dir,
            nodes_dir,
            derivations_dir,
        })
    }

    /// 打开已存在的图历史
    pub fn open(root: &Path) -> Result<Self> {
        let history_dir = root.join(".memexia/history");
        let snapshots_dir = history_dir.join("snapshots");
        let nodes_dir = history_dir.join("nodes");
        let derivations_dir = history_dir.join("derivations");

        if !history_dir.exists() {
            fs::create_dir_all(&snapshots_dir)
                .with_context(|| format!("Failed to create history dir: {:?}", snapshots_dir))?;
            fs::create_dir_all(&nodes_dir)
                .with_context(|| format!("Failed to create nodes dir: {:?}", nodes_dir))?;
            fs::create_dir_all(&derivations_dir)
                .with_context(|| format!("Failed to create derivations dir: {:?}", derivations_dir))?;
        }

        Ok(Self {
            root: history_dir,
            snapshots_dir,
            nodes_dir,
            derivations_dir,
        })
    }

    /// 创建图快照
    ///
    /// 将当前图数据导出为 N-Quads，计算哈希并存储
    pub fn snapshot(&self, storage: &Storage) -> Result<String> {
        // 导出图为 N-Quads
        let nquads = storage.graph().export_nquads()?;

        // 计算哈希
        let hash = hash_content(nquads.as_bytes());

        // 存储快照
        self.store_snapshot(&hash, &nquads)?;

        Ok(hash)
    }

    /// 存储快照
    fn store_snapshot(&self, hash: &str, nquads: &str) -> Result<()> {
        let (dir_name, file_name) = hash.split_at(2);
        let snapshot_dir = self.snapshots_dir.join(dir_name);
        let snapshot_path = snapshot_dir.join(file_name);

        fs::create_dir_all(&snapshot_dir)
            .with_context(|| format!("Failed to create snapshot dir: {:?}", snapshot_dir))?;

        // 同时存储元数据
        let meta = SnapshotMetadata {
            hash: hash.to_string(),
            size: nquads.len(),
            timestamp: Utc::now(),
        };

        let meta_path = snapshot_path.with_extension("meta");
        let meta_json = serde_json::to_string(&meta)?;
        fs::write(&meta_path, meta_json)?;

        // 存储 N-Quads 数据
        fs::write(&snapshot_path, nquads)?;

        Ok(())
    }

    /// 获取快照
    pub fn get_snapshot(&self, hash: &str) -> Result<GraphSnapshot> {
        let (dir_name, file_name) = hash.split_at(2);
        let snapshot_path = self.snapshots_dir.join(dir_name).join(file_name);

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot not found: {}", hash);
        }

        let nquads = fs::read_to_string(&snapshot_path)?;

        // 读取元数据
        let meta_path = snapshot_path.with_extension("meta");
        let meta: SnapshotMetadata = if meta_path.exists() {
            let meta_json = fs::read_to_string(&meta_path)?;
            serde_json::from_str(&meta_json)?
        } else {
            SnapshotMetadata {
                hash: hash.to_string(),
                size: nquads.len(),
                timestamp: Utc::now(),
            }
        };

        Ok(GraphSnapshot {
            hash: meta.hash,
            nquads,
            timestamp: meta.timestamp,
        })
    }

    /// 记录提交关联
    ///
    /// 将 Git 提交哈希与图快照哈希关联
    pub fn record(&self, commit_hash: &str, graph_hash: &str) -> Result<()> {
        let link_file = self.root.join("commit-links");

        // 以 append 模式写入
        use std::fs::OpenOptions;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&link_file)?;

        writeln!(file, "{} -> {}", commit_hash, graph_hash)?;

        Ok(())
    }

    /// 获取提交关联的图哈希
    pub fn get_commit_graph_hash(&self, commit_hash: &str) -> Result<Option<String>> {
        let link_file = self.root.join("commit-links");

        if !link_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&link_file)?;

        for line in content.lines() {
            if let Some((c_hash, g_hash)) = line.split_once(" -> ") {
                if c_hash.trim() == commit_hash {
                    return Ok(Some(g_hash.trim().to_string()));
                }
            }
        }

        Ok(None)
    }

    /// 获取历史记录
    pub fn get_history(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let link_file = self.root.join("commit-links");

        if !link_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&link_file)?;
        let mut entries: Vec<HistoryEntry> = Vec::new();

        for line in content.lines().rev().take(limit) {
            if let Some((c_hash, g_hash)) = line.split_once(" -> ") {
                let snapshot = self.get_snapshot(g_hash.trim()).ok();

                entries.push(HistoryEntry {
                    commit_hash: c_hash.trim().to_string(),
                    graph_hash: g_hash.trim().to_string(),
                    snapshot,
                });
            }
        }

        Ok(entries)
    }

    /// 计算两个快照之间的差异
    pub fn diff(&self, old_hash: &str, new_hash: &str) -> Result<GraphDelta> {
        let old_snapshot = self.get_snapshot(old_hash)?;
        let new_snapshot = self.get_snapshot(new_hash)?;

        let old_lines: std::collections::HashSet<_> = old_snapshot.nquads.lines().collect();
        let new_lines: std::collections::HashSet<_> = new_snapshot.nquads.lines().collect();

        let added: Vec<_> = new_lines.difference(&old_lines).collect();
        let removed: Vec<_> = old_lines.difference(&new_lines).collect();

        Ok(GraphDelta {
            added_lines: added.iter().map(|s| s.to_string()).collect(),
            removed_lines: removed.iter().map(|s| s.to_string()).collect(),
        })
    }

    // ==================== 节点快照功能 ====================

    /// 创建单个节点的快照
    ///
    /// 记录节点的完整内容和哈希
    pub fn snapshot_node(
        &self,
        node_id: &str,
        content: &str,
        commit_hash: &str,
    ) -> Result<String> {
        // 计算哈希
        let hash = hash_content(content.as_bytes());

        // 存储快照
        self.store_node_snapshot(node_id, &hash, content, commit_hash)?;

        Ok(hash)
    }

    /// 存储节点快照
    fn store_node_snapshot(
        &self,
        node_id: &str,
        hash: &str,
        content: &str,
        commit_hash: &str,
    ) -> Result<()> {
        let safe_id = sanitize_node_id_for_path(node_id);
        let node_dir = self.nodes_dir.join(&safe_id);
        fs::create_dir_all(&node_dir)
            .with_context(|| format!("Failed to create node dir: {:?}", node_dir))?;

        // 存储快照数据
        let snapshot_path = node_dir.join(hash);
        fs::write(&snapshot_path, content)?;

        // 更新节点历史索引
        let history_file = node_dir.join("history.json");
        let mut history: Vec<NodeHistoryEntry> = if history_file.exists() {
            let content = fs::read_to_string(&history_file)?;
            serde_json::from_str(&content)?
        } else {
            Vec::new()
        };

        history.push(NodeHistoryEntry {
            hash: hash.to_string(),
            timestamp: Utc::now(),
            commit_hash: commit_hash.to_string(),
        });

        let json = serde_json::to_string(&history)?;
        fs::write(&history_file, json)?;

        Ok(())
    }

    /// 获取节点的历史记录
    pub fn get_node_history(&self, node_id: &str) -> Result<Vec<NodeHistoryEntry>> {
        let safe_id = sanitize_node_id_for_path(node_id);
        let history_file = self.nodes_dir.join(&safe_id).join("history.json");

        if !history_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&history_file)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// 获取节点指定版本的快照
    pub fn get_node_snapshot(&self, node_id: &str, hash: &str) -> Result<Option<String>> {
        let safe_id = sanitize_node_id_for_path(node_id);
        let snapshot_path = self.nodes_dir.join(&safe_id).join(hash);

        if !snapshot_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&snapshot_path)?;
        Ok(Some(content))
    }

    /// 获取节点最新版本的快照
    pub fn get_latest_node_snapshot(&self, node_id: &str) -> Result<Option<NodeSnapshot>> {
        let history = self.get_node_history(node_id)?;

        if let Some(latest) = history.last() {
            if let Some(content) = self.get_node_snapshot(node_id, &latest.hash)? {
                return Ok(Some(NodeSnapshot {
                    node_id: node_id.to_string(),
                    hash: latest.hash.clone(),
                    content,
                    timestamp: latest.timestamp,
                    commit_hash: latest.commit_hash.clone(),
                }));
            }
        }

        Ok(None)
    }

    // ==================== 推导链追踪功能 ====================

    /// 记录推导关系
    ///
    /// 记录从父节点推导出子节点的关系
    pub fn record_derivation(
        &self,
        child_id: &str,
        parent_id: &str,
        commit_hash: &str,
    ) -> Result<()> {
        let derivations_file = self.derivations_dir.join("derivations.json");

        // 读取现有推导记录
        let mut derivations: Vec<DerivationRecord> = if derivations_file.exists() {
            let content = fs::read_to_string(&derivations_file)?;
            serde_json::from_str(&content)?
        } else {
            Vec::new()
        };

        // 添加新记录
        derivations.push(DerivationRecord {
            child_id: child_id.to_string(),
            parent_id: parent_id.to_string(),
            timestamp: Utc::now(),
            commit_hash: commit_hash.to_string(),
        });

        let json = serde_json::to_string(&derivations)?;
        fs::write(&derivations_file, json)?;

        Ok(())
    }

    /// 获取某节点的所有推导来源（父节点）
    pub fn get_derivations(&self, node_id: &str) -> Result<Vec<DerivationEntry>> {
        let derivations_file = self.derivations_dir.join("derivations.json");

        if !derivations_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&derivations_file)?;
        let records: Vec<DerivationRecord> = serde_json::from_str(&content)?;

        Ok(records
            .into_iter()
            .filter(|r| r.child_id == node_id)
            .map(|r| DerivationEntry {
                child_id: r.child_id,
                parent_id: r.parent_id,
                timestamp: r.timestamp,
                commit_hash: r.commit_hash,
            })
            .collect())
    }

    /// 获取从某节点推导出的所有子节点
    pub fn get_derived_nodes(&self, node_id: &str) -> Result<Vec<DerivationEntry>> {
        let derivations_file = self.derivations_dir.join("derivations.json");

        if !derivations_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&derivations_file)?;
        let records: Vec<DerivationRecord> = serde_json::from_str(&content)?;

        Ok(records
            .into_iter()
            .filter(|r| r.parent_id == node_id)
            .map(|r| DerivationEntry {
                child_id: r.child_id,
                parent_id: r.parent_id,
                timestamp: r.timestamp,
                commit_hash: r.commit_hash,
            })
            .collect())
    }

    /// 构建完整的推导链
    ///
    /// 返回从给定节点到根节点的完整推导路径
    pub fn build_derivation_chain(&self, node_id: &str) -> Result<Vec<DerivationEntry>> {
        let mut chain = Vec::new();
        let mut current = node_id.to_string();
        let mut visited = std::collections::HashSet::new();

        // 向上追溯推导来源
        while !visited.contains(&current) {
            visited.insert(current.clone());

            let parents = self.get_derivations(&current)?;
            if let Some(first) = parents.first() {
                chain.push(first.clone());
                current = first.parent_id.clone();
            } else {
                break;
            }
        }

        Ok(chain)
    }

    /// 构建反向推导链（从根到叶子）
    pub fn build_reverse_derivation_chain(&self, root_id: &str, limit: usize) -> Result<Vec<DerivationEntry>> {
        let mut chain = Vec::new();
        let mut current = root_id.to_string();
        let mut visited = std::collections::HashSet::new();
        let mut depth = 0;

        // 向下追溯推导出的节点
        while depth < limit {
            if visited.contains(&current) {
                break;
            }
            visited.insert(current.clone());

            let derived = self.get_derived_nodes(&current)?;
            if let Some(first) = derived.first() {
                chain.push(first.clone());
                current = first.child_id.clone();
                depth += 1;
            } else {
                break;
            }
        }

        Ok(chain)
    }
}

/// 图快照
#[derive(Debug)]
pub struct GraphSnapshot {
    /// 快照哈希
    pub hash: String,
    /// N-Quads 格式的图数据
    pub nquads: String,
    /// 创建时间
    pub timestamp: DateTime<Utc>,
}

/// 快照元数据
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotMetadata {
    hash: String,
    size: usize,
    timestamp: DateTime<Utc>,
}

/// 图差异
#[derive(Debug)]
pub struct GraphDelta {
    /// 新增的行
    pub added_lines: Vec<String>,
    /// 删除的行
    pub removed_lines: Vec<String>,
}

impl GraphDelta {
    /// 检查是否有变化
    pub fn is_empty(&self) -> bool {
        self.added_lines.is_empty() && self.removed_lines.is_empty()
    }

    /// 统计变化数量
    pub fn stats(&self) -> (usize, usize) {
        (self.added_lines.len(), self.removed_lines.len())
    }
}

/// 历史条目
#[derive(Debug)]
pub struct HistoryEntry {
    /// Git 提交哈希
    pub commit_hash: String,
    /// 图快照哈希
    pub graph_hash: String,
    /// 快照数据
    pub snapshot: Option<GraphSnapshot>,
}

// ==================== 节点快照类型 ====================

/// 单个节点的快照
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    /// 节点 ID
    pub node_id: String,
    /// 快照哈希
    pub hash: String,
    /// 节点内容（JSON 序列化）
    pub content: String,
    /// 创建时间
    pub timestamp: DateTime<Utc>,
    /// 关联的提交哈希
    pub commit_hash: String,
}

/// 节点历史条目（用于索引）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHistoryEntry {
    /// 快照哈希
    pub hash: String,
    /// 创建时间
    pub timestamp: DateTime<Utc>,
    /// 关联的提交哈希
    pub commit_hash: String,
}

// ==================== 推导链类型 ====================

/// 推导链条目
#[derive(Debug, Clone)]
pub struct DerivationEntry {
    /// 子节点 ID（推导出的节点）
    pub child_id: String,
    /// 父节点 ID（来源节点）
    pub parent_id: String,
    /// 创建时间
    pub timestamp: DateTime<Utc>,
    /// 关联的提交哈希
    pub commit_hash: String,
}

/// 推导记录（存储格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DerivationRecord {
    /// 子节点 ID
    child_id: String,
    /// 父节点 ID
    parent_id: String,
    /// 创建时间
    timestamp: DateTime<Utc>,
    /// 关联的提交哈希
    commit_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::storage::Storage;

    #[test]
    fn test_snapshot() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        // 先创建存储（会创建 .memexia 目录）
        let storage = Storage::init(path).unwrap();

        // 然后创建图历史
        let history = GraphHistory::init(path).unwrap();

        // 创建快照
        let hash = history.snapshot(&storage).unwrap();
        assert!(!hash.is_empty());

        // 获取快照
        let snapshot = history.get_snapshot(&hash).unwrap();
        assert_eq!(snapshot.hash, hash);
    }

    #[test]
    fn test_commit_link() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录关联
        history.record("abc123", "def456").unwrap();

        // 获取关联
        let graph_hash = history.get_commit_graph_hash("abc123").unwrap();
        assert_eq!(graph_hash, Some("def456".to_string()));
    }

    // ==================== 节点快照测试 ====================

    #[test]
    fn test_node_snapshot() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建节点快照
        let node_id = "urn:memexia:node:test";
        let content = r#"{"id":"urn:memexia:node:test","title":"Test Node","content":"Hello"}"#;
        let hash = history.snapshot_node(node_id, content, "abc123").unwrap();

        assert!(!hash.is_empty());

        // 获取节点历史
        let node_history = history.get_node_history(node_id).unwrap();
        assert_eq!(node_history.len(), 1);
        assert_eq!(node_history[0].hash, hash);
        assert_eq!(node_history[0].commit_hash, "abc123");
    }

    #[test]
    fn test_node_multiple_snapshots() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        let node_id = "urn:memexia:node:test";
        let content1 = r#"{"id":"urn:memexia:node:test","title":"Test","v":1}"#;
        let content2 = r#"{"id":"urn:memexia:node:test","title":"Test","v":2}"#;

        // 创建多个快照
        let hash1 = history.snapshot_node(node_id, content1, "commit1").unwrap();
        let hash2 = history.snapshot_node(node_id, content2, "commit2").unwrap();

        assert_ne!(hash1, hash2);

        // 获取历史记录
        let node_history = history.get_node_history(node_id).unwrap();
        assert_eq!(node_history.len(), 2);
        assert_eq!(node_history[0].commit_hash, "commit1");
        assert_eq!(node_history[1].commit_hash, "commit2");

        // 获取最新快照
        let latest = history.get_latest_node_snapshot(node_id).unwrap();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().hash, hash2);
    }

    #[test]
    fn test_get_node_snapshot() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        let node_id = "urn:memexia:node:test";
        let content = r#"{"id":"urn:memexia:node:test","title":"Test"}"#;
        let hash = history.snapshot_node(node_id, content, "commit1").unwrap();

        // 获取快照内容
        let retrieved = history.get_node_snapshot(node_id, &hash).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), content);

        // 获取不存在的快照
        let not_found = history.get_node_snapshot(node_id, "nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    // ==================== 推导链测试 ====================

    #[test]
    fn test_derivation_record() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录推导关系
        history.record_derivation("child_id", "parent_id", "commit123").unwrap();

        // 获取推导来源
        let derivations = history.get_derivations("child_id").unwrap();
        assert_eq!(derivations.len(), 1);
        assert_eq!(derivations[0].parent_id, "parent_id");
        assert_eq!(derivations[0].commit_hash, "commit123");
    }

    #[test]
    fn test_get_derived_nodes() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录推导关系
        history.record_derivation("child1", "parent", "commit1").unwrap();
        history.record_derivation("child2", "parent", "commit2").unwrap();

        // 获取推导出的节点
        let derived = history.get_derived_nodes("parent").unwrap();
        assert_eq!(derived.len(), 2);
        let child_ids: Vec<_> = derived.iter().map(|d| d.child_id.clone()).collect();
        assert!(child_ids.contains(&"child1".to_string()));
        assert!(child_ids.contains(&"child2".to_string()));
    }

    #[test]
    fn test_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建推导链: root -> child1 -> child2 -> child3
        history.record_derivation("child1", "root", "c1").unwrap();
        history.record_derivation("child2", "child1", "c2").unwrap();
        history.record_derivation("child3", "child2", "c3").unwrap();

        // 构建推导链
        let chain = history.build_derivation_chain("child3").unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].parent_id, "child2");
        assert_eq!(chain[1].parent_id, "child1");
        assert_eq!(chain[2].parent_id, "root");
    }

    #[test]
    fn test_reverse_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建推导链: root -> child1 -> child2
        history.record_derivation("child1", "root", "c1").unwrap();
        history.record_derivation("child2", "child1", "c2").unwrap();
        history.record_derivation("child3", "child2", "c3").unwrap();

        // 构建反向推导链
        let chain = history.build_reverse_derivation_chain("root", 5).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].child_id, "child1");
        assert_eq!(chain[1].child_id, "child2");
        assert_eq!(chain[2].child_id, "child3");
    }

    #[test]
    fn test_derivation_chain_no_cycle() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录推导关系（形成循环）
        history.record_derivation("node2", "node1", "c1").unwrap();
        history.record_derivation("node1", "node2", "c2").unwrap();

        // 应该不会无限循环
        let chain = history.build_derivation_chain("node2").unwrap();
        assert!(chain.len() <= 2);
    }

    // ==================== 扩展测试 ====================

    #[test]
    fn test_get_latest_node_snapshot() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        let node_id = "urn:memexia:node:latest";
        let content1 = r#"{"id":"urn:memexia:node:latest","version":1}"#;
        let content2 = r#"{"id":"urn:memexia:node:latest","version":2}"#;
        let content3 = r#"{"id":"urn:memexia:node:latest","version":3}"#;

        let _hash1 = history.snapshot_node(node_id, content1, "c1").unwrap();
        let _hash2 = history.snapshot_node(node_id, content2, "c2").unwrap();
        let hash3 = history.snapshot_node(node_id, content3, "c3").unwrap();

        // 获取最新快照
        let latest = history.get_latest_node_snapshot(node_id).unwrap().unwrap();
        assert_eq!(latest.hash, hash3);
        assert_eq!(latest.content, content3);
        assert_eq!(latest.commit_hash, "c3");
    }

    #[test]
    fn test_get_latest_node_snapshot_not_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 获取不存在的节点
        let latest = history.get_latest_node_snapshot("not_exists").unwrap();
        assert!(latest.is_none());
    }

    #[test]
    fn test_empty_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 没有推导关系的节点
        let chain = history.build_derivation_chain("orphan").unwrap();
        assert!(chain.is_empty());
    }

    #[test]
    fn test_reverse_derivation_chain_limit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建长推导链: root -> n1 -> n2 -> n3 -> n4 -> n5
        history.record_derivation("n1", "root", "c1").unwrap();
        history.record_derivation("n2", "n1", "c2").unwrap();
        history.record_derivation("n3", "n2", "c3").unwrap();
        history.record_derivation("n4", "n3", "c4").unwrap();
        history.record_derivation("n5", "n4", "c5").unwrap();

        // 限制深度为 2
        let chain = history.build_reverse_derivation_chain("root", 2).unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].child_id, "n1");
        assert_eq!(chain[1].child_id, "n2");
    }

    #[test]
    fn test_multiple_derivations_same_parent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 同一个父节点有多个子节点
        history.record_derivation("child1", "parent", "c1").unwrap();
        history.record_derivation("child2", "parent", "c2").unwrap();
        history.record_derivation("child3", "parent", "c3").unwrap();

        // 获取推导出的节点
        let derived = history.get_derived_nodes("parent").unwrap();
        assert_eq!(derived.len(), 3);

        // 获取推导来源
        for child in &["child1", "child2", "child3"] {
            let derivations = history.get_derivations(child).unwrap();
            assert_eq!(derivations.len(), 1);
            assert_eq!(derivations[0].parent_id, "parent");
        }
    }

    #[test]
    fn test_node_history_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 获取不存在的节点历史
        let history = history.get_node_history("not_exists").unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_node_snapshot_not_found() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 节点存在但快照不存在
        let node_id = "urn:memexia:node:exists";
        history.snapshot_node(node_id, r#"{"id":"urn:memexia:node:exists"}"#, "c1").unwrap();

        // 获取不存在的快照
        let snapshot = history.get_node_snapshot(node_id, "fake_hash").unwrap();
        assert!(snapshot.is_none());
    }

    #[test]
    fn test_derivation_empty_derived_nodes() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 没有子节点的节点
        let derived = history.get_derived_nodes("orphan").unwrap();
        assert!(derived.is_empty());
    }

    #[test]
    fn test_derivation_empty_derivations() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 没有父节点的节点
        let derivations = history.get_derivations("root_node").unwrap();
        assert!(derivations.is_empty());
    }

    #[test]
    fn test_long_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建长度为 10 的推导链
        let mut previous = "root".to_string();
        for i in 1..=10 {
            history.record_derivation(&format!("node{}", i), &previous, &format!("c{}", i)).unwrap();
            previous = format!("node{}", i);
        }

        // 构建完整的推导链
        let chain = history.build_derivation_chain("node10").unwrap();
        assert_eq!(chain.len(), 10);

        // 验证顺序（从叶到根）
        assert_eq!(chain[0].parent_id, "node9");
        assert_eq!(chain[9].parent_id, "root");
    }

    #[test]
    fn test_complex_derivation_graph() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 创建复杂推导图:
        //     root
        //    /    \
        //   n1     n2
        //   |      |
        //   n3    n4
        //    \    /
        //     n5
        history.record_derivation("n1", "root", "c1").unwrap();
        history.record_derivation("n2", "root", "c2").unwrap();
        history.record_derivation("n3", "n1", "c3").unwrap();
        history.record_derivation("n4", "n2", "c4").unwrap();
        history.record_derivation("n5", "n3", "c5").unwrap();
        history.record_derivation("n5", "n4", "c6").unwrap();  // n5 有多个父节点

        // n5 的推导链应该追溯到一个父节点（第一个匹配的）
        let chain = history.build_derivation_chain("n5").unwrap();
        assert!(!chain.is_empty());
        assert_eq!(chain.last().unwrap().parent_id, "root");
    }

    #[test]
    fn test_graph_diff_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let storage = Storage::init(path).unwrap();
        let history = GraphHistory::init(path).unwrap();

        // 创建快照
        let hash1 = history.snapshot(&storage).unwrap();

        // 同一个图两次快照应该没有差异
        let hash2 = history.snapshot(&storage).unwrap();
        let diff = history.diff(&hash1, &hash2).unwrap();

        assert!(diff.is_empty());
    }

    #[test]
    fn test_history_entry() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录提交关联
        history.record("commit1", "graph1").unwrap();
        history.record("commit2", "graph2").unwrap();

        // 获取历史
        let entries = history.get_history(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].commit_hash, "commit2");  // 最新的在前
        assert_eq!(entries[1].commit_hash, "commit1");
    }

    #[test]
    fn test_history_limit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();

        // 记录多个提交
        for i in 1..=10 {
            history.record(&format!("commit{}", i), &format!("graph{}", i)).unwrap();
        }

        // 限制获取数量
        let entries = history.get_history(5).unwrap();
        assert_eq!(entries.len(), 5);
    }
}
