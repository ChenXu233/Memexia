//! N-Quads 序列化模块
//!
//! 实现 RDF N-Quads 格式的导入导出功能
//!
//! N-Quads 格式规范: https://www.w3.org/TR/n-quads/

use super::{Edge, GraphStorage, RelationType};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// N-Quads 编码器
pub struct NQuadsEncoder<W: Write> {
    /// 写入器
    writer: W,
}

impl<W: Write> NQuadsEncoder<W> {
    /// 创建新的编码器
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// 写入一个三元组
    pub fn write_triple(
        &mut self,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> Result<()> {
        let subject_iri = escape_iri(subject);
        let predicate_iri = escape_iri(predicate);
        let object_value = if object.starts_with("http://")
            || object.starts_with("https://")
            || object.starts_with("urn:")
        {
            escape_iri(object)
        } else {
            escape_string(object)
        };

        writeln!(
            self.writer,
            "{} {} {} .",
            subject_iri, predicate_iri, object_value
        )
        .context("Failed to write N-Quads triple")?;

        Ok(())
    }
}

/// N-Quads 解码器
pub struct NQuadsDecoder<R: BufRead> {
    /// 读取器
    reader: R,
    /// 当前行号
    line_number: usize,
}

impl<R: BufRead> NQuadsDecoder<R> {
    /// 创建新的解码器
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line_number: 0,
        }
    }

    /// 读取下一个三元组
    pub fn read_triple(&mut self) -> Result<Option<(String, String, String)>> {
        let mut line = String::new();

        // 跳过空行和注释
        loop {
            self.line_number += 1;
            match self.reader.read_line(&mut line) {
                Ok(0) => return Ok(None), // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        line.clear();
                        continue;
                    }
                    // 解析 N-Quads 行
                    if let Some(triple) = parse_nquads_line(trimmed) {
                        return Ok(Some(triple));
                    }
                    return Err(anyhow::anyhow!(
                        "Failed to parse N-Quads at line {}: {}",
                        self.line_number,
                        trimmed
                    ));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to read line {}: {}", self.line_number, e))
                }
            }
        }
    }
}

/// 从 IRI 字符串转义为 N-Quads 格式
fn escape_iri(iri: &str) -> String {
    format!("<{}>", iri)
}

/// 从字符串转义为 N-Quads 文字格式
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');

    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(c),
        }
    }

    result.push('"');
    result
}

/// 解析 N-Quads 行
fn parse_nquads_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim_end_matches('.').trim();

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut in_angle: i32 = 0;

    for c in line.chars() {
        if c == '"' && in_angle == 0 {
            in_quote = !in_quote;
            current.push(c);
        } else if c == '<' && !in_quote {
            in_angle += 1;
            current.push(c);
        } else if c == '>' && !in_quote {
            in_angle -= 1;
            if in_angle > 0 {
                current.push(c);
            }
        } else if c == ' ' && !in_quote && in_angle == 0 {
            if !current.is_empty() {
                parts.push(current.trim().to_string());
                current = String::new();
            }
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }

    if parts.len() < 3 {
        return None;
    }

    // 去除 < > 包装
    let subject = unescape_iri(&parts[0]);
    let predicate = unescape_iri(&parts[1]);
    let mut object = parts[2].to_string();

    // 处理文字类型
    if object.starts_with('"') {
        object = object.trim_matches('"').to_string();
        object = unescape_string(&object);
    } else {
        // 如果是 IRI，去除 < >
        object = unescape_iri(&object);
    }

    Some((subject, predicate, object))
}

/// 从 N-Quads 格式还原 IRI
fn unescape_iri(s: &str) -> String {
    s.trim_start_matches('<').trim_end_matches('>').to_string()
}

