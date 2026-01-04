//! CLI 参数解析测试

use memexia::cli::{Cli, Commands};
use clap::Parser;

#[test]
fn test_cli_parse_init() {
    let args = vec!["memexia", "init", "/path/to/repo"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Init(init_args) = cli.command {
        assert_eq!(init_args.path.to_string_lossy(), "/path/to/repo");
    }
}

#[test]
fn test_cli_parse_add() {
    let args = vec!["memexia", "add", "file1.md", "file2.md"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Add(add_args) = cli.command {
        assert_eq!(add_args.files.len(), 2);
    }
}

#[test]
fn test_cli_parse_status() {
    let args = vec!["memexia", "status"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::Status(_)));
}

#[test]
fn test_cli_parse_commit() {
    let args = vec!["memexia", "commit", "-m", "Test message"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Commit(commit_args) = cli.command {
        assert_eq!(commit_args.message, "Test message");
    }
}

#[test]
fn test_cli_parse_amend() {
    let args = vec!["memexia", "amend", "-m", "Amended message"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Amend(amend_args) = cli.command {
        assert_eq!(amend_args.message, "Amended message");
    }
}

#[test]
fn test_cli_parse_log() {
    let args = vec!["memexia", "log", "--limit", "10"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Log(log_args) = cli.command {
        assert_eq!(log_args.limit, Some(10));
    }
}

#[test]
fn test_cli_parse_log_default_limit() {
    let args = vec!["memexia", "log"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Log(log_args) = cli.command {
        assert_eq!(log_args.limit, None);
    }
}

#[test]
fn test_cli_parse_verbose() {
    let args = vec!["memexia", "-v", "status"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(cli.verbose);
}

#[test]
fn test_cli_parse_graph() {
    let args = vec!["memexia", "graph", "stats"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::Graph(_)));
}

#[test]
fn test_cli_parse_search() {
    let args = vec!["memexia", "search", "test query"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Search(search_args) = cli.command {
        assert_eq!(search_args.query, "test query");
    }
}

#[test]
fn test_cli_parse_sync() {
    let args = vec!["memexia", "sync"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::Sync(_)));
}

#[test]
fn test_cli_parse_config() {
    let args = vec!["memexia", "config", "user.name", "Test User"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Config(config_args) = cli.command {
        assert_eq!(config_args.key, Some("user.name".to_string()));
        assert_eq!(config_args.value, Some("Test User".to_string()));
    }
}

#[test]
fn test_cli_parse_reindex() {
    let args = vec!["memexia", "reindex"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::Reindex(_)));
}

#[test]
fn test_cli_parse_file() {
    let args = vec!["memexia", "file", "info", "test.md"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::File(_)));
}

#[test]
fn test_cli_parse_link() {
    let args = vec!["memexia", "link", "query"];
    let cli = Cli::try_parse_from(&args).unwrap();
    assert!(matches!(cli.command, Commands::Link(_)));
}

#[test]
fn test_cli_parse_serve() {
    let args = vec!["memexia", "serve", "--port", "8080"];
    let cli = Cli::try_parse_from(&args).unwrap();
    if let Commands::Serve(serve_args) = cli.command {
        assert_eq!(serve_args.port, 8080);
    }
}
