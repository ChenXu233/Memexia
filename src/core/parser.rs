//! Markdown 解析器模块
//!
//! 解析 Markdown 文件，提取 frontmatter、wiki 链接、标签等
//!
//! ## 功能
//!
//! - 解析 YAML frontmatter
//! - 解析 `[[wiki links]]` 格式链接
//! - 提取 `#tags` 标签
//! - 提取标题（从 frontmatter 或 H1）
//! - 生成纯文本内容（去链接）

pub mod frontmatter;
pub mod wiki_link;

pub use frontmatter::{parse_frontmatter, extract_frontmatter, has_frontmatter, Frontmatter};
pub use wiki_link::{parse_wiki_links, remove_wiki_links, replace_wiki_links_with_text, WikiLink};

use crate::storage::{Node, NodeType};
use std::collections::HashSet;

/// 解析后的文档结构
///
/// 包含从 Markdown 文件解析出的所有信息
#[derive(Debug, Clone)]
pub struct ParsedDoc {
    /// Frontmatter 元数据
    pub frontmatter: Option<Frontmatter>,
    /// Wiki 链接列表
    pub wiki_links: Vec<WikiLink>,
    /// 标签列表（从 #tag 提取）
    pub tags: Vec<String>,
    /// 纯文本内容（去链接）
    pub content: String,
    /// 提取的标题（优先使用 frontmatter.title，其次 H1）
    pub title: Option<String>,
    /// 文件名（用于生成节点 ID）
    pub file_name: String,
}

impl Default for ParsedDoc {
    fn default() -> Self {
        Self {
            frontmatter: None,
            wiki_links: Vec::new(),
            tags: Vec::new(),
            content: String::new(),
            title: None,
            file_name: String::new(),
        }
    }
}

impl ParsedDoc {
    /// 创建新的 ParsedDoc
    pub fn new(file_name: &str) -> Self {
        Self {
            file_name: file_name.to_string(),
            ..Default::default()
        }
    }

    /// 获取节点标题
    ///
    /// 优先级：frontmatter.title > H1 > 文件名
    pub fn get_title(&self) -> String {
        if let Some(ref fm) = self.frontmatter {
            if let Some(ref title) = fm.title {
                return title.clone();
            }
        }
        if let Some(ref title) = self.title {
            return title.clone();
        }
        // 从文件名提取
        self.file_name.split('.').next().unwrap_or(&self.file_name).to_string()
    }

    /// 获取节点类型
    pub fn get_node_type(&self) -> NodeType {
        self.frontmatter
            .as_ref()
            .map(|f| f.node_type.clone())
            .unwrap_or(NodeType::Concept)
    }

    /// 获取所有标签
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags: HashSet<String> = HashSet::new();

        // frontmatter 中的标签
        if let Some(ref fm) = self.frontmatter {
            for tag in &fm.tags {
                tags.insert(tag.clone());
            }
        }

        // 内容中的 #tag
        for tag in &self.tags {
            tags.insert(tag.clone());
        }

        tags.into_iter().collect()
    }

    /// 转换为 Node
    pub fn to_node(&self) -> Node {
        let node_id = Self::file_name_to_id(&self.file_name);
        let mut node = Node::new(
            &node_id,
            self.get_node_type(),
            self.get_title().to_string(),
        );

        // 设置内容（移除 wiki 链接后的纯内容）
        let content = remove_wiki_links(&self.content);
        if !content.trim().is_empty() {
            node.content = Some(content);
        }

        // 设置标签
        for tag in self.get_all_tags() {
            node.add_tag(tag);
        }

        // 设置摘要（如果有）
        if let Some(ref fm) = self.frontmatter {
            if let Some(ref summary) = fm.summary {
                if node.content.is_none() || node.content.as_ref().map(|c| c.len()).unwrap_or(0) > summary.len() {
                    // 如果摘要比完整内容短，使用摘要
                    if summary.len() < 200 {
                        node.content = Some(summary.clone());
                    }
                }
            }
        }

        node
    }

    /// 将文件路径转换为 URN 格式 ID（带 URL 编码）
    fn file_name_to_id(file_name: &str) -> String {
        let path_str = file_name.replace('\\', "/");
        let encoded = encode_iri_component(&path_str);
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

/// 解析 Markdown 内容
///
/// # Arguments
///
/// * `content` - Markdown 文件内容
/// * `file_name` - 文件名（用于生成节点 ID）
///
/// # Returns
///
/// 解析后的 ParsedDoc
pub fn parse_markdown(content: &str, file_name: &str) -> ParsedDoc {
    let mut doc = ParsedDoc::new(file_name);

    // 1. 提取并解析 frontmatter
    let (fm_yaml, remaining) = extract_frontmatter(content);
    if let Some(yaml_str) = fm_yaml {
        doc.frontmatter = parse_frontmatter(&format!("---\n{}---", yaml_str));
    }

    // 2. 解析 wiki 链接
    doc.wiki_links = parse_wiki_links(&remaining);

    // 3. 提取 #tag 标签
    doc.tags = extract_tags(&remaining);

    // 4. 提取标题（如果 frontmatter 没有）
    if doc.frontmatter.is_none() || doc.frontmatter.as_ref().unwrap().title.is_none() {
        doc.title = extract_title(&remaining);
    }

    // 5. 设置纯内容（移除 wiki 链接，保留其他内容）
    doc.content = remove_wiki_links(&remaining);

    doc
}

/// 从内容中提取标签
///
/// 匹配 `#tag` 格式的标签
fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let re = regex::Regex::new(r"(?m)(?:^|\s)#(\w[\w-]*)").unwrap();

    for cap in re.captures_iter(content) {
        if let Some(tag) = cap.get(1) {
            let tag_str = tag.as_str().to_string();
            if !tags.contains(&tag_str) {
                tags.push(tag_str);
            }
        }
    }

    tags
}

