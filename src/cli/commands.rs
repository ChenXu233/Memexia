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