/// 从 N-Quads 格式还原字符串
fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('u') => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            hex.push(h);
                        }
                    }
                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(cp) {
                            result.push(c);
                        }
                    }
                }
                Some(c) => result.push(c),
                None => break,
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// 导出存储为 N-Quads 格式
pub fn export_nquads(storage: &dyn GraphStorage, path: &Path) -> Result<File> {
    let file = File::create(path).with_context(|| format!("Failed to create {:?}", path))?;

    let mut encoder = NQuadsEncoder::new(&file);

    // 导出节点
    let nodes = storage.list_nodes()?;
    for node in nodes {
        let node_type_iri = match node.node_type {
            super::NodeType::Concept => "memexia:Concept",
            super::NodeType::Question => "memexia:Question",
            super::NodeType::Evidence => "memexia:Evidence",
            super::NodeType::Resource => "memexia:Resource",
            super::NodeType::Person => "memexia:Person",
            super::NodeType::Event => "memexia:Event",
            super::NodeType::Meta => "memexia:Meta",
        };

        encoder.write_triple(&node.id, "rdf:type", node_type_iri)?;
        encoder.write_triple(&node.id, "memexia:title", &node.title)?;

        if let Some(content) = &node.content {
            encoder.write_triple(&node.id, "memexia:content", content)?;
        }

        for tag in &node.tags {
            encoder.write_triple(&node.id, "memexia:tag", tag)?;
        }

        encoder.write_triple(
            &node.id,
            "memexia:createdAt",
            &node.created_at.to_rfc3339(),
        )?;
        encoder.write_triple(
            &node.id,
            "memexia:updatedAt",
            &node.updated_at.to_rfc3339(),
        )?;
    }

    // 导出边
    let edges = storage.list_edges()?;
    for edge in edges {
        // 使用 to_lowercase() 以匹配 parse_relation_type 的期望
        let predicate = format!("memexia:{}", edge.relation.to_string().to_lowercase());

        let mut object = edge.to.clone();
        if edge.strength != 1.0 || edge.description.is_some() {
            object = format!(
                "{}|{}:{}:{}",
                edge.to,
                edge.relation.to_string().to_lowercase(),
                edge.strength,
                edge.description.as_deref().unwrap_or("")
            );
        }

        encoder.write_triple(&edge.from, &predicate, &object)?;
    }

    Ok(file)
}

/// 从 N-Quads 格式导入
pub fn import_nquads(storage: &dyn GraphStorage, path: &Path) -> Result<()> {
    use super::Node;

    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let reader = BufReader::new(file);
    let mut decoder = NQuadsDecoder::new(reader);

    // 用于收集节点三元组: subject -> [(predicate, object)]
    let mut node_triples: HashMap<String, Vec<(String, String)>> = HashMap::new();
    // 用于收集边三元组
    let mut edge_triples: Vec<(String, String, String)> = Vec::new();

    // 解析所有三元组
    while let Some((subject, predicate, object)) = decoder.read_triple()? {
        // 跳过非 memexia 命名空间的边
        if predicate.starts_with("memexia:") && parse_relation_type(predicate.strip_prefix("memexia:").unwrap()).is_some() {
            edge_triples.push((subject, predicate, object));
        } else {
            // 收集节点相关的三元组
            node_triples
                .entry(subject.clone())
                .or_insert_with(Vec::new)
                .push((predicate, object));
        }
    }

    // 导入节点
    let mut added_nodes: HashSet<String> = HashSet::new();
    for (subject, triples) in &node_triples {
        if added_nodes.contains(subject.as_str()) {
            continue;
        }

        // 构建节点
        let mut node_type = super::NodeType::Concept;
        let mut title = subject.split(':').last().unwrap_or(subject).to_string();
        let mut content: Option<String> = None;
        let mut tags: Vec<String> = Vec::new();

        for (pred, obj) in triples {
            match pred.as_str() {
                "rdf:type" => {
                    node_type = match obj.strip_prefix("memexia:") {
                        Some("Concept") => super::NodeType::Concept,
                        Some("Question") => super::NodeType::Question,
                        Some("Evidence") => super::NodeType::Evidence,
                        Some("Resource") => super::NodeType::Resource,
                        Some("Person") => super::NodeType::Person,
                        Some("Event") => super::NodeType::Event,
                        Some("Meta") => super::NodeType::Meta,
                        _ => super::NodeType::Concept,
                    };
                }
                "memexia:title" => {
                    title = obj.clone();
                }
                "memexia:content" => {
                    content = Some(obj.clone());
                }
                "memexia:tag" => {
                    if !obj.is_empty() {
                        tags.push(obj.clone());
                    }
                }
                _ => {}
            }
        }

        // 创建并添加节点
        let node = Node::new(subject, node_type, title);
        storage.add_node(&node)?;
        added_nodes.insert(subject.clone());
    }

    // 导入边
    let mut added_edges: HashSet<String> = HashSet::new();
    for (subject, predicate, object) in edge_triples {
        let relation_str = predicate.strip_prefix("memexia:").unwrap();

        if let Some(relation) = parse_relation_type(relation_str) {
            let (to, _rel, strength, description) = parse_link_object(&object);

            let edge_id = format!("urn:memexia:edge:{}-{}", subject, to);

            if !added_edges.contains(&edge_id) {
                let mut edge = Edge::new(&edge_id, &subject, &to, relation);

                if strength != 1.0 {
                    edge.update_strength(strength);
                }
                if !description.is_empty() {
                    edge.update_description(description);
                }

                storage.add_edge(&edge)?;
                added_edges.insert(edge_id);
            }
        }
    }

    Ok(())
}

