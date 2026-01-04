//! 回退管理器模块
//!
//! 提供节点和推导链的回退功能，支持回退到任意历史版本

use std::path::{Path, PathBuf};
use anyhow::{Result, Context};

use crate::vcs::graph_history::{GraphHistory, NodeSnapshot, NodeHistoryEntry, DerivationEntry};

/// 回退管理器
pub struct RollbackManager {
    /// 项目根目录
    root: PathBuf,
    /// 图历史追踪器
    history: GraphHistory,
}

impl RollbackManager {
    /// 创建回退管理器
    pub fn new(root: &Path) -> Result<Self> {
        let history = GraphHistory::open(root)?;

        Ok(Self {
            root: root.to_path_buf(),
            history,
        })
    }

    /// 回退单个节点到指定版本
    ///
    /// 返回回退后的节点内容
    pub fn rollback_node(&self, node_id: &str, target_hash: &str) -> Result<String> {
        // 获取目标快照
        let snapshot = self.history.get_node_snapshot(node_id, target_hash)?
            .with_context(|| format!("Node snapshot not found: {}@{}", node_id, target_hash))?;

        Ok(snapshot)
    }

    /// 回退单个节点到最新版本
    pub fn rollback_node_to_latest(&self, node_id: &str) -> Result<Option<NodeSnapshot>> {
        self.history.get_latest_node_snapshot(node_id)
    }

    /// 回退节点到指定历史版本
    pub fn rollback_node_to_history(&self, node_id: &str, commit_hash: &str) -> Result<Option<String>> {
        // 获取节点历史
        let history = self.history.get_node_history(node_id)?;

        // 找到指定提交对应的快照
        for entry in history.iter().rev() {
            if entry.commit_hash == commit_hash {
                return self.history.get_node_snapshot(node_id, &entry.hash);
            }
        }

        Ok(None)
    }

    /// 预览节点回退影响
    ///
    /// 返回将受影响的节点列表
    pub fn preview_node_rollback(&self, node_id: &str) -> Result<Vec<String>> {
        let mut affected = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // 查找所有从该节点推导出的节点（这些节点可能会受到影响）
        self.collect_derived_nodes(node_id, &mut affected, &mut visited)?;

        Ok(affected)
    }

    /// 递归收集所有推导出的节点
    fn collect_derived_nodes(
        &self,
        node_id: &str,
        affected: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(node_id) {
            return Ok(());
        }
        visited.insert(node_id.to_string());

        let derived = self.history.get_derived_nodes(node_id)?;
        for entry in derived {
            if !affected.contains(&entry.child_id) {
                affected.push(entry.child_id.clone());
                self.collect_derived_nodes(&entry.child_id, affected, visited)?;
            }
        }

        Ok(())
    }

    /// 回退推导链到根节点
    ///
    /// 将推导链中的所有节点回退到根节点所在的状态
    pub fn rollback_derivation_chain(&self, leaf_node_id: &str) -> Result<Vec<RollbackResult>> {
        let mut results = Vec::new();

        // 构建推导链
        let chain = self.history.build_derivation_chain(leaf_node_id)?;

        // 找到根节点
        let root_id = if let Some(last) = chain.last() {
            last.parent_id.clone()
        } else {
            leaf_node_id.to_string()
        };

        // 获取根节点的最新快照
        if let Some(root_snapshot) = self.history.get_latest_node_snapshot(&root_id)? {
            // 回退根节点
            results.push(RollbackResult {
                node_id: root_id.clone(),
                from_content: None,
                to_content: root_snapshot.content.clone(),
                commit_hash: root_snapshot.commit_hash,
            });

            // 回退推导链上的所有节点
            for entry in chain.iter() {
                // 获取该节点在推导创建时的历史记录
                let node_history = self.history.get_node_history(&entry.child_id)?;
                let historical = node_history.iter()
                    .find(|h| h.commit_hash == entry.commit_hash)
                    .and_then(|h| self.history.get_node_snapshot(&entry.child_id, &h.hash).ok().flatten());

                results.push(RollbackResult {
                    node_id: entry.child_id.clone(),
                    from_content: historical.clone(),
                    to_content: historical.unwrap_or_default(),
                    commit_hash: entry.commit_hash.clone(),
                });
            }
        }

        Ok(results)
    }

    /// 获取节点的完整历史
    pub fn get_node_history(&self, node_id: &str) -> Result<Vec<NodeHistoryEntry>> {
        self.history.get_node_history(node_id)
    }

