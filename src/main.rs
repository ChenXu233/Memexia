use clap::{Parser, Subcommand};
use memexia::cli;
use tracing::{error, info};

fn main() -> anyhow::Result<()> {
    // 初始化应用
    memexia::init()?;
    
    // 解析命令行参数
    let cli = Cli::parse();
    
    // 设置日志级别
    if cli.verbose {
        unsafe {
            std::env::set_var("RUST_LOG", "debug");
        }
    }
    
    // 执行命令
    match cli.command {
        Commands::Init(args) => cli::commands::init_repository(args),
        Commands::Add(args) => cli::commands::add_files(args),
        Commands::Status(args) => cli::commands::status(args),
        Commands::Commit(args) => cli::commands::commit(args),
        Commands::Graph(args) => cli::commands::graph_operations(args),
        Commands::Search(args) => cli::commands::search(args),
        Commands::Sync(args) => cli::commands::sync(args),
        Commands::Serve(args) => cli::commands::serve(args),
        Commands::Config(args) => cli::commands::config(args),
    }
}

#[derive(Parser)]
#[command(name = "memexia")]
#[command(about = "Personal knowledge graph system")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Memexia repository
    Init(cli::InitArgs),
    
    /// Add files to the repository
    Add(cli::AddArgs),
    
    /// Show repository status
    Status(cli::StatusArgs),
    
    /// Commit changes to the repository
    Commit(cli::CommitArgs),
    
    /// Graph database operations
    Graph(cli::GraphArgs),
    
    /// Search in the repository
    Search(cli::SearchArgs),
    
    /// Sync with remote repository
    Sync(cli::SyncArgs),
    
    /// Start the local API server for GUI
    Serve(cli::ServeArgs),
    
    /// Manage configuration
    Config(cli::ConfigArgs),
}