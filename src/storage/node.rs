//! 节点数据模型定义
//!
//! 定义 Memexia 知识图谱中的节点类型和数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 节点类型枚举
///
/// 符合项目文档 3.2.1 定义的节点类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum NodeType {
    /// 核心思想、理论、定义
    #[serde(rename = "Concept")]
    Concept,
    /// 未解决的问题、研究问题
    #[serde(rename = "Question")]
    Question,
    /// 数据、引用、实验证据
    #[serde(rename = "Evidence")]
    Evidence,
    /// 书籍、文章、链接引用
    #[serde(rename = "Resource")]
    Resource,
    /// 思想家、作者、协作者
    #[serde(rename = "Person")]
    Person,
    /// 历史事件、个人时刻
    #[serde(rename = "Event")]
    Event,
    /// 对思想本身的反思
    #[serde(rename = "Meta")]
    Meta,
}

impl Default for NodeType {
    fn default() -> Self {
        NodeType::Concept
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Concept => write!(f, "Concept"),
            NodeType::Question => write!(f, "Question"),
            NodeType::Evidence => write!(f, "Evidence"),
            NodeType::Resource => write!(f, "Resource"),
            NodeType::Person => write!(f, "Person"),
            NodeType::Event => write!(f, "Event"),
            NodeType::Meta => write!(f, "Meta"),
        }
    }
}

/// 节点结构体
///
/// 表示知识图谱中的基本单元，包含内容和元数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// 节点的唯一标识符，URN 格式
    ///
    /// 格式: `urn:memexia:file:{relative_path}` 或 `urn:memexia:generated:{uuid}`
    pub id: String,

    /// 节点类型
    pub node_type: NodeType,

    /// 节点标题
    pub title: String,

    /// Markdown 内容
    #[serde(default)]
    pub content: Option<String>,

    /// 标签列表
    #[serde(default)]
    pub tags: Vec<String>,

    /// 扩展元数据
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 最后修改时间
    pub updated_at: DateTime<Utc>,
}

impl Node {
    /// 创建新节点
    ///
    /// # Arguments
    ///
    /// * `id` - 节点 ID
    /// * `node_type` - 节点类型
    /// * `title` - 节点标题
    ///
    /// # Returns
    ///
    /// 带有默认值的 `Node` 实例
    pub fn new(id: impl Into<String>, node_type: NodeType, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            node_type,
            title: title.into(),
            content: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 从文件路径创建节点
    ///
    /// # Arguments
    ///
    /// * `file_path` - 相对于仓库根目录的文件路径
    ///
    /// # Returns
    ///
    /// 对应文件的节点
    pub fn from_file_path(file_path: &std::path::Path) -> Self {
        let id = Self::file_path_to_id(file_path);
        let title = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        Self::new(id, NodeType::Concept, title)
    }

    /// 将文件路径转换为 URN 格式 ID
    fn file_path_to_id(file_path: &std::path::Path) -> String {
        let path_str = file_path.to_string_lossy().replace('\\', "/");
        format!("urn:memexia:file:{}", path_str)
    }

    /// 更新节点内容
    ///
    /// # Arguments
    ///
    /// * `content` - 新的 Markdown 内容
    pub fn update_content(&mut self, content: impl Into<String>) {
        self.content = Some(content.into());
        self.updated_at = Utc::now();
    }

    /// 添加标签
    ///
    /// # Arguments
    ///
    /// * `tag` - 要添加的标签
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self.updated_at = Utc::now();
    }

    /// 移除标签
    ///
    /// # Arguments
    ///
    /// * `tag` - 要移除的标签
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_new() {
        let node = Node::new("test-id", NodeType::Concept, "Test Title");

        assert_eq!(node.id, "test-id");
        assert_eq!(node.node_type, NodeType::Concept);
        assert_eq!(node.title, "Test Title");
        assert!(node.content.is_none());
        assert!(node.tags.is_empty());
    }

    #[test]
    fn test_node_from_file_path() {
        let node = Node::from_file_path(std::path::Path::new("notes/philosophy/free_will.md"));

        assert_eq!(
            node.id,
            "urn:memexia:file:notes/philosophy/free_will.md"
        );
        assert_eq!(node.node_type, NodeType::Concept);
        assert_eq!(node.title, "free_will");
    }

    #[test]
    fn test_node_add_tag() {
        let mut node = Node::new("test", NodeType::Concept, "Test");
        node.add_tag("philosophy");
        node.add_tag("ethics");

        assert_eq!(node.tags, vec!["philosophy", "ethics"]);
    }

    #[test]
    fn test_node_add_duplicate_tag() {
        let mut node = Node::new("test", NodeType::Concept, "Test");
        node.add_tag("philosophy");
        node.add_tag("philosophy");

        assert_eq!(node.tags.len(), 1);
        assert_eq!(node.tags[0], "philosophy");
    }

    #[test]
    fn test_node_serde_roundtrip() {
        let mut node = Node::new("test-id", NodeType::Question, "Test Question");
        node.content = Some("This is a question".to_string());
        node.add_tag("test");

        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: Node = serde_json::from_str(&serialized).unwrap();

        assert_eq!(node, deserialized);
    }
}
