//! 图存储抽象层
//!
//! 定义 `GraphStorage` trait，为不同图数据库实现提供统一接口

use super::{Edge, Node};
use anyhow::Result;

/// SPARQL 查询结果
#[derive(Debug, Clone, Default)]
pub struct QueryResult {
    /// 绑定变量名到值的映射
    pub bindings: Vec<std::collections::HashMap<String, String>>,
}

impl QueryResult {
    /// 创建新的查询结果
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// 添加绑定结果
    pub fn add_binding(&mut self, binding: std::collections::HashMap<String, String>) {
        self.bindings.push(binding);
    }

    /// 检查是否有结果
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// 获取结果数量
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
}

/// 图存储抽象 trait
///
/// 定义节点和边的 CRUD 操作以及 SPARQL 查询接口
pub trait GraphStorage: Send + Sync {
    /// 添加节点
    ///
    /// # Arguments
    ///
    /// * `node` - 要添加的节点
    ///
    /// # Returns
    ///
    /// 操作结果
    fn add_node(&self, node: &Node) -> Result<()>;

    /// 获取节点
    ///
    /// # Arguments
    ///
    /// * `id` - 节点 ID
    ///
    /// # Returns
    ///
    /// 找到的节点或 None
    fn get_node(&self, id: &str) -> Result<Option<Node>>;

    /// 更新节点
    ///
    /// # Arguments
    ///
    /// * `node` - 要更新的节点
    ///
    /// # Returns
    ///
    /// 操作结果
    fn update_node(&self, node: &Node) -> Result<()>;

    /// 删除节点
    ///
    /// # Arguments
    ///
    /// * `id` - 要删除的节点 ID
    ///
    /// # Returns
    ///
    /// 操作结果
    fn delete_node(&self, id: &str) -> Result<()>;

    /// 列出所有节点
    ///
    /// # Returns
    ///
    /// 所有节点的列表
    fn list_nodes(&self) -> Result<Vec<Node>>;

    /// 获取所有节点
    ///
    /// # Returns
    ///
    /// 所有节点的列表
    fn get_all_nodes(&self) -> Result<Vec<Node>>;

    /// 添加边
    ///
    /// # Arguments
    ///
    /// * `edge` - 要添加的边
    ///
    /// # Returns
    ///
    /// 操作结果
    fn add_edge(&self, edge: &Edge) -> Result<()>;

    /// 获取边
    ///
    /// # Arguments
    ///
    /// * `id` - 边 ID
    ///
    /// # Returns
    ///
    /// 找到的边或 None
    fn get_edge(&self, id: &str) -> Result<Option<Edge>>;

    /// 获取节点的所有边
    ///
    /// # Arguments
    ///
    /// * `node_id` - 节点 ID
    /// * `direction` - 边的方向
    ///
    /// # Returns
    ///
    /// 匹配的边列表
    fn get_edges_for_node(
        &self,
        node_id: &str,
        direction: EdgeDirection,
    ) -> Result<Vec<Edge>>;

    /// 获取从指定源节点出发的所有边
    ///
    /// # Arguments
    ///
    /// * `source` - 源节点 ID
    ///
    /// # Returns
    ///
    /// 匹配的边列表
    fn get_edges_by_source(&self, source: &str) -> Result<Vec<Edge>>;

    /// 获取指向指定目标节点的所有边
    ///
    /// # Arguments
    ///
    /// * `target` - 目标节点 ID
    ///
    /// # Returns
    ///
    /// 匹配的边列表
    fn get_edges_by_target(&self, target: &str) -> Result<Vec<Edge>>;

    /// 获取满足条件的边
    ///
    /// # Arguments
    ///
    /// * `filter` - 边过滤器
    ///
    /// # Returns
    ///
    /// 匹配的边列表
    fn query_edges(&self, filter: super::EdgeFilter) -> Result<Vec<Edge>>;

    /// 删除边
    ///
    /// # Arguments
    ///
    /// * `id` - 要删除的边 ID
    ///
    /// # Returns
    ///
    /// 操作结果
    fn delete_edge(&self, id: &str) -> Result<()>;

    /// 移除边（删除边的别名）
    ///
    /// # Arguments
    ///
    /// * `id` - 要删除的边 ID
    ///
    /// # Returns
    ///
    /// 操作结果
    fn remove_edge(&self, id: &str) -> Result<()>;

    /// 列出所有边
    ///
    /// # Returns
    ///
    /// 所有边的列表
    fn list_edges(&self) -> Result<Vec<Edge>>;

    /// 获取所有边
    ///
    /// # Returns
    ///
    /// 所有边的列表
    fn get_all_edges(&self) -> Result<Vec<Edge>>;

    /// 执行 SPARQL 查询
    ///
    /// # Arguments
    ///
    /// * `sparql` - SPARQL 查询语句
    ///
    /// # Returns
    ///
    /// 查询结果
    fn query(&self, sparql: &str) -> Result<QueryResult>;

    /// 执行 SPARQL 查询（别名方法）
    ///
    /// # Arguments
    ///
    /// * `sparql` - SPARQL 查询语句
    ///
    /// # Returns
    ///
    /// 查询结果（格式化的字符串列表）
    fn sparql_query(&self, sparql: &str) -> Result<Vec<String>>;

    /// 检查节点是否存在
    ///
    /// # Arguments
    ///
    /// * `id` - 节点 ID
    ///
    /// # Returns
    ///
    /// 是否存在
    fn node_exists(&self, id: &str) -> Result<bool>;

    /// 检查边是否存在
    ///
    /// # Arguments
    ///
    /// * `id` - 边 ID
    ///
    /// # Returns
    ///
    /// 是否存在
    fn edge_exists(&self, id: &str) -> Result<bool>;

    /// 获取图统计信息
    ///
    /// # Returns
    ///
    /// 统计信息
    fn get_stats(&self) -> Result<GraphStats>;

    /// 查找两点间的路径
    ///
    /// # Arguments
    ///
    /// * `source` - 源节点 ID
    /// * `target` - 目标节点 ID
    ///
    /// # Returns
    ///
    /// 路径节点列表（如果存在）
    fn find_path(&self, source: &str, target: &str) -> Result<Option<Vec<String>>>;
}

/// 边的方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeDirection {
    /// 出边（从节点出发）
    #[default]
    Outgoing,
    /// 入边（指向节点）
    Incoming,
    /// 双向
    Both,
}

/// 图统计信息
#[derive(Debug, Clone, Default)]
pub struct GraphStats {
    /// 节点数量
    pub node_count: usize,
    /// 边数量
    pub edge_count: usize,
    /// 节点类型分布
    pub node_type_counts: Vec<(super::NodeType, usize)>,
    /// 关系类型分布
    pub relation_counts: Vec<(super::RelationType, usize)>,
}

impl GraphStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self {
            node_count: 0,
            edge_count: 0,
            node_type_counts: Vec::new(),
            relation_counts: Vec::new(),
        }
    }
}