/// 解析关系类型字符串
pub(super) fn parse_relation_type(s: &str) -> Option<RelationType> {
    match s.to_lowercase().as_str() {
        "contains" => Some(RelationType::Contains),
        "partof" | "part_of" => Some(RelationType::PartOf),
        "instanceof" | "instance_of" => Some(RelationType::InstanceOf),
        "derivesfrom" | "derives_from" => Some(RelationType::DerivesFrom),
        "leadsto" | "leads_to" => Some(RelationType::LeadsTo),
        "supports" => Some(RelationType::Supports),
        "contradicts" => Some(RelationType::Contradicts),
        "refines" => Some(RelationType::Refines),
        "references" => Some(RelationType::References),
        "relatedto" | "related_to" => Some(RelationType::RelatedTo),
        "analogousto" | "analogous_to" => Some(RelationType::AnalogousTo),
        "precedes" => Some(RelationType::Precedes),
        "follows" => Some(RelationType::Follows),
        "simultaneous" => Some(RelationType::Simultaneous),
        _ => None,
    }
}

/// 解析链接对象字符串
fn parse_link_object(s: &str) -> (String, RelationType, f64, String) {
    let mut target = s.to_string();
    let mut relation = RelationType::RelatedTo;
    let mut strength = 1.0;
    let mut description = String::new();

    if let Some(idx) = s.find('|') {
        target = s[..idx].to_string();
        let after_pipe = &s[idx + 1..];

        if let Some(idx2) = after_pipe.find(':') {
            let rel_str = &after_pipe[..idx2];
            if let Some(r) = parse_relation_type(rel_str) {
                relation = r;
            }

            let after_colon = &after_pipe[idx2 + 1..];
            if let Some(idx3) = after_colon.find(':') {
                if let Ok(s) = after_colon[..idx3].parse::<f64>() {
                    strength = s;
                }
                description = after_colon[idx3 + 1..].to_string();
            } else if let Ok(s) = after_colon.parse::<f64>() {
                strength = s;
            }
        } else {
            if let Some(r) = parse_relation_type(after_pipe) {
                relation = r;
            }
        }
    }

    (target, relation, strength, description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "\"hello\"");
        assert_eq!(escape_string("hello world"), "\"hello world\"");
        assert_eq!(escape_string("hello\"world"), "\"hello\\\"world\"");
        assert_eq!(escape_string("hello\\world"), "\"hello\\\\world\"");
    }

    #[test]
    fn test_parse_nquads_line() {
        let line = "<http://example.org/s> <http://example.org/p> <http://example.org/o> .";
        let (s, p, o) = parse_nquads_line(line).unwrap();
        assert_eq!(s, "http://example.org/s");
        assert_eq!(p, "http://example.org/p");
        assert_eq!(o, "http://example.org/o");
    }

    #[test]
    fn test_parse_nquads_line_with_literal() {
        let line = r#"<http://example.org/s> <http://example.org/p> "hello world" ."#;
        let (s, p, o) = parse_nquads_line(line).unwrap();
        assert_eq!(s, "http://example.org/s");
        assert_eq!(p, "http://example.org/p");
        assert_eq!(o, "hello world");
    }

    #[test]
    fn test_nquads_export_import_roundtrip() {
        use tempfile::TempDir;
        use super::super::{Node, NodeType, Edge, RelationType, EdgeDirection};

        let temp_dir = TempDir::new().unwrap();
        let storage = super::super::Storage::init(temp_dir.path()).unwrap();

        // 添加节点
        let node1 = Node::new("urn:memexia:file:a.md", NodeType::Concept, "Node A");
        let node2 = Node::new("urn:memexia:file:b.md", NodeType::Question, "Node B");
        storage.graph().add_node(&node1).unwrap();
        storage.graph().add_node(&node2).unwrap();

        // 添加边
        let edge = Edge::new(
            "urn:memexia:edge:a-b",
            "urn:memexia:file:a.md",
            "urn:memexia:file:b.md",
            RelationType::Contradicts,
        );
        storage.graph().add_edge(&edge).unwrap();

        // 导出到 N-Quads
        let nq_path = temp_dir.path().join("export.nq");
        export_nquads(storage.graph(), &nq_path).unwrap();

        // 验证导出文件存在且有内容
        assert!(nq_path.exists());
        let content = std::fs::read_to_string(&nq_path).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("memexia:Concept"));
        assert!(content.contains("memexia:title"));
        assert!(content.contains("contradicts"));

        // 创建新存储并导入
        let temp_dir2 = TempDir::new().unwrap();
        let storage2 = super::super::Storage::init(temp_dir2.path()).unwrap();

        // 导入 N-Quads
        import_nquads(storage2.graph(), &nq_path).unwrap();

        // 验证节点导入成功
        let nodes = storage2.graph().list_nodes().unwrap();
        assert_eq!(nodes.len(), 2);

        // 验证边导入成功
        let edges = storage2.graph().list_edges().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, RelationType::Contradicts);
    }
}
