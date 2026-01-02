pub mod repository;
pub mod object;
pub mod parser;
pub mod watcher;
pub mod indexer;
pub mod watch_config;

// 重新导出 repository 模块中的公共 API
pub use repository::Repository;

// 重新导出 parser 模块中的公共 API
pub use parser::{
    parse_markdown, quick_parse, ParsedDoc, WikiLink, Frontmatter,
    frontmatter::{parse_frontmatter, extract_frontmatter, has_frontmatter},
    wiki_link::{parse_wiki_links, remove_wiki_links, replace_wiki_links_with_text},
};

// 重新导出 watcher 模块中的公共 API
pub use watcher::{FileWatcher, FileWatcherConfig, FileEvent, run_watcher};

// 重新导出 indexer 模块中的公共 API
pub use indexer::{Indexer, IndexResult, IndexSummary};

// 重新导出 watch_config 模块中的公共 API
pub use watch_config::WatchConfig;
