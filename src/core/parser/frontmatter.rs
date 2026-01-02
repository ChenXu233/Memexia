//! YAML Frontmatter 解析模块
//!
//! 解析 Markdown 文件头部的 YAML 格式元数据
//!
//! ## Frontmatter 格式
//!
//! ```yaml
//! ---
//! title: 节点标题
//! type: Concept
//! tags: [哲学, 心灵]
//! summary: 简短描述
//! ---
//! ```

use crate::storage::{Node, NodeType};
use yaml_rust2::{Yaml, YamlLoader};

/// Frontmatter 结构
///
/// 解析后的 YAML 元数据
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    /// 节点标题
    pub title: Option<String>,
    /// 节点类型（默认 Concept）
    pub node_type: NodeType,
    /// 标签列表
    pub tags: Vec<String>,
    /// 简短描述
    pub summary: Option<String>,
}

impl Frontmatter {
    /// 创建新的 Frontmatter
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 Node 转换为 Frontmatter
    pub fn from_node(node: &Node) -> Self {
        Self {
            title: Some(node.title.clone()),
            node_type: node.node_type.clone(),
            tags: node.tags.clone(),
            summary: node.content.clone(),
        }
    }

    /// 转换为 YAML 字符串
    pub fn to_yaml(&self) -> String {
        let mut yaml = String::new();

        if let Some(title) = &self.title {
            yaml.push_str(&format!("title: \"{}\"\n", escape_yaml_string(title)));
        }

        yaml.push_str(&format!("type: {}\n", self.node_type));

        if !self.tags.is_empty() {
            yaml.push_str("tags: [");
            for (i, tag) in self.tags.iter().enumerate() {
                if i > 0 {
                    yaml.push_str(", ");
                }
                yaml.push_str(&escape_yaml_string(tag));
            }
            yaml.push_str("]\n");
        }

        if let Some(summary) = &self.summary {
            yaml.push_str(&format!("summary: \"{}\"\n", escape_yaml_string(summary)));
        }

        yaml
    }
}

/// 解析 YAML frontmatter
///
/// # Arguments
///
/// * `content` - 包含 frontmatter 的 Markdown 内容
///
/// # Returns
///
/// 解析后的 Frontmatter，如果 frontmatter 不存在或解析失败返回 None
pub fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    // 检查是否有 frontmatter
    let start = content.trim_start();
    if !start.starts_with("---") {
        return None;
    }

    // 找到结束 ---
    let end_marker = match start[4..].find("---") {
        Some(pos) => pos + 4,
        None => return None,
    };

    let yaml_str = &start[4..end_marker];

    // 解析 YAML
    let docs = YamlLoader::load_from_str(yaml_str).ok()?;
    let doc = docs.first()?;
    let hash = doc.as_hash()?;
    let mut frontmatter = Frontmatter::new();

    // 解析各字段
    if let Some(title) = hash.get(&Yaml::String("title".to_string())) {
        frontmatter.title = title.as_str().map(|s| s.to_string());
    }

    if let Some(node_type) = hash.get(&Yaml::String("type".to_string())) {
        frontmatter.node_type = parse_node_type(node_type.as_str().unwrap_or("Concept"));
    }

    if let Some(tags) = hash.get(&Yaml::String("tags".to_string())) {
        if let Some(tags_array) = tags.as_vec() {
            for tag in tags_array {
                if let Some(tag_str) = tag.as_str() {
                    frontmatter.tags.push(tag_str.to_string());
                }
            }
        }
    }

    if let Some(summary) = hash.get(&Yaml::String("summary".to_string())) {
        frontmatter.summary = summary.as_str().map(|s| s.to_string());
    }

    Some(frontmatter)
}

/// 解析节点类型字符串
fn parse_node_type(s: &str) -> NodeType {
    match s.to_lowercase().as_str() {
        "concept" => NodeType::Concept,
        "question" => NodeType::Question,
        "evidence" => NodeType::Evidence,
        "resource" => NodeType::Resource,
        "person" => NodeType::Person,
        "event" => NodeType::Event,
        "meta" => NodeType::Meta,
        _ => NodeType::Concept,
    }
}

/// 转义 YAML 字符串中的特殊字符
fn escape_yaml_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c => result.push(c),
        }
    }
    result
}

/// 检查内容是否有 frontmatter
pub fn has_frontmatter(content: &str) -> bool {
    content.trim_start().starts_with("---")
}

/// 从内容中提取并移除 frontmatter
///
/// # Returns
///
/// (frontmatter_yaml, content_without_frontmatter)
pub fn extract_frontmatter(content: &str) -> (Option<String>, String) {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    let end_marker = match trimmed[4..].find("---") {
        Some(pos) => pos + 4,
        None => return (None, content.to_string()),
    };

    let yaml_str = &trimmed[4..end_marker];
    let remaining = &trimmed[end_marker + 4..];

    (Some(yaml_str.to_string()), remaining.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = r#"---
title: 自由意志
type: Concept
tags: [哲学, 心灵]
summary: 关于自由意志的讨论
---

# 自由意志

自由意志是..."#;

        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, Some("自由意志".to_string()));
        assert_eq!(fm.node_type, NodeType::Concept);
        assert_eq!(fm.tags, vec!["哲学", "心灵"]);
        assert_eq!(fm.summary, Some("关于自由意志的讨论".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_minimal() {
        let content = r#"---
title: 最小示例
---

# 最小示例"#;

        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, Some("最小示例".to_string()));
        assert_eq!(fm.node_type, NodeType::Concept); // 默认值
        assert!(fm.tags.is_empty());
        assert!(fm.summary.is_none());
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# 标题\n\n内容";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_invalid_type() {
        let content = r#"---
title: 测试
type: UnknownType
---

测试内容"#;

        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.node_type, NodeType::Concept); // 未知类型使用默认值
    }

    #[test]
    fn test_has_frontmatter() {
        assert!(has_frontmatter("---\ntitle: test\n---"));
        assert!(!has_frontmatter("# 标题"));
    }

    #[test]
    fn test_extract_frontmatter() {
        let content = r#"---
title: 测试
summary: 描述
---

内容"#;

        let (yaml, remaining) = extract_frontmatter(content);
        assert!(yaml.is_some());
        assert_eq!(remaining.trim(), "内容");
    }

    #[test]
    fn test_frontmatter_to_yaml() {
        let mut fm = Frontmatter::new();
        fm.title = Some("测试".to_string());
        fm.node_type = NodeType::Question;
        fm.tags = vec!["tag1".to_string(), "tag2".to_string()];

        let yaml = fm.to_yaml();
        assert!(yaml.contains("title: \"测试\""));
        assert!(yaml.contains("type: Question"));
        assert!(yaml.contains("tags: [tag1, tag2]"));
    }
}