    /// 获取节点的推导链
    pub fn get_derivation_chain(&self, node_id: &str) -> Result<Vec<DerivationEntry>> {
        self.history.build_derivation_chain(node_id)
    }

    /// 检查节点是否存在
    pub fn node_exists(&self, node_id: &str) -> bool {
        self.history.get_latest_node_snapshot(node_id).ok().flatten().is_some()
    }
}

/// 回退结果
#[derive(Debug, Clone)]
pub struct RollbackResult {
    /// 节点 ID
    pub node_id: String,
    /// 回退前的内容
    pub from_content: Option<String>,
    /// 回退后的内容
    pub to_content: String,
    /// 关联的提交哈希
    pub commit_hash: String,
}

impl RollbackResult {
    /// 检查是否有实际变化
    pub fn has_changes(&self) -> bool {
        match &self.from_content {
            Some(from) => from != &self.to_content,
            None => !self.to_content.is_empty(),
        }
    }
}

/// 回退预览信息
#[derive(Debug, Clone)]
pub struct RollbackPreview {
    /// 将被回退的节点数
    pub node_count: usize,
    /// 影响最大的节点
    pub affected_nodes: Vec<String>,
    /// 最早可回退的提交
    pub earliest_commit: Option<String>,
    /// 最新可回退的提交
    pub latest_commit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rollback_node() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建多个版本的节点快照
        let node_id = "urn:memexia:node:test";
        let content1 = r#"{"id":"urn:memexia:node:test","title":"Version 1"}"#;
        let content2 = r#"{"id":"urn:memexia:node:test","title":"Version 2"}"#;

        let hash1 = history.snapshot_node(node_id, content1, "commit1").unwrap();
        let _hash2 = history.snapshot_node(node_id, content2, "commit2").unwrap();

