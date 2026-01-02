//! Wiki 链接解析模块
//!
//! 解析 Markdown 中的 `[[目标|类型:strength:描述]]` 格式链接
//!
//! ## 链接格式
//!
//! ```markdown
//! [[目标]]                                    # 默认 RelatedTo
//! [[目标|关系]]                               # 指定关系类型
//! [[目标|关系:0.85]]                          # 指定关系 + 强度
//! [[目标|关系:0.85:描述]]                     # 完整格式
//! ```

use regex::Regex;
use crate::storage::{Edge, RelationType};

/// Wiki 链接结构
///
/// 表示一个解析后的 wiki 链接
#[derive(Debug, Clone, PartialEq)]
pub struct WikiLink {
    /// 目标节点 ID
    pub target: String,
    /// 关系类型
    pub relation: RelationType,
    /// 关系强度 (0.0 - 1.0)
    pub strength: f64,
    /// 描述文本
    pub description: String,
}

impl Default for WikiLink {
    fn default() -> Self {
        Self {
            target: String::new(),
            relation: RelationType::RelatedTo,
            strength: 1.0,
            description: String::new(),
        }
    }
}

impl WikiLink {
    /// 创建新的 WikiLink
    pub fn new(target: String, relation: RelationType, strength: f64, description: String) -> Self {
        Self {
            target,
            relation,
            strength,
            description,
        }
    }

