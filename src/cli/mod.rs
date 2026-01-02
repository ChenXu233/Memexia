use clap::Args;

pub mod commands;

#[derive(Args)]
pub struct InitArgs {
    /// Path to initialize the repository in
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,
}

#[derive(Args)]
pub struct AddArgs {
    /// Files to add
    #[arg(required = true)]
    pub files: Vec<std::path::PathBuf>,
}

#[derive(Args)]
pub struct StatusArgs {}

#[derive(Args)]
pub struct CommitArgs {
    /// Commit message
    #[arg(short, long)]
    pub message: String,
}

#[derive(Args)]
pub struct GraphArgs {
    /// Subcommand for graph operations
    #[arg(long)]
    pub query: Option<String>,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,
}

#[derive(Args)]
pub struct SyncArgs {}

#[derive(Args)]
pub struct ServeArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    pub port: u16,
}

#[derive(Args)]
pub struct ConfigArgs {
    /// Key to set or get
    pub key: Option<String>,
    /// Value to set
    pub value: Option<String>,
}

#[derive(Args)]
pub struct ReindexArgs {
    /// Full reindex (delete all and rebuild)
    #[arg(short, long)]
    pub full: bool,
    /// Path to reindex (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: std::path::PathBuf,
}