        // 回退到第一个版本
        let result = rollback.rollback_node(node_id, &hash1).unwrap();
        assert_eq!(result, content1);
    }

    #[test]
    fn test_rollback_node_to_history() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        let node_id = "urn:memexia:node:test";
        let content1 = r#"{"id":"urn:memexia:node:test","v":1}"#;
        let content2 = r#"{"id":"urn:memexia:node:test","v":2}"#;

        history.snapshot_node(node_id, content1, "commit1").unwrap();
        history.snapshot_node(node_id, content2, "commit2").unwrap();

        // 回退到 commit1 对应的版本
        let result = rollback.rollback_node_to_history(node_id, "commit1").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), content1);
    }

    #[test]
    fn test_preview_node_rollback() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建推导链: root -> child1 -> child2
        history.record_derivation("child1", "root", "c1").unwrap();
        history.record_derivation("child2", "child1", "c2").unwrap();

        // 预览 root 节点回退影响
        let affected = rollback.preview_node_rollback("root").unwrap();
        assert_eq!(affected.len(), 2);
        assert!(affected.contains(&"child1".to_string()));
        assert!(affected.contains(&"child2".to_string()));
    }

    #[test]
    fn test_rollback_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建推导链: root -> child1 -> child2
        history.record_derivation("child1", "root", "c1").unwrap();
        history.record_derivation("child2", "child1", "c2").unwrap();

        // 为每个节点创建快照
        history.snapshot_node("root", r#"{"id":"root"}"#, "c1").unwrap();
        history.snapshot_node("child1", r#"{"id":"child1"}"#, "c1").unwrap();
        history.snapshot_node("child2", r#"{"id":"child2"}"#, "c2").unwrap();

        // 回退推导链
        let results = rollback.rollback_derivation_chain("child2").unwrap();
        // 应该包含 root, child1, child2 的回退结果
        assert!(!results.is_empty());
    }

    #[test]
    fn test_rollback_result_has_changes() {
        let result = RollbackResult {
            node_id: "test".to_string(),
            from_content: Some("old".to_string()),
            to_content: "new".to_string(),
            commit_hash: "abc".to_string(),
        };
        assert!(result.has_changes());

        let no_change = RollbackResult {
            node_id: "test".to_string(),
            from_content: Some("same".to_string()),
            to_content: "same".to_string(),
            commit_hash: "abc".to_string(),
        };
        assert!(!no_change.has_changes());
    }

    // ==================== 扩展测试 ====================

    #[test]
    fn test_rollback_node_not_found() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let rollback = RollbackManager::new(path).unwrap();

        // 回退不存在的节点
        let result = rollback.rollback_node("not_exists", "fake_hash");
        assert!(result.is_err());
    }

    #[test]
    fn test_rollback_node_to_history_not_found() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        let node_id = "urn:memexia:node:test";
        history.snapshot_node(node_id, r#"{"id":"urn:memexia:node:test"}"#, "commit1").unwrap();

        // 回退到不存在的提交
        let result = rollback.rollback_node_to_history(node_id, "not_exists").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_rollback_node_to_latest() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        let node_id = "urn:memexia:node:latest";
        let content1 = r#"{"v":1}"#;
        let content2 = r#"{"v":2}"#;

        history.snapshot_node(node_id, content1, "c1").unwrap();
        history.snapshot_node(node_id, content2, "c2").unwrap();

        let latest = rollback.rollback_node_to_latest(node_id).unwrap().unwrap();
        assert_eq!(latest.content, content2);
    }

    #[test]
    fn test_rollback_node_to_latest_not_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let rollback = RollbackManager::new(path).unwrap();

        let latest = rollback.rollback_node_to_latest("not_exists").unwrap();
        assert!(latest.is_none());
    }

    #[test]
    fn test_preview_node_rollback_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 预览孤立节点
        let affected = rollback.preview_node_rollback("orphan").unwrap();
        assert!(affected.is_empty());
    }

    #[test]
    fn test_preview_node_rollback_deep_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建深度推导链: root -> n1 -> n2 -> n3 -> n4 -> n5
        let mut parent = "root".to_string();
        for i in 1..=5 {
            history.record_derivation(&format!("n{}", i), &parent, &format!("c{}", i)).unwrap();
            parent = format!("n{}", i);
        }

        // 预览 root 的影响
        let affected = rollback.preview_node_rollback("root").unwrap();
        assert_eq!(affected.len(), 5);
    }

    #[test]
    fn test_get_node_history() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        let node_id = "urn:memexia:node:history";
        history.snapshot_node(node_id, "v1", "c1").unwrap();
        history.snapshot_node(node_id, "v2", "c2").unwrap();
        history.snapshot_node(node_id, "v3", "c3").unwrap();

        let history = rollback.get_node_history(node_id).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].commit_hash, "c1");
        assert_eq!(history[2].commit_hash, "c3");
    }

    #[test]
    fn test_get_derivation_chain() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建推导链: root -> child1 -> child2 -> child3
        history.record_derivation("child1", "root", "c1").unwrap();
        history.record_derivation("child2", "child1", "c2").unwrap();
        history.record_derivation("child3", "child2", "c3").unwrap();

        let chain = rollback.get_derivation_chain("child3").unwrap();
        assert_eq!(chain.len(), 3);
    }

    #[test]
    fn test_node_exists() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 不存在的节点
        assert!(!rollback.node_exists("not_exists"));

        // 创建节点快照
        history.snapshot_node("exists", r#"{"id":"exists"}"#, "c1").unwrap();

        // 存在的节点
        assert!(rollback.node_exists("exists"));
    }

    #[test]
    fn test_rollback_derivation_chain_empty() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 没有推导关系的节点
        let results = rollback.rollback_derivation_chain("orphan").unwrap();
        // 只有一个根节点回退结果
        assert!(results.is_empty() || results.len() == 1);
    }

    #[test]
    fn test_rollback_derivation_chain_with_snapshots() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        let history = GraphHistory::init(path).unwrap();
        let rollback = RollbackManager::new(path).unwrap();

        // 创建推导链: A -> B -> C
        history.record_derivation("B", "A", "c1").unwrap();
        history.record_derivation("C", "B", "c2").unwrap();

        // 创建快照
        history.snapshot_node("A", r#"{"id":"A"}"#, "c1").unwrap();
        history.snapshot_node("B", r#"{"id":"B"}"#, "c1").unwrap();
        history.snapshot_node("C", r#"{"id":"C"}"#, "c2").unwrap();

        // 回退推导链
        let results = rollback.rollback_derivation_chain("C").unwrap();

        // 应该包含 A 和 B 的回退结果
        let node_ids: Vec<_> = results.iter().map(|r| r.node_id.clone()).collect();
        assert!(node_ids.contains(&"A".to_string()));
        assert!(node_ids.contains(&"B".to_string()));
    }

    #[test]
    fn test_rollback_result_no_from_content() {
        let result = RollbackResult {
            node_id: "test".to_string(),
            from_content: None,
            to_content: "new content".to_string(),
            commit_hash: "abc".to_string(),
        };
        assert!(result.has_changes());
    }

    #[test]
    fn test_rollback_result_empty_to_content() {
        let result = RollbackResult {
            node_id: "test".to_string(),
            from_content: Some("old content".to_string()),
            to_content: "".to_string(),
            commit_hash: "abc".to_string(),
        };
        assert!(result.has_changes());
    }
}