    /// 转换为 Edge
    pub fn to_edge(&self, from: &str) -> Edge {
        let encoded_target = encode_iri_component(&self.target);
        let target_urn = format!("urn:memexia:file:{}", encoded_target);
        let edge_id = format!("urn:memexia:edge:{}-{}", from, encoded_target);
        let mut edge = Edge::new(&edge_id, from, &target_urn, self.relation);
        if self.strength != 1.0 {
            edge.update_strength(self.strength);
        }
        if !self.description.is_empty() {
            edge.update_description(&self.description);
        }
        edge
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

/// 解析 wiki 链接
///
/// # Arguments
///
/// * `content` - Markdown 内容
///
/// # Returns
///
/// 解析出的 WikiLink 列表
pub fn parse_wiki_links(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();

    // 正则表达式匹配 [[目标|关系:strength:描述]] 或 [[目标|关系]]
    // 格式: [[目标]] 或 [[目标|关系]] 或 [[目标|关系:strength]] 或 [[目标|关系:strength:描述]]
    let re = Regex::new(r"\[\[([^]\|]+(?:\|[^]\|]+)?)\]\]").unwrap();

    for cap in re.captures_iter(content) {
        if let Some(link_str) = cap.get(1) {
            if let Some(link) = parse_link_str(link_str.as_str()) {
                links.push(link);
            }
        }
    }

    links
}

/// 解析单个链接字符串
///
/// # Arguments
///
/// * `link_str` - 链接字符串，如 "目标" 或 "目标|关系:0.8:描述"
///
/// # Returns
///
/// 解析后的 WikiLink，解析失败返回 None
fn parse_link_str(link_str: &str) -> Option<WikiLink> {
    let parts: Vec<&str> = link_str.split('|').collect();

    let target = parts[0].trim().to_string();
    if target.is_empty() {
        return None;
    }

    // 默认值
    let mut relation = RelationType::RelatedTo;
    let mut strength = 1.0;
    let mut description = String::new();

    // 解析第二部分（关系和强度）
    if parts.len() > 1 && !parts[1].is_empty() {
        let after_pipe = parts[1];

        // 检查是否有强度（包含 :）
        if let Some(colon_pos) = after_pipe.find(':') {
            // 格式: 关系:strength:描述
            let rel_str = &after_pipe[..colon_pos];
            relation = parse_relation(rel_str);

            let after_colon = &after_pipe[colon_pos + 1..];

            // 检查是否有描述（再找到一个 :）
            if let Some(desc_colon) = after_colon.find(':') {
                // 格式: strength:描述
                if let Ok(s) = after_colon[..desc_colon].parse::<f64>() {
                    strength = s.clamp(0.0, 1.0);
                }
                description = after_colon[desc_colon + 1..].to_string();
            } else {
                // 格式: strength
                if let Ok(s) = after_colon.parse::<f64>() {
                    strength = s.clamp(0.0, 1.0);
                }
            }
        } else {
            // 格式: 关系
            relation = parse_relation(after_pipe);
        }
    }

    Some(WikiLink::new(target, relation, strength, description))
}

/// 解析关系类型字符串
fn parse_relation(s: &str) -> RelationType {
    match s.to_lowercase().replace('_', "").as_str() {
        "contains" => RelationType::Contains,
        "partof" => RelationType::PartOf,
        "instanceof" => RelationType::InstanceOf,
        "derivesfrom" => RelationType::DerivesFrom,
        "leadsto" => RelationType::LeadsTo,
        "supports" => RelationType::Supports,
        "contradicts" => RelationType::Contradicts,
        "refines" => RelationType::Refines,
        "references" => RelationType::References,
        "relatedto" => RelationType::RelatedTo,
        "analogousto" => RelationType::AnalogousTo,
        "precedes" => RelationType::Precedes,
        "follows" => RelationType::Follows,
        "simultaneous" => RelationType::Simultaneous,
        _ => RelationType::RelatedTo, // 默认
    }
}

/// 从内容中移除所有 wiki 链接标记
///
/// # Arguments
///
/// * `content` - 原始 Markdown 内容
///
/// # Returns
///
/// 移除链接标记后的纯内容
pub fn remove_wiki_links(content: &str) -> String {
    let re = Regex::new(r"\[\[[^\]]+\]\]").unwrap();
    re.replace_all(content, "").to_string()
}

/// 从内容中移除 wiki 链接并保留显示文本
///
/// 例如: `[[目标|显示文本]]` 替换为 `显示文本`
pub fn replace_wiki_links_with_text(content: &str) -> String {
    // 匹配 [[目标|显示文本]] 或 [[目标]]
    let re = Regex::new(r"\[\[([^]\|]+)\|([^]\|]+)\]\]").unwrap();
    let result = re.replace_all(content, "$2");

    // 移除剩余的 [[目标]]
    let re2 = Regex::new(r"\[\[[^\]]+\]\]").unwrap();
    re2.replace_all(&result, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_link_basic() {
        let link = parse_link_str("目标").unwrap();
        assert_eq!(link.target, "目标");
        assert_eq!(link.relation, RelationType::RelatedTo);
        assert_eq!(link.strength, 1.0);
        assert!(link.description.is_empty());
    }

    #[test]
    fn test_parse_link_with_relation() {
        let link = parse_link_str("目标|Contradicts").unwrap();
        assert_eq!(link.target, "目标");
        assert_eq!(link.relation, RelationType::Contradicts);
    }

    #[test]
    fn test_parse_link_with_strength() {
        let link = parse_link_str("目标|Supports:0.85").unwrap();
        assert_eq!(link.target, "目标");
        assert_eq!(link.relation, RelationType::Supports);
        assert_eq!(link.strength, 0.85);
    }

    #[test]
    fn test_parse_link_full() {
        let link = parse_link_str("目标|Refines:0.9:更精确的描述").unwrap();
        assert_eq!(link.target, "目标");
        assert_eq!(link.relation, RelationType::Refines);
        assert_eq!(link.strength, 0.9);
        assert_eq!(link.description, "更精确的描述");
    }

    #[test]
    fn test_parse_link_with_underscore() {
        let link = parse_link_str("目标|Part_Of").unwrap();
        assert_eq!(link.relation, RelationType::PartOf);
    }

    #[test]
    fn test_parse_wiki_links_multiple() {
        let content = r#"这是一个文档，包含 [[目标1]] 和 [[目标2|Contradicts]]。

还有 [[目标3|Supports:0.8:描述]] 在这里。"#;

        let links = parse_wiki_links(content);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "目标1");
        assert_eq!(links[1].target, "目标2");
        assert_eq!(links[2].target, "目标3");
    }

    #[test]
    fn test_remove_wiki_links() {
        let content = "文本 [[目标1]] 更多文本 [[目标2]] 结束";
        let cleaned = remove_wiki_links(content);
        assert_eq!(cleaned, "文本  更多文本  结束");
    }

    #[test]
    fn test_replace_wiki_links_with_text() {
        let content = "文本 [[目标|显示]] 更多 [[目标2]] 结束";
        let replaced = replace_wiki_links_with_text(content);
        assert_eq!(replaced, "文本 显示 更多  结束");
    }

    #[test]
    fn test_wiki_link_to_edge() {
        let link = WikiLink::new(
            "目标节点".to_string(),
            RelationType::Supports,
            0.85,
            "支持证据".to_string(),
        );
        let edge = link.to_edge("源节点");

        assert_eq!(edge.from, "源节点");
        // 目标节点 ID 应该被编码，且是完整的 URN 格式
        assert_eq!(edge.to, "urn:memexia:file:%E7%9B%AE%E6%A0%87%E8%8A%82%E7%82%B9");
        assert_eq!(edge.relation, RelationType::Supports);
    }

    #[test]
    fn test_parse_relation_case_insensitive() {
        assert_eq!(parse_relation("contradicts"), RelationType::Contradicts);
        assert_eq!(parse_relation("CONTRADICTS"), RelationType::Contradicts);
        assert_eq!(parse_relation("Contradicts"), RelationType::Contradicts);
    }

    #[test]
    fn test_parse_relation_unknown() {
        let link = parse_link_str("目标|UnknownRelation").unwrap();
        assert_eq!(link.relation, RelationType::RelatedTo); // 默认值
    }

    #[test]
    fn test_strength_clamping() {
        let link1 = parse_link_str("目标|Supports:1.5").unwrap();
        assert_eq!(link1.strength, 1.0); // 超过1.0 被钳制

        let link2 = parse_link_str("目标|Supports:-0.5").unwrap();
        assert_eq!(link2.strength, 0.0); // 低于0.0 被钳制
    }
}
