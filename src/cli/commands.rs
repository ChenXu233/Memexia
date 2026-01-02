use super::*;
use crate::core::repository::Repository;
use crate::storage::{Edge, Node, NodeType, RelationType};
use anyhow::{bail, Result};
use serde_json::json;
use std::path::Path;
use tracing::info;

pub fn init_repository(args: InitArgs) -> Result<()> {
    info!("Initializing repository at {:?}", args.path);
    Repository::init(&args.path)?;
    println!("Repository initialized at {:?}", args.path);
    Ok(())
}

pub fn add_files(args: AddArgs) -> Result<()> {
    info!("Adding files: {:?}", args.files);
    let repo = Repository::open(Path::new("."))?;
    repo.add(&args.files)?;
    Ok(())
}

pub fn status(_args: StatusArgs) -> Result<()> {
    let repo = Repository::open(Path::new("."))?;
    let status = repo.status()?;
    println!("{}", status);
    Ok(())
}

pub fn commit(args: CommitArgs) -> Result<()> {
    info!("Committing with message: {}", args.message);
    let repo = Repository::open(Path::new("."))?;
    repo.commit(&args.message)?;
    Ok(())
}

pub fn graph_operations(args: GraphArgs) -> Result<()> {
    let repo = Repository::open(Path::new("."))?;
    let storage = repo.storage();

    match args.command {
        GraphCommands::Show(args) => {
            let nodes = storage.graph().get_all_nodes()?;
            let edges = storage.graph().get_all_edges()?;

            if args.json {
                let output = json!({
                    "nodes": nodes.iter().map(|n| json!({
                        "id": n.id,
                        "type": format!("{:?}", n.node_type),
                        "label": n.title
                    })).collect::<Vec<_>>(),
                    "edges": edges.iter().map(|e| json!({
                        "from": e.from,
                        "to": e.to,
                        "type": format!("{:?}", e.relation),
                        "strength": e.strength
                    })).collect::<Vec<_>>()
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("=== Graph Structure ===");
                println!("Nodes ({}):", nodes.len());
                for node in nodes.iter().take(20) {
                    println!("  - {} [{}]", node.id, format!("{:?}", node.node_type));
                }
                if nodes.len() > 20 {
                    println!("  ... and {} more", nodes.len() - 20);
                }
                println!("\nEdges ({}):", edges.len());
                for edge in edges.iter().take(20) {
                    println!("  {} --[{}]--> {}", edge.from, format!("{:?}", edge.relation), edge.to);
                }
                if edges.len() > 20 {
                    println!("  ... and {} more", edges.len() - 20);
                }
            }
            Ok(())
        }

        GraphCommands::Dot(_args) => {
            let nodes = storage.graph().get_all_nodes()?;
            let edges = storage.graph().get_all_edges()?;

            println!("digraph memexia_graph {{");
            println!("  node [shape=box, style=filled];");
            for node in &nodes {
                let color = match node.node_type {
                    NodeType::Concept => "lightblue",
                    NodeType::Question => "lightyellow",
                    _ => "lightgreen",
                };
                println!("  \"{}\" [fillcolor={}];", node.id, color);
            }
            println!();
            for edge in &edges {
                println!("  \"{}\" -> \"{}\" [label=\"{}\"];", edge.from, edge.to, format!("{:?}", edge.relation));
            }
            println!("}}");
            Ok(())
        }

        GraphCommands::Stats(_args) => {
            let nodes = storage.graph().get_all_nodes()?;
            let edges = storage.graph().get_all_edges()?;

            let mut type_counts = std::collections::HashMap::new();
            for node in &nodes {
                *type_counts.entry(node.node_type.clone()).or_insert(0) += 1;
            }

            let mut relation_counts = std::collections::HashMap::new();
            for edge in &edges {
                *relation_counts.entry(edge.relation).or_insert(0) += 1;
            }

            println!("=== Graph Statistics ===");
            println!("Total nodes: {}", nodes.len());
            println!("Total edges: {}", edges.len());
            println!("\nNodes by type:");
            for (node_type, count) in &type_counts {
                println!("  {:?}: {}", node_type, count);
            }
            println!("\nEdges by relation:");
            for (relation, count) in &relation_counts {
                println!("  {:?}: {}", relation, count);
            }

            if !edges.is_empty() {
                let avg_strength: f64 = edges.iter().map(|e| e.strength).sum::<f64>() / edges.len() as f64;
                println!("\nAverage edge strength: {:.2}", avg_strength);
            }

            Ok(())
        }

        GraphCommands::Query(args) => {
            let results = storage.graph().sparql_query(&args.query)?;
            println!("SPARQL Results:");
            for result in results {
                println!("{}", result);
            }
            Ok(())
        }

        GraphCommands::Path(args) => {
            let path = storage.graph().find_path(&args.source, &args.target)?;
            match path {
                Some(nodes) => {
                    println!("Path found ({} hops):", nodes.len() - 1);
                    for (i, node) in nodes.iter().enumerate() {
                        if i > 0 {
                            print!(" --> ");
                        }
                        print!("{}", node);
                    }
                    println!();
                }
                None => {
                    println!("No path found between {} and {}", args.source, args.target);
                }
            }
            Ok(())
        }
    }
}

pub fn search(_args: SearchArgs) -> Result<()> {
    info!("Search not implemented yet");
    Ok(())
}

pub fn sync(_args: SyncArgs) -> Result<()> {
    info!("Sync not implemented yet");
    Ok(())
}

pub fn serve(_args: ServeArgs) -> Result<()> {
    info!("Serve not implemented yet");
    Ok(())
}

pub fn config(_args: ConfigArgs) -> Result<()> {
    info!("Config not implemented yet");
    Ok(())
}

pub fn reindex(args: ReindexArgs) -> Result<()> {
    use crate::core::{Indexer, Repository};

    let root = args.path.canonicalize()?;

    if args.full {
        info!("Full reindex of {:?}", root);
    } else {
        info!("Incremental reindex of {:?}", root);
    }

    let repo = Repository::open(&root)?;
    let storage = repo.storage();
    let indexer = Indexer::new(storage.clone());

    let summary = if args.full {
        indexer.reindex_all(&root)
    } else {
        indexer.index_all(&root)
    }?;

    println!("\n=== Reindex Summary ===");
    println!("Files indexed: {}", summary.files_indexed);
    println!("Files skipped: {}", summary.files_skipped);
    println!("Files deleted: {}", summary.files_deleted);
    println!("Nodes created: {}", summary.nodes_created);
    println!("Edges created: {}", summary.edges_created);

    if !summary.errors.is_empty() {
        println!("\nErrors:");
        for (path, error) in &summary.errors {
            println!("  - {}: {}", path, error);
        }
    }

    if summary.has_errors() {
        anyhow::bail!("Reindex completed with errors");
    }

    Ok(())
}

pub fn file_operations(args: FileArgs) -> Result<()> {
    let repo = Repository::open(Path::new("."))?;
    let storage = repo.storage();

    match args.command {
        FileCommands::Info(args) => {
            let file_path = args.path;
            let relative = file_path.strip_prefix(repo.path()).unwrap_or(&file_path);
            let node_id = format!("urn:memexia:file:{}", relative.to_string_lossy().replace('\\', "/"));

            if let Some(node) = storage.graph().get_node(&node_id)? {
                println!("=== File Info ===");
                println!("Path: {}", file_path.display());
                println!("Node ID: {}", node.id);
                println!("Type: {:?}", node.node_type);
                println!("Label: {}", node.title);
                if !node.tags.is_empty() {
                    println!("Tags: {}", node.tags.join(", "));
                }
            } else {
                println!("File not indexed: {}", file_path.display());
            }
            Ok(())
        }

        FileCommands::Links(args) => {
            let file_path = args.path;
            let relative = file_path.strip_prefix(repo.path()).unwrap_or(&file_path);
            let node_id = format!("urn:memexia:file:{}", relative.to_string_lossy().replace('\\', "/"));

            let edges = storage.graph().get_edges_by_source(&node_id)?;

            println!("=== Outgoing Links ===");
            if edges.is_empty() {
                println!("No outgoing links from {}", file_path.display());
            } else {
                for edge in &edges {
                    println!("  --[{:?}]--> {}", edge.relation, edge.to);
                }
                println!("\nTotal: {} outgoing links", edges.len());
            }
            Ok(())
        }

        FileCommands::Backlinks(args) => {
            let file_path = args.path;
            let relative = file_path.strip_prefix(repo.path()).unwrap_or(&file_path);
            let node_id = format!("urn:memexia:file:{}", relative.to_string_lossy().replace('\\', "/"));

            let edges = storage.graph().get_edges_by_target(&node_id)?;

            println!("=== Backlinks ===");
            if edges.is_empty() {
                println!("No backlinks to {}", file_path.display());
            } else {
                for edge in &edges {
                    println!("  <--[{:?}]-- {}", edge.relation, edge.from);
                }
                println!("\nTotal: {} backlinks", edges.len());
            }
            Ok(())
        }
    }
}

pub fn link_operations(args: LinkArgs) -> Result<()> {
    let repo = Repository::open(Path::new("."))?;
    let storage = repo.storage();

    match args.command {
        LinkCommands::Create(args) => {
            let relation = match args.relation.to_lowercase().as_str() {
                "contains" | "belongsto" => RelationType::Contains,
                "partof" | "part_of" => RelationType::PartOf,
                "derivesfrom" | "derives_from" | "leadsto" | "leads_to" => RelationType::LeadsTo,
                "supports" => RelationType::Supports,
                "contradicts" => RelationType::Contradicts,
                "refines" => RelationType::Refines,
                "relatedto" | "related_to" | "similar" => RelationType::RelatedTo,
                "analogousto" | "analogous_to" => RelationType::AnalogousTo,
                "references" | "cites" => RelationType::References,
                "instanceof" | "instance_of" => RelationType::InstanceOf,
                "precedes" => RelationType::Precedes,
                "follows" => RelationType::Follows,
                "simultaneous" => RelationType::Simultaneous,
                _ => bail!("Unknown relation type: {}", args.relation),
            };

            let source = args.source.strip_prefix(repo.path()).unwrap_or(&args.source);
            let target = args.target.strip_prefix(repo.path()).unwrap_or(&args.target);

            let source_id = format!("urn:memexia:file:{}", source.to_string_lossy().replace('\\', "/"));
            let target_id = format!("urn:memexia:file:{}", target.to_string_lossy().replace('\\', "/"));

            if !storage.graph().node_exists(&source_id)? {
                let source_node = Node::new(&source_id, NodeType::Resource, &*source.to_string_lossy());
                storage.graph().add_node(&source_node)?;
            }

            if !storage.graph().node_exists(&target_id)? {
                let target_node = Node::new(&target_id, NodeType::Resource, &*target.to_string_lossy());
                storage.graph().add_node(&target_node)?;
            }

            let edge_id = format!("urn:memexia:edge:{}-{}", source_id, target_id);
            let edge = Edge::new(&edge_id, &source_id, &target_id, relation);
            storage.graph().add_edge(&edge)?;

            println!("Link created: {} --[{:?}]--> {}", args.source.display(), relation, args.target.display());
            Ok(())
        }

        LinkCommands::Delete(args) => {
            let source = args.source.strip_prefix(repo.path()).unwrap_or(&args.source);
            let target = args.target.strip_prefix(repo.path()).unwrap_or(&args.target);

            let source_id = format!("urn:memexia:file:{}", source.to_string_lossy().replace('\\', "/"));
            let target_id = format!("urn:memexia:file:{}", target.to_string_lossy().replace('\\', "/"));

            let edge_id = format!("urn:memexia:edge:{}-{}", source_id, target_id);
            storage.graph().remove_edge(&edge_id)?;

            println!("Link deleted: {} --> {}", args.source.display(), args.target.display());
            Ok(())
        }

        LinkCommands::Query(args) => {
            let mut edges = storage.graph().get_all_edges()?;

            if let Some(source_path) = &args.source {
                let relative = source_path.strip_prefix(repo.path()).unwrap_or(source_path);
                let source_id = format!("urn:memexia:file:{}", relative.to_string_lossy().replace('\\', "/"));
                edges.retain(|e| e.from == source_id);
            }

            if let Some(target_path) = &args.target {
                let relative = target_path.strip_prefix(repo.path()).unwrap_or(target_path);
                let target_id = format!("urn:memexia:file:{}", relative.to_string_lossy().replace('\\', "/"));
                edges.retain(|e| e.to == target_id);
            }

            if let Some(relation_str) = &args.relation {
                let relation = match relation_str.to_lowercase().as_str() {
                    "contains" | "belongsto" => RelationType::Contains,
                    "partof" | "part_of" => RelationType::PartOf,
                    "derivesfrom" | "derives_from" | "leadsto" | "leads_to" => RelationType::LeadsTo,
                    "supports" => RelationType::Supports,
                    "contradicts" => RelationType::Contradicts,
                    "refines" => RelationType::Refines,
                    "relatedto" | "related_to" | "similar" => RelationType::RelatedTo,
                    "analogousto" | "analogous_to" => RelationType::AnalogousTo,
                    "references" | "cites" => RelationType::References,
                    "instanceof" | "instance_of" => RelationType::InstanceOf,
                    "precedes" => RelationType::Precedes,
                    "follows" => RelationType::Follows,
                    "simultaneous" => RelationType::Simultaneous,
                    _ => RelationType::RelatedTo,
                };
                edges.retain(|e| e.relation == relation);
            }

            println!("=== Query Results ===");
            if edges.is_empty() {
                println!("No matching links found");
            } else {
                for edge in &edges {
                    println!("  {} --[{:?}]--> {}", edge.from, edge.relation, edge.to);
                }
                println!("\nTotal: {} links", edges.len());
            }
            Ok(())
        }
    }
}