/// 从内容中提取标题
///
/// 匹配第一个 H1 (`# 标题`)
fn extract_title(content: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?m)^#\s+(.+)$").unwrap();
    re.captures(content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().trim().to_string())
}

/// 快速解析（不保留内容）
///
/// 仅解析 frontmatter 和链接，用于索引更新检测
pub fn quick_parse(content: &str) -> (Option<Frontmatter>, Vec<WikiLink>, Vec<String>) {
    let (fm_yaml, remaining) = extract_frontmatter(content);
    let frontmatter = fm_yaml.and_then(|yaml| parse_frontmatter(&format!("---\n{}---", &yaml)));
    let links = parse_wiki_links(&remaining);
    let tags = extract_tags(&remaining);

    (frontmatter, links, tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_full() {
        let content = r#"---
title: 自由意志
type: Concept
tags: [哲学, 心灵]
summary: 关于自由意志的讨论
---

# 自由意志

自由意志是哲学中的重要概念 [[决定论|Contradicts]]。

## 主要观点

- 观点1 [[支持观点|Supports:0.9:强支持]]
- 观点2

#标签1 #标签2"#;

        let doc = parse_markdown(content, "notes/free_will.md");

        assert_eq!(doc.file_name, "notes/free_will.md");
        assert!(doc.frontmatter.is_some());
        assert_eq!(doc.frontmatter.as_ref().unwrap().title, Some("自由意志".to_string()));
        assert_eq!(doc.wiki_links.len(), 2);
        assert!(doc.content.contains("哲学中的重要概念"));
    }

    #[test]
    fn test_parse_markdown_no_frontmatter() {
        let content = r##"# 标题

内容 [[目标|RelatedTo]] #tag1"##;

        let doc = parse_markdown(content, "test.md");

        assert!(doc.frontmatter.is_none());
        assert_eq!(doc.title, Some("标题".to_string()));
        assert_eq!(doc.wiki_links.len(), 1);
        assert_eq!(doc.tags, vec!["tag1"]);
    }

    #[test]
    fn test_parsed_doc_to_node() {
        let content = r#"---
title: 测试节点
type: Question
tags: [test]
---

测试内容 [[目标|Supports]]"#;

        let doc = parse_markdown(content, "test.md");
        let node = doc.to_node();

        assert_eq!(node.id, "urn:memexia:file:test.md");
        assert_eq!(node.node_type, NodeType::Question);
        assert_eq!(node.title, "测试节点");
        assert!(node.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_get_all_tags() {
        let content = r#"---
tags: [fm-tag]
---

# 标题

内容 #content-tag #another"#;

        let doc = parse_markdown(content, "test.md");
        let all_tags = doc.get_all_tags();

        assert!(all_tags.contains(&"fm-tag".to_string()));
        assert!(all_tags.contains(&"content-tag".to_string()));
        assert!(all_tags.contains(&"another".to_string()));
    }

    #[test]
    fn test_quick_parse() {
        let content = r#"---
title: 快速解析
type: Evidence
---

内容 [[目标|Contradicts]] #tag"#;

        let (fm, links, tags) = quick_parse(content);

        assert!(fm.is_some());
        assert_eq!(fm.unwrap().title, Some("快速解析".to_string()));
        assert_eq!(links.len(), 1);
        assert_eq!(tags, vec!["tag"]);
    }

    #[test]
    fn test_extract_tags_duplicates() {
        let content = "#tag1 #tag2 #tag1 #tag3";
        let tags = extract_tags(content);

        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
        assert!(tags.contains(&"tag3".to_string()));
    }
}
