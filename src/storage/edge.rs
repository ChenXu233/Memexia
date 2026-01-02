//! 边（关系）数据模型定义
//!
//! 定义 Memexia 知识图谱中节点之间的连接关系

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 关系类型枚举
///
/// 符合项目文档 3.3.1 预定义的关系类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum RelationType {
    /// 包含关系（整体-部分）
    #[serde(rename = "Contains")]
    Contains,
    /// 部分关系（部分-整体）
    #[serde(rename = "PartOf")]
    PartOf,
    /// 实例关系
    #[serde(rename = "InstanceOf")]
    InstanceOf,
    /// 推导自（逻辑推导）
    #[serde(rename = "DerivesFrom")]
    DerivesFrom,
    /// 导向（因果或时间顺序）
    #[serde(rename = "LeadsTo")]
    LeadsTo,
    /// 支持（证据支持）
    #[serde(rename = "Supports")]
    Supports,
    /// 矛盾（逻辑对立）
    #[serde(rename = "Contradicts")]
    Contradicts,
    /// 精炼（更精确表述）
    #[serde(rename = "Refines")]
    Refines,
    /// 引用（外部来源）
    #[serde(rename = "References")]
    References,
    /// 相关（一般性关联）
    #[serde(rename = "RelatedTo")]
    RelatedTo,
    /// 类比（相似但不同领域）
    #[serde(rename = "AnalogousTo")]
    AnalogousTo,
    /// 先于
    #[serde(rename = "Precedes")]
    Precedes,
    /// 后于
    #[serde(rename = "Follows")]
    Follows,
    /// 同时
    #[serde(rename = "Simultaneous")]
    Simultaneous,
}

impl Default for RelationType {
    fn default() -> Self {
        RelationType::RelatedTo
    }
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationType::Contains => write!(f, "Contains"),
            RelationType::PartOf => write!(f, "PartOf"),
            RelationType::InstanceOf => write!(f, "InstanceOf"),
            RelationType::DerivesFrom => write!(f, "DerivesFrom"),
            RelationType::LeadsTo => write!(f, "LeadsTo"),
            RelationType::Supports => write!(f, "Supports"),
            RelationType::Contradicts => write!(f, "Contradicts"),
            RelationType::Refines => write!(f, "Refines"),
            RelationType::References => write!(f, "References"),
            RelationType::RelatedTo => write!(f, "RelatedTo"),
            RelationType::AnalogousTo => write!(f, "AnalogousTo"),
            RelationType::Precedes => write!(f, "Precedes"),
            RelationType::Follows => write!(f, "Follows"),
            RelationType::Simultaneous => write!(f, "Simultaneous"),
        }
    }
}

/// 关系来源类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeSource {
    /// 显式创建，用户手动指定
    #[serde(rename = "explicit")]
    Explicit,
    /// AI 推荐，系统分析后建议
    #[serde(rename = "ai")]
    AI,
    /// 推导得出，基于现有关系推断
    #[serde(rename = "derived")]
    Derived,
}

impl Default for EdgeSource {
    fn default() -> Self {
        EdgeSource::Explicit
    }
}

impl fmt::Display for EdgeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdgeSource::Explicit => write!(f, "explicit"),
            EdgeSource::AI => write!(f, "ai"),
            EdgeSource::Derived => write!(f, "derived"),
        }
    }
}

/// 边结构体
///
/// 表示知识图谱中两个节点之间的连接关系
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// 边的唯一标识符
    pub id: String,

    /// 源节点 ID（URN 格式）
    pub from: String,

    /// 目标节点 ID（URN 格式）
    pub to: String,

    /// 关系类型
    pub relation: RelationType,

    /// 关系强度 (0.0 - 1.0)
    ///
    /// 符合项目文档 3.3.2 定义
    #[serde(default = "default_strength")]
    pub strength: f64,

    /// 置信度
    ///
    /// AI 生成时的置信水平
    #[serde(default = "default_confidence")]
    pub confidence: f64,

    /// 关系描述
    ///
    /// 自由文本描述关系的性质
    #[serde(default)]
    pub description: Option<String>,

    /// 来源类型
    pub source: EdgeSource,

    /// 创建时间
    pub created_at: DateTime<Utc>,
}

