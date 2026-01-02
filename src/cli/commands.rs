use super::*;
use crate::core::repository::Repository;
use anyhow::Result;
use tracing::info;
use std::path::Path;

pub fn init_repository(args: InitArgs) -> Result<()> {
    info!("Initializing repository at {:?}", args.path);
    Repository::init(&args.path)?;
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
    if let Some(query) = args.query {
        let repo = Repository::open(std::path::Path::new("."))?;
        let results = repo.query_graph(&query)?;
        for result in results {
            println!("{}", result);
        }
    } else {
        info!("No query provided. Use --query to execute SPARQL.");
    }
    Ok(())
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

    // 打开仓库
    let repo = Repository::open(&root)?;

    // 获取存储
    let storage = repo.storage();

    // 创建索引器
    let indexer = Indexer::new(storage.clone());

    // 执行索引
    let summary = if args.full {
        indexer.reindex_all(&root)
    } else {
        indexer.index_all(&root)
    }?;

    // 打印结果
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
