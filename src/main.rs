use clap::Parser;
use memexia::cli::{Cli, Commands, commands};

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
        Commands::Init(args) => commands::init_repository(args),
        Commands::Add(args) => commands::add_files(args),
        Commands::Status(args) => commands::status(args),
        Commands::Commit(args) => commands::commit(args),
        Commands::Graph(args) => commands::graph_operations(args),
        Commands::Search(args) => commands::search(args),
        Commands::Sync(args) => commands::sync(args),
        Commands::Serve(args) => commands::serve(args),
        Commands::Config(args) => commands::config(args),
        Commands::Reindex(args) => commands::reindex(args),
        Commands::File(args) => commands::file_operations(args),
        Commands::Link(args) => commands::link_operations(args),
    }
}