fn default_strength() -> f64 {
    1.0
}

fn default_confidence() -> f64 {
    1.0
}

impl Edge {
    /// 创建新边
    ///
    /// # Arguments
    ///
    /// * `id` - 边的唯一标识符
    /// * `from` - 源节点 ID
    /// * `to` - 目标节点 ID
    /// * `relation` - 关系类型
    ///
    /// # Returns
    ///
    /// 带有默认值的 `Edge` 实例
    pub fn new(
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        relation: RelationType,
    ) -> Self {
        Self {
            id: id.into(),
            from: from.into(),
            to: to.into(),
            relation,
            strength: 1.0,
            confidence: 1.0,
            description: None,
            source: EdgeSource::Explicit,
            created_at: Utc::now(),
        }
    }

    /// 创建显式链接边
    ///
    /// 解析 `[[目标|类型]]` 语法的链接
    ///
    /// # Arguments
    ///
    /// * `from_node_id` - 源节点 ID
    /// * `link_text` - 链接文本，如 `目标|类型` 或 `目标`
    ///
    /// # Returns
    ///
    /// 解析后的边，如果类型无法解析则使用默认关系
    pub fn from_link(from_node_id: &str, link_text: &str) -> Self {
        let (target, relation_str, strength, description) = Self::parse_link_text(link_text);

        let id = format!("urn:memexia:edge:{}-{}", from_node_id, target);

        let relation = Self::parse_relation(&relation_str).unwrap_or_default();

        Self {
            id,
            from: from_node_id.to_string(),
            to: format!("urn:memexia:file:{}", target),
            relation,
            strength,
            confidence: 1.0,
            description: if !description.is_empty() {
                Some(description.to_string())
            } else {
                None
            },
            source: EdgeSource::Explicit,
            created_at: Utc::now(),
        }
    }

    /// 解析链接文本
    ///
    /// 格式: `目标|类型:强度:描述` 或 `目标|类型` 或 `目标`
    fn parse_link_text(text: &str) -> (&str, &str, f64, &str) {
        let mut target = text;
        let mut relation_str = "";
        let mut strength = 1.0;
        let mut description = "";

        if let Some(idx) = text.find('|') {
            target = &text[..idx];
            let after_pipe = &text[idx + 1..];

            if let Some(idx2) = after_pipe.find(':') {
                relation_str = &after_pipe[..idx2];
                let after_colon = &after_pipe[idx2 + 1..];

                if let Some(idx3) = after_colon.find(':') {
                    // 有强度和描述
                    if let Ok(s) = after_colon[..idx3].parse::<f64>() {
                        strength = s;
                    }
                    description = &after_colon[idx3 + 1..];
                } else {
                    // 只有强度
                    if let Ok(s) = after_colon.parse::<f64>() {
                        strength = s;
                    }
                }
            } else {
                // 只有关系类型
                relation_str = after_pipe;
            }
        }

        (target, relation_str, strength, description)
    }

    /// 解析关系类型字符串
    ///
    /// # Arguments
    ///
    /// * `relation_str` - 关系类型字符串，如 "矛盾"、"Contradicts" 等
    ///
    /// # Returns
    ///
    /// 对应的 `RelationType`，无法解析时返回 `None`
    fn parse_relation(relation_str: &str) -> Option<RelationType> {
        let relation_lower = relation_str.to_lowercase();

        match relation_lower.as_str() {
            "矛盾" | "contradicts" => Some(RelationType::Contradicts),
            "属于" | "partof" | "part_of" => Some(RelationType::PartOf),
            "包含" | "contains" => Some(RelationType::Contains),
            "实例" | "instanceof" | "instance_of" => Some(RelationType::InstanceOf),
            "推导" | "derivesfrom" | "derives_from" => Some(RelationType::DerivesFrom),
            "导向" | "leadsto" | "leads_to" => Some(RelationType::LeadsTo),
            "支持" | "supports" => Some(RelationType::Supports),
            "精炼" | "refines" => Some(RelationType::Refines),
            "引用" | "references" => Some(RelationType::References),
            "相关" | "relatedto" | "related_to" => Some(RelationType::RelatedTo),
            "类比" | "analogousto" | "analogous_to" => Some(RelationType::AnalogousTo),
            "先于" | "precedes" => Some(RelationType::Precedes),
            "后于" | "follows" => Some(RelationType::Follows),
            "同时" | "simultaneous" => Some(RelationType::Simultaneous),
            _ => None,
        }
    }

    /// 更新关系强度
    pub fn update_strength(&mut self, strength: f64) {
        self.strength = strength.clamp(0.0, 1.0);
    }

    /// 更新描述
    pub fn update_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
    }
}

/// 边的查询过滤器
#[derive(Debug, Default)]
pub struct EdgeFilter {
    /// 源节点 ID（可选）
    pub from: Option<String>,
    /// 目标节点 ID（可选）
    pub to: Option<String>,
    /// 关系类型（可选）
    pub relation: Option<RelationType>,
    /// 最小强度阈值
    pub min_strength: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_new() {
        let edge = Edge::new(
            "edge-1",
            "urn:memexia:file:notes/a.md",
            "urn:memexia:file:notes/b.md",
            RelationType::Contradicts,
        );

        assert_eq!(edge.id, "edge-1");
        assert_eq!(edge.from, "urn:memexia:file:notes/a.md");
        assert_eq!(edge.to, "urn:memexia:file:notes/b.md");
        assert_eq!(edge.relation, RelationType::Contradicts);
        assert_eq!(edge.strength, 1.0);
    }

    #[test]
    fn test_edge_from_link_basic() {
        let edge = Edge::from_link("urn:memexia:file:notes/free_will.md", "决定论");

        assert_eq!(edge.from, "urn:memexia:file:notes/free_will.md");
        assert_eq!(edge.to, "urn:memexia:file:决定论");
        assert_eq!(edge.relation, RelationType::RelatedTo);
        assert_eq!(edge.strength, 1.0);
    }

    #[test]
    fn test_edge_from_link_with_type() {
        let edge = Edge::from_link("urn:memexia:file:notes/free_will.md", "决定论|矛盾");

        assert_eq!(edge.to, "urn:memexia:file:决定论");
        assert_eq!(edge.relation, RelationType::Contradicts);
    }

    #[test]
    fn test_edge_from_link_with_strength() {
        let edge = Edge::from_link("urn:memexia:file:notes/free_will.md", "意识|属于:0.95");

        assert_eq!(edge.relation, RelationType::PartOf);
        assert!((edge.strength - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_edge_from_link_with_full_info() {
        let edge = Edge::from_link(
            "urn:memexia:file:notes/free_will.md",
            "意识|属于:0.95:意识是心灵哲学核心",
        );

        assert_eq!(edge.relation, RelationType::PartOf);
        assert!((edge.strength - 0.95).abs() < 0.001);
        assert_eq!(edge.description, Some("意识是心灵哲学核心".to_string()));
    }

    #[test]
    fn test_edge_parse_relation_chinese() {
        assert_eq!(
            Edge::parse_relation("矛盾"),
            Some(RelationType::Contradicts)
        );
        assert_eq!(Edge::parse_relation("属于"), Some(RelationType::PartOf));
        assert_eq!(Edge::parse_relation("支持"), Some(RelationType::Supports));
    }

    #[test]
    fn test_edge_parse_relation_english() {
        assert_eq!(
            Edge::parse_relation("Contradicts"),
            Some(RelationType::Contradicts)
        );
        assert_eq!(Edge::parse_relation("PartOf"), Some(RelationType::PartOf));
    }

    #[test]
    fn test_edge_parse_relation_unknown() {
        assert_eq!(Edge::parse_relation("unknown"), None);
    }

    #[test]
    fn test_edge_serde_roundtrip() {
        let mut edge = Edge::new("test", "from", "to", RelationType::Supports);
        edge.update_strength(0.8);
        edge.update_description("Test description");

        let serialized = serde_json::to_string(&edge).unwrap();
        let deserialized: Edge = serde_json::from_str(&serialized).unwrap();

        assert_eq!(edge, deserialized);
    }
}
