//! Oxigraph 图数据库实现
//!
//! 基于 Oxigraph 库实现 `GraphStorage` trait
//! 使用 Oxigraph 0.5.3 的 Store API

use super::{Edge, EdgeDirection, GraphStats, Node, NodeType, RelationType};
use crate::storage::graph::GraphStorage;
use crate::storage::graph::QueryResult;
use crate::storage::EdgeFilter;
use anyhow::{bail, Context, Result};
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Term, NamedOrBlankNode};
use oxigraph::store::Store;
use std::path::Path;

/// Oxigraph 存储实现
#[derive(Clone)]
pub struct OxigraphStorage {
    /// Oxigraph 存储实例
    store: Store,
}

impl std::fmt::Debug for OxigraphStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OxigraphStorage").finish_non_exhaustive()
    }
}

impl OxigraphStorage {
    /// 打开已有存储
    pub fn open(path: &Path) -> Result<Self> {
        let store = Store::open(path)
            .with_context(|| format!("Failed to open Oxigraph store at {:?}", path))?;

        Ok(Self { store })
    }

    /// 创建新存储
    pub fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let store = Store::open(path)
            .with_context(|| format!("Failed to create Oxigraph store at {:?}", path))?;

        Ok(Self { store })
    }

    /// 清理 IRI 字符串，去掉尖括号
    fn clean_iri(iri: &str) -> String {
        iri.trim_start_matches('<').trim_end_matches('>').to_string()
    }
}

impl GraphStorage for OxigraphStorage {
    fn add_node(&self, node: &Node) -> Result<()> {
        let subject = NamedOrBlankNode::from(NamedNode::new(&node.id)?);
        let graph_name = GraphName::DefaultGraph;

        // 添加类型三元组
        let type_pred = NamedNode::new("rdf:type")?;
        let type_obj = match node.node_type {
            NodeType::Concept => Term::from(NamedNode::new("memexia:Concept")?),
            NodeType::Question => Term::from(NamedNode::new("memexia:Question")?),
            NodeType::Evidence => Term::from(NamedNode::new("memexia:Evidence")?),
            NodeType::Resource => Term::from(NamedNode::new("memexia:Resource")?),
            NodeType::Person => Term::from(NamedNode::new("memexia:Person")?),
            NodeType::Event => Term::from(NamedNode::new("memexia:Event")?),
            NodeType::Meta => Term::from(NamedNode::new("memexia:Meta")?),
        };

        let quad = Quad::new(subject.clone(), type_pred, type_obj, graph_name.clone());
        self.store.insert(&quad)?;

        // 添加标题
        if !node.title.is_empty() {
            let title_pred = NamedNode::new("memexia:title")?;
            let title_obj = Term::from(Literal::new_simple_literal(&node.title));
            let quad = Quad::new(subject.clone(), title_pred, title_obj, graph_name.clone());
            self.store.insert(&quad)?;
        }

        // 添加内容
        if let Some(content) = &node.content {
            let content_pred = NamedNode::new("memexia:content")?;
            let content_obj = Term::from(Literal::new_simple_literal(content));
            let quad = Quad::new(subject.clone(), content_pred, content_obj, graph_name.clone());
            self.store.insert(&quad)?;
        }

        // 添加标签
        for tag in &node.tags {
            let tag_pred = NamedNode::new("memexia:tag")?;
            let tag_obj = Term::from(Literal::new_simple_literal(tag));
            let quad = Quad::new(subject.clone(), tag_pred, tag_obj, graph_name.clone());
            self.store.insert(&quad)?;
        }

        // 添加时间戳
        let created_pred = NamedNode::new("memexia:createdAt")?;
        let created_obj = Term::from(Literal::new_simple_literal(node.created_at.to_rfc3339()));
        let quad = Quad::new(subject.clone(), created_pred, created_obj, graph_name.clone());
        self.store.insert(&quad)?;

        let updated_pred = NamedNode::new("memexia:updatedAt")?;
        let updated_obj = Term::from(Literal::new_simple_literal(node.updated_at.to_rfc3339()));
        let quad = Quad::new(subject, updated_pred, updated_obj, graph_name);
        self.store.insert(&quad)?;

        Ok(())
    }

    fn get_node(&self, id: &str) -> Result<Option<Node>> {
        let subject = NamedOrBlankNode::from(NamedNode::new(id)?);

        // 查询所有以该节点为主题的三元组
        let mut quads = Vec::new();
        for result in self.store.quads_for_pattern(Some((&subject).into()), None, None, None) {
            match result {
                Ok(q) => quads.push(q),
                Err(_) => continue,
            }
        }

        if quads.is_empty() {
            return Ok(None);
        }

        let mut node = Node::new(id, NodeType::Concept, "");

        for quad in quads {
            // predicate 带尖括号，需要去掉
            let pred_str = Self::clean_iri(&quad.predicate.to_string());

            if pred_str == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type" || pred_str == "rdf:type" {
                if let Term::NamedNode(obj_node) = &quad.object {
                    let type_str = obj_node.to_string();
                    node.node_type = match type_str.as_str() {
                        "memexia:Concept" => NodeType::Concept,
                        "memexia:Question" => NodeType::Question,
                        "memexia:Evidence" => NodeType::Evidence,
                        "memexia:Resource" => NodeType::Resource,
                        "memexia:Person" => NodeType::Person,
                        "memexia:Event" => NodeType::Event,
                        "memexia:Meta" => NodeType::Meta,
                        _ => NodeType::Concept,
                    };
                }
            } else if pred_str == "memexia:title" {
                if let Term::Literal(lit) = &quad.object {
                    node.title = lit.value().to_string();
                }
            } else if pred_str == "memexia:content" {
                if let Term::Literal(lit) = &quad.object {
                    node.content = Some(lit.value().to_string());
                }
            } else if pred_str == "memexia:tag" {
                if let Term::Literal(lit) = &quad.object {
                    node.tags.push(lit.value().to_string());
                }
            } else if pred_str == "memexia:createdAt" {
                if let Term::Literal(lit) = &quad.object {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(
                        lit.value().to_string().as_str(),
                    ) {
                        node.created_at = dt.with_timezone(&chrono::Utc);
                    }
                }
            } else if pred_str == "memexia:updatedAt" {
                if let Term::Literal(lit) = &quad.object {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(
                        lit.value().to_string().as_str(),
                    ) {
                        node.updated_at = dt.with_timezone(&chrono::Utc);
                    }
                }
            }
        }

