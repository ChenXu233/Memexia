use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

pub mod commands;

#[derive(Parser)]
#[command(name = "memexia")]
#[command(about = "Personal knowledge graph system")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Memexia repository
    Init(InitArgs),

    /// Add files to the repository
    Add(AddArgs),

    /// Show repository status
    Status(StatusArgs),

    /// Commit changes to the repository
    Commit(CommitArgs),

    /// Amend the last commit
    Amend(AmendArgs),

    /// Show commit history
    Log(LogArgs),

    /// Graph database operations
    Graph(GraphArgs),

    /// Search in the repository
    Search(SearchArgs),

    /// Sync with remote repository
    Sync(SyncArgs),

    /// Start the local API server for GUI
    Serve(ServeArgs),

    /// Manage configuration
    Config(ConfigArgs),

    /// Reindex all files in the repository
    Reindex(ReindexArgs),

    /// File operations (view info, links, backlinks)
    File(FileArgs),

    /// Link operations (create, delete, query)
    Link(LinkArgs),
}

#[derive(Args)]
pub struct InitArgs {
    /// Path to initialize the repository in
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

#[derive(Args)]
pub struct AddArgs {
    /// Files to add
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
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
pub struct AmendArgs {
    /// New commit message
    #[arg(short, long)]
    pub message: String,
}

#[derive(Args)]
pub struct LogArgs {
    /// Number of commits to show
    #[arg(short, long)]
    pub limit: Option<usize>,
    /// Show in oneline format
    #[arg(short, long)]
    pub oneline: bool,
}

/// Graph subcommands
#[derive(Args)]
pub struct GraphArgs {
    #[command(subcommand)]
    pub command: GraphCommands,
}

#[derive(Subcommand)]
pub enum GraphCommands {
    /// Show graph structure (JSON format)
    Show(GraphShowArgs),

    /// Export graph in DOT format for visualization
    Dot(GraphDotArgs),

    /// Show graph statistics
    Stats(GraphStatsArgs),

    /// Execute SPARQL query
    Query(GraphQueryArgs),

    /// Find path between two nodes
    Path(GraphPathArgs),
}

#[derive(Args)]
pub struct GraphShowArgs {
    /// Output as JSON
    #[arg(short, long)]
    pub json: bool,
}

#[derive(Args)]
pub struct GraphDotArgs {
    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct GraphStatsArgs {}

#[derive(Args)]
pub struct GraphQueryArgs {
    /// SPARQL query string
    #[arg(required = true)]
    pub query: String,
}

#[derive(Args)]
pub struct GraphPathArgs {
    /// Source node ID or name
    pub source: String,
    /// Target node ID or name
    pub target: String,
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
    pub path: PathBuf,
}

/// File subcommands
#[derive(Args)]
pub struct FileArgs {
    #[command(subcommand)]
    pub command: FileCommands,
}

#[derive(Subcommand)]
pub enum FileCommands {
    /// Show file/node information
    Info(FileInfoArgs),
    /// Show outgoing links from a file
    Links(FileLinksArgs),
    /// Show incoming backlinks to a file
    Backlinks(FileBacklinksArgs),
}

#[derive(Args)]
pub struct FileInfoArgs {
    /// File path
    pub path: PathBuf,
}

#[derive(Args)]
pub struct FileLinksArgs {
    /// File path
    pub path: PathBuf,
}

#[derive(Args)]
pub struct FileBacklinksArgs {
    /// File path
    pub path: PathBuf,
}

/// Link subcommands
#[derive(Args)]
pub struct LinkArgs {
    #[command(subcommand)]
    pub command: LinkCommands,
}

#[derive(Subcommand)]
pub enum LinkCommands {
    /// Create a manual link between two nodes
    Create(LinkCreateArgs),
    /// Delete a link
    Delete(LinkDeleteArgs),
    /// Query links
    Query(LinkQueryArgs),
}

#[derive(Args)]
pub struct LinkCreateArgs {
    /// Source file path
    pub source: PathBuf,
    /// Target file path
    pub target: PathBuf,
    /// Relation type (e.g., RelatedTo, Supports, Contradicts)
    #[arg(short, long, default_value = "RelatedTo")]
    pub relation: String,
}

#[derive(Args)]
pub struct LinkDeleteArgs {
    /// Source file path
    pub source: PathBuf,
    /// Target file path
    pub target: PathBuf,
}

#[derive(Args)]
pub struct LinkQueryArgs {
    /// Filter by source node
    pub source: Option<PathBuf>,
    /// Filter by target node
    pub target: Option<PathBuf>,
    /// Filter by relation type
    pub relation: Option<String>,
}
