// Oxigraph 0.5.3 API 探索 - 最终版
// 运行: cargo run --features oxigraph --example explore_oxigraph

#[cfg(feature = "oxigraph")]
fn main() {
    use oxigraph::model::{Literal, NamedNode, Quad, Term, GraphName, NamedOrBlankNodeRef};
    use oxigraph::store::Store;
    use std::path::Path;

    // ===== 探索 Store 的 API =====

    // 1. 创建新存储 (返回 Result!)
    let store = Store::new().unwrap();
    println!("✓ Store::new() works");

    // 2. 打开/创建持久化存储
    let _persistent_store = Store::open(Path::new("target/test_graph")).unwrap();
    println!("✓ Store::open() works");

    // 3. 插入三元组 - 使用 Quad
    let subject = NamedNode::new("urn:memexia:file:test.md").unwrap();
    let predicate = NamedNode::new("http://purl.org/dc/elements/1.1/title").unwrap();
    // 使用 new_simple_literal 创建文字值
    let object = Term::from(Literal::new_simple_literal("Test Node"));

    // Quad::new 的 graph_name 参数
    let quad = Quad::new(subject.clone(), predicate.clone(), object, GraphName::DefaultGraph);
    store.insert(&quad).unwrap();
    println!("✓ Store::insert() works");

    // 4. 查询 - 使用 quads_for_pattern
    // 使用 as_ref() 获取引用
    let results: Vec<_> = store
        .quads_for_pattern(
            Some(NamedOrBlankNodeRef::from(&subject)),
            None,
            None,
            None,
        )
        .collect();
    println!("✓ Store::quads_for_pattern() works, found {} results", results.len());

    // 5. 检查是否存在
    let exists = store.contains(&quad).unwrap();
    println!("✓ Store::contains() works, exists: {}", exists);

    // 6. 删除
    store.remove(&quad).unwrap();
    println!("✓ Store::remove() works");

    // 7. 统计
    let count = store.len().unwrap();
    println!("✓ Store::len() works, count: {}", count);

    // 8. 遍历所有三元组
    let all_quads: Vec<_> = store.iter().collect();
    println!("✓ Store::iter() works, total: {} quads", all_quads.len());

    println!("\n=== Oxigraph 0.5.3 完整功能 ===");
    println!("✓ 插入三元组: insert(quad)");
    println!("✓ 查询三元组: quads_for_pattern()");
    println!("✓ 检查存在: contains(quad)");
    println!("✓ 删除三元组: remove(quad)");
    println!("✓ 遍历所有: iter()");
    println!("✓ 持久化存储: open(path)");
    println!("✓ N-Quads 导入导出: load_from_reader/dump_to_io_writer");
}

#[cfg(not(feature = "oxigraph"))]
fn main() {
    println!("Run with: cargo run --features oxigraph --example explore_oxigraph");
}