        Ok(Some(node))
    }

    fn update_node(&self, node: &Node) -> Result<()> {
        self.delete_node(&node.id)?;
        self.add_node(node)?;
        Ok(())
    }

    fn delete_node(&self, id: &str) -> Result<()> {
        let subject = NamedOrBlankNode::from(NamedNode::new(id)?);

        let quads: Result<Vec<Quad>, _> = self
            .store
            .quads_for_pattern(Some((&subject).into()), None, None, None)
            .collect();

        match quads {
            Ok(quads) => {
                for quad in quads {
                    self.store.remove(&quad)?;
                }
            }
            Err(e) => {
                bail!("Failed to query quads: {:?}", e);
            }
        }

        Ok(())
    }

    fn list_nodes(&self) -> Result<Vec<Node>> {
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for result in self.store.iter() {
            match result {
                Ok(quad) => {
                    // 使用 is_named_node() 方法检查是否是 NamedNode
                    if quad.subject.is_named_node() {
                        // subject.to_string() 返回带尖括号的IRI，需要清理
                        let subj_str = Self::clean_iri(&quad.subject.to_string());
                        if subj_str.starts_with("urn:memexia:") && seen.insert(subj_str.clone()) {
                            if let Ok(Some(node)) = self.get_node(&subj_str) {
                                nodes.push(node);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }

        Ok(nodes)
    }

    fn add_edge(&self, edge: &Edge) -> Result<()> {
        let subject = NamedOrBlankNode::from(NamedNode::new(&edge.from)?);
        let predicate = NamedNode::new(&format!("memexia:{}", edge.relation))?;
        let object = NamedNode::new(&edge.to)?;
        let object_term = Term::from(object);
        let graph_name = GraphName::DefaultGraph;

        let quad = Quad::new(subject, predicate, object_term, graph_name);
        self.store.insert(&quad)?;

        Ok(())
    }

    fn get_edge(&self, id: &str) -> Result<Option<Edge>> {
        if !id.starts_with("urn:memexia:edge:") {
            bail!("Invalid edge ID format");
        }

        let suffix = id.strip_prefix("urn:memexia:edge:").unwrap();
        let parts: Vec<&str> = suffix.split('-').collect();
        if parts.len() < 2 {
            bail!("Invalid edge ID format");
        }

        let from = parts[0];
        let to = parts[1..].join("-");

        let from_node = NamedOrBlankNode::from(NamedNode::new(from)?);
        let to_node = NamedNode::new(&to)?;
        let to_term = Term::from(to_node);

        for result in self.store.quads_for_pattern(
            Some((&from_node).into()),
            None,
            Some((&to_term).into()),
            None,
        ) {
            match result {
                Ok(quad) => {
                    if let Some(relation) = parse_relation_type(&Self::clean_iri(&quad.predicate.to_string())) {
                        return Ok(Some(Edge::new(id, from, &to, relation)));
                    }
                }
                Err(_) => {}
            }
        }

        Ok(None)
    }

    fn get_edges_for_node(&self, node_id: &str, direction: EdgeDirection) -> Result<Vec<Edge>> {
        let mut edges = Vec::new();
        let node = NamedOrBlankNode::from(NamedNode::new(node_id)?);

        match direction {
            EdgeDirection::Outgoing => {
                for result in self.store.quads_for_pattern(Some((&node).into()), None, None, None) {
                    match result {
                        Ok(quad) => {
                            let pred_str = Self::clean_iri(&quad.predicate.to_string());
                            if pred_str.starts_with("memexia:") {
                                if let Term::NamedNode(obj_node) = &quad.object {
                                    let obj_str = Self::clean_iri(&obj_node.to_string());
                                    if let Some(relation) = parse_relation_type(&pred_str) {
                                        let edge_id = format!("urn:memexia:edge:{}-{}", node_id, obj_str);
                                        edges.push(Edge::new(&edge_id, node_id, &obj_str, relation));
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            EdgeDirection::Incoming => {
                let target = Term::from(NamedNode::new(node_id)?);
                for result in self.store.quads_for_pattern(None, None, Some((&target).into()), None) {
                    match result {
                        Ok(quad) => {
                            if quad.subject.is_named_node() {
                                let pred_str = Self::clean_iri(&quad.predicate.to_string());
                                if pred_str.starts_with("memexia:") {
                                    if let Some(relation) = parse_relation_type(&pred_str) {
                                        let subj_str = Self::clean_iri(&quad.subject.to_string());
                                        let edge_id = format!("urn:memexia:edge:{}-{}", subj_str, node_id);
                                        edges.push(Edge::new(&edge_id, &subj_str, node_id, relation));
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            EdgeDirection::Both => {
                // Outgoing
                for result in self.store.quads_for_pattern(Some((&node).into()), None, None, None) {
                    match result {
                        Ok(quad) => {
                            let pred_str = Self::clean_iri(&quad.predicate.to_string());
                            if pred_str.starts_with("memexia:") {
                                if let Term::NamedNode(obj_node) = &quad.object {
                                    let obj_str = Self::clean_iri(&obj_node.to_string());
                                    if let Some(relation) = parse_relation_type(&pred_str) {
                                        let edge_id = format!("urn:memexia:edge:{}-{}", node_id, obj_str);
                                        edges.push(Edge::new(&edge_id, node_id, &obj_str, relation));
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
                // Incoming
                let target = Term::from(NamedNode::new(node_id)?);
                for result in self.store.quads_for_pattern(None, None, Some((&target).into()), None) {
                    match result {
                        Ok(quad) => {
                            if quad.subject.is_named_node() {
                                let pred_str = Self::clean_iri(&quad.predicate.to_string());
                                if pred_str.starts_with("memexia:") {
                                    if let Some(relation) = parse_relation_type(&pred_str) {
                                        let subj_str = Self::clean_iri(&quad.subject.to_string());
                                        let edge_id = format!("urn:memexia:edge:{}-{}", subj_str, node_id);
                                        edges.push(Edge::new(&edge_id, &subj_str, node_id, relation));
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
        }

        Ok(edges)
    }

    fn query_edges(&self, filter: EdgeFilter) -> Result<Vec<Edge>> {
        let mut edges = Vec::new();

        let subject = filter.from.as_ref().map(|s| {
            NamedOrBlankNode::from(NamedNode::new(s).unwrap())
        });
        let object = filter.to.as_ref().map(|t| Term::from(NamedNode::new(t).unwrap()));

        for result in self.store.quads_for_pattern(
            subject.as_ref().map(|s| (s as &NamedOrBlankNode).into()),
            None,
            object.as_ref().map(|t| (t as &Term).into()),
            None,
        ) {
            match result {
                Ok(quad) => {
                    let pred_str = Self::clean_iri(&quad.predicate.to_string());
                    if pred_str.starts_with("memexia:") {
                        if quad.subject.is_named_node() && quad.object.is_named_node() {
                            if let Some(relation) = parse_relation_type(&pred_str) {
                                let s_str = Self::clean_iri(&quad.subject.to_string());
                                let o_str = Self::clean_iri(&quad.object.to_string());
                                let edge_id = format!("urn:memexia:edge:{}-{}", s_str, o_str);
                                edges.push(Edge::new(&edge_id, &s_str, &o_str, relation));
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }

        Ok(edges)
    }

    fn delete_edge(&self, id: &str) -> Result<()> {
        if !id.starts_with("urn:memexia:edge:") {
            bail!("Invalid edge ID format");
        }

        let suffix = id.strip_prefix("urn:memexia:edge:").unwrap();
        let parts: Vec<&str> = suffix.split('-').collect();
        if parts.len() < 2 {
            bail!("Invalid edge ID format");
        }

        let from = parts[0];
        let to = parts[1..].join("-");

        let from_node = NamedOrBlankNode::from(NamedNode::new(from)?);
        let to_term = Term::from(NamedNode::new(&to)?);

        let quads: Result<Vec<Quad>, _> = self
            .store
            .quads_for_pattern(
                Some((&from_node).into()),
                None,
                Some((&to_term).into()),
                None,
            )
            .collect();

        match quads {
            Ok(quads) => {
                for quad in quads {
                    self.store.remove(&quad)?;
                }
            }
            Err(e) => {
                bail!("Failed to query quads: {:?}", e);
            }
        }

        Ok(())
    }

    fn list_edges(&self) -> Result<Vec<Edge>> {
        let mut edges = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for result in self.store.iter() {
            match result {
                Ok(quad) => {
                    let pred_str = Self::clean_iri(&quad.predicate.to_string());
                    if pred_str.starts_with("memexia:") {
                        if quad.subject.is_named_node() && quad.object.is_named_node() {
                            // subject 和 object 都需要清理
                            let s_str = Self::clean_iri(&quad.subject.to_string());
                            let o_str = Self::clean_iri(&quad.object.to_string());
                            let edge_id = format!("urn:memexia:edge:{}-{}", s_str, o_str);
                            if seen.insert(edge_id.clone()) {
                                if let Some(relation) = parse_relation_type(&pred_str) {
                                    edges.push(Edge::new(&edge_id, &s_str, &o_str, relation));
                                }
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }

        Ok(edges)
    }

    fn query(&self, sparql: &str) -> Result<QueryResult> {
        use oxigraph::sparql::{QueryResults, SparqlEvaluator};

        let mut result = QueryResult::default();

        // 使用 SparqlEvaluator 执行 SPARQL 查询
        let evaluation = SparqlEvaluator::new()
            .parse_query(sparql)?
            .on_store(&self.store)
            .execute()?;

        // 处理查询结果
        match evaluation {
            QueryResults::Boolean(_) => {
                // ASK 查询结果 - 没有绑定
            }
            QueryResults::Solutions(solutions) => {
                for solution in solutions {
                    match solution {
                        Ok(b) => {
                            let mut row = std::collections::HashMap::new();
                            for (var, term) in &b {
                                let value = term.to_string();
                                row.insert(var.to_string(), Self::clean_iri(&value));
                            }
                            result.add_binding(row);
                        }
                        Err(_) => {
                            // 跳过无效的绑定
                            continue;
                        }
                    }
                }
            }
            QueryResults::Graph(_) => {
                // CONSTRUCT/DESCRIBE 查询
                bail!("Graph results not supported yet. Use SELECT queries instead.");
            }
        }

        Ok(result)
    }

    fn node_exists(&self, id: &str) -> Result<bool> {
        let subject = NamedOrBlankNode::from(NamedNode::new(id)?);
        Ok(self
            .store
            .quads_for_pattern(Some((&subject).into()), None, None, None)
            .next()
            .transpose()?
            .is_some())
    }

    fn edge_exists(&self, id: &str) -> Result<bool> {
        Ok(self.get_edge(id)?.is_some())
    }

    fn get_stats(&self) -> Result<GraphStats> {
        let nodes = self.list_nodes()?;
        let edges = self.list_edges()?;

        let mut node_type_counts = std::collections::HashMap::new();
        for node in &nodes {
            *node_type_counts.entry(node.node_type.clone()).or_insert(0) += 1;
        }

        let mut relation_counts = std::collections::HashMap::new();
        for edge in &edges {
            *relation_counts.entry(edge.relation).or_insert(0) += 1;
        }

        Ok(GraphStats {
            node_count: nodes.len(),
            edge_count: edges.len(),
            node_type_counts: node_type_counts.into_iter().collect(),
            relation_counts: relation_counts.into_iter().collect(),
        })
    }
}

fn parse_relation_type(s: &str) -> Option<RelationType> {
    use RelationType::*;

    let normalized = s.to_lowercase().replace("memexia:", "");

    match normalized.as_str() {
        "contains" => Some(Contains),
        "partof" | "part_of" => Some(PartOf),
        "instanceof" | "instance_of" => Some(InstanceOf),
        "derivesfrom" | "derives_from" => Some(DerivesFrom),
        "leadsto" | "leads_to" => Some(LeadsTo),
        "supports" => Some(Supports),
        "contradicts" => Some(Contradicts),
        "refines" => Some(Refines),
        "references" => Some(References),
        "relatedto" | "related_to" => Some(RelatedTo),
        "analogousto" | "analogous_to" => Some(AnalogousTo),
        "precedes" => Some(Precedes),
        "follows" => Some(Follows),
        "simultaneous" => Some(Simultaneous),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OxigraphStorage::create(&temp_dir.path().join("graph")).unwrap();

        let node = Node::new("urn:memexia:file:test.md", NodeType::Concept, "Test Node");
        storage.add_node(&node).unwrap();

        assert!(storage.node_exists("urn:memexia:file:test.md").unwrap());

        let retrieved = storage.get_node("urn:memexia:file:test.md").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Node");
    }

    #[test]
    fn test_edge_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OxigraphStorage::create(&temp_dir.path().join("graph")).unwrap();

        let node1 = Node::new("urn:memexia:file:a.md", NodeType::Concept, "A");
        let node2 = Node::new("urn:memexia:file:b.md", NodeType::Concept, "B");
        storage.add_node(&node1).unwrap();
        storage.add_node(&node2).unwrap();

        let edge = Edge::new(
            "urn:memexia:edge:a-b",
            "urn:memexia:file:a.md",
            "urn:memexia:file:b.md",
            RelationType::Contradicts,
        );
        storage.add_edge(&edge).unwrap();

        let edges = storage
            .get_edges_for_node("urn:memexia:file:a.md", EdgeDirection::Outgoing)
            .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].relation, RelationType::Contradicts);

        let stats = storage.get_stats().unwrap();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
    }

    #[test]
    fn test_sparql_query() {
        let temp_dir = TempDir::new().unwrap();
        let storage = OxigraphStorage::create(&temp_dir.path().join("graph")).unwrap();

        // 添加测试数据
        let node1 = Node::new("urn:memexia:file:doc1.md", NodeType::Concept, "Document 1");
        let node2 = Node::new("urn:memexia:file:doc2.md", NodeType::Concept, "Document 2");
        storage.add_node(&node1).unwrap();
        storage.add_node(&node2).unwrap();

        let edge = Edge::new(
            "urn:memexia:edge:doc1-doc2",
            "urn:memexia:file:doc1.md",
            "urn:memexia:file:doc2.md",
            RelationType::RelatedTo,
        );
        storage.add_edge(&edge).unwrap();

        // SPARQL 查询测试 - 查询所有节点
        let result = storage.query("SELECT ?s ?title WHERE { ?s <memexia:title> ?title }").unwrap();
        assert_eq!(result.bindings.len(), 2);

        // 验证返回的绑定包含正确的结果
        assert!(!result.bindings.is_empty());
        for binding in &result.bindings {
            // 验证绑定包含 s 和 title 变量（SPARQL 变量带 ? 前缀）
            assert!(binding.contains_key("?s"));
            assert!(binding.contains_key("?title"));
        }

        // 测试查询条件过滤 - 只查询特定类型的节点
        let result2 = storage
            .query("SELECT ?s WHERE { ?s <rdf:type> <memexia:Concept> }")
            .unwrap();
        assert_eq!(result2.bindings.len(), 2);
    }
}
