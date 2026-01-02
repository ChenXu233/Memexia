//! 文件监听配置模块
//!
//! 管理文件监听的白名单和黑名单配置
//!
//! ## 配置格式
//!
//! ```json
//! {
//!   "whitelist": ["*.md", "notes/**/*"],
//!   "blacklist": [".git/**/*", "*.tmp"]
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 文件监听配置
///
/// 控制哪些文件需要被监听和索引
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchConfig {
    /// 白名单 glob 模式列表
    ///
    /// 只有匹配这些模式的文件才会被监听
    /// 空列表表示允许所有文件
    #[serde(default)]
    pub whitelist: Vec<String>,

    /// 黑名单 glob 模式列表
    ///
    /// 匹配这些模式的文件会被排除
    #[serde(default)]
    pub blacklist: Vec<String>,
}

impl WatchConfig {
    /// 创建新的默认配置
    ///
    /// 默认允许所有 .md 文件，排除 .git 和临时文件
    pub fn new() -> Self {
        Self {
            whitelist: vec!["*.md".to_string()],
            blacklist: vec![
                ".git/**/*".to_string(),
                ".memexia/**/*".to_string(),
                "*.tmp".to_string(),
                "*.bak".to_string(),
                ".DS_Store".to_string(),
            ],
        }
    }

    /// 从文件加载配置
    ///
    /// # Arguments
    ///
    /// * `path` - 配置文件路径
    ///
    /// # Returns
    ///
    /// 加载的配置，如果文件不存在或解析失败返回默认配置
    pub fn from_file(path: &Path) -> Self {
        if !path.exists() {
            return Self::new();
        }

        match std::fs::read_to_string(path) {
            Ok(content) => {
                serde_json::from_str(&content).unwrap_or_else(|_| Self::new())
            }
            Err(_) => Self::new(),
        }
    }

    /// 保存配置到文件
    ///
    /// # Arguments
    ///
    /// * `path` - 配置文件路径
    ///
    /// # Returns
    ///
    /// 操作结果
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 检查路径是否被允许
    ///
    /// 规则：
    /// 1. 如果白名单非空，文件必须匹配白名单中的一个模式
    /// 2. 文件不能匹配黑名单中的任何模式
    ///
    /// # Arguments
    ///
    /// * `path` - 要检查的文件路径
    ///
    /// # Returns
    ///
    /// 如果文件应该被监听返回 true
    pub fn is_allowed(&self, path: &Path) -> bool {
        // 检查黑名单
        if self.is_blacklisted(path) {
            return false;
        }

        // 如果白名单为空，允许所有文件
        if self.whitelist.is_empty() {
            return true;
        }

        // 检查白名单
        self.is_whitelisted(path)
    }

    /// 检查路径是否匹配白名单
    pub fn is_whitelisted(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        for pattern in &self.whitelist {
            if matches_pattern(pattern, &path_str) || matches_pattern(pattern, &file_name) {
                return true;
            }
        }

        false
    }

    /// 检查路径是否匹配黑名单
    pub fn is_blacklisted(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let file_name = path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        for pattern in &self.blacklist {
            // 如果模式包含路径分隔符，检查完整路径
            // 否则只检查文件名（避免目录名误匹配）
            if pattern.contains('/') || pattern.contains('\\') {
                if matches_pattern(pattern, &path_str) {
                    return true;
                }
            } else {
                // 纯文件名模式只检查文件名
                if matches_pattern(pattern, &file_name) {
                    return true;
                }
            }
        }

        false
    }

    /// 添加白名单模式
    ///
    /// # Arguments
    ///
    /// * `pattern` - glob 模式
    pub fn add_whitelist(&mut self, pattern: impl Into<String>) {
        self.whitelist.push(pattern.into());
    }

    /// 添加黑名单模式
    ///
    /// # Arguments
    ///
    /// * `pattern` - glob 模式
    pub fn add_blacklist(&mut self, pattern: impl Into<String>) {
        self.blacklist.push(pattern.into());
    }

    /// 清除白名单
    pub fn clear_whitelist(&mut self) {
        self.whitelist.clear();
    }

    /// 清除黑名单
    pub fn clear_blacklist(&mut self) {
        self.blacklist.clear();
    }
}

/// 简单的 glob 模式匹配
///
/// 支持 * 匹配任意字符（不包括路径分隔符）
/// 支持 ** 匹配任意字符（包括路径分隔符）
fn matches_pattern(pattern: &str, text: &str) -> bool {
    // 简单实现：处理 * 通配符
    let pattern = pattern.replace("**/", "*");

    if pattern.contains('*') {
        let regex_pattern = glob_to_regex(&pattern);
        regex::Regex::new(&regex_pattern)
            .ok()
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    } else {
        pattern == text
    }
}

/// 将 glob 模式转换为正则表达式
fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::with_capacity(pattern.len() * 2);

    for c in pattern.chars() {
        match c {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' => {
                regex.push('\\');
                regex.push(c);
            }
            c => regex.push(c),
        }
    }

    regex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_allowed_default_config() {
        let config = WatchConfig::new();

        // Markdown 文件应该被允许
        assert!(config.is_allowed(Path::new("notes/test.md")));
        assert!(config.is_allowed(Path::new("notes/philosophy/free_will.md")));

        // .git 目录应该被排除
        assert!(!config.is_allowed(Path::new(".git/config")));
        assert!(!config.is_allowed(Path::new("notes/.gitkeep")));

        // .tmp 文件应该被排除
        assert!(!config.is_allowed(Path::new("notes/tmp.md.tmp")));
    }

    #[test]
    fn test_is_allowed_with_whitelist() {
        let mut config = WatchConfig::new();
        config.clear_whitelist();
        config.add_whitelist("notes/*.md");
        config.add_whitelist("docs/**/*.md");

        assert!(config.is_allowed(Path::new("notes/test.md")));
        assert!(config.is_allowed(Path::new("docs/guide.md")));
        assert!(!config.is_allowed(Path::new("other.txt")));
    }

    #[test]
    fn test_is_allowed_with_blacklist() {
        let mut config = WatchConfig::new();
        config.clear_blacklist();
        config.add_blacklist("drafts/**/*");
        config.add_blacklist("*.bak");

        assert!(config.is_allowed(Path::new("notes/test.md")));
        assert!(!config.is_allowed(Path::new("drafts/idea.md")));
        assert!(!config.is_allowed(Path::new("notes/backup.md.bak")));
    }

    #[test]
    fn test_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("watch.json");

        // 写入自定义配置
        let custom_config = r#"{
            "whitelist": ["*.txt"],
            "blacklist": ["secret/**/*"]
        }"#;
        fs::write(&config_path, custom_config).unwrap();

        // 加载配置
        let config = WatchConfig::from_file(&config_path);

        assert!(config.is_whitelisted(Path::new("test.txt")));
        assert!(!config.is_whitelisted(Path::new("test.md")));
        assert!(config.is_blacklisted(Path::new("secret/file.txt")));
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("watch.json");

        let mut config = WatchConfig::new();
        config.add_whitelist("custom/*.md");
        config.add_blacklist("exclude/**/*");

        config.save(&config_path).unwrap();
        let loaded = WatchConfig::from_file(&config_path);

        assert!(loaded.is_allowed(Path::new("custom/test.md")));
        assert!(!loaded.is_allowed(Path::new("exclude/test.md")));
    }

    #[test]
    fn test_empty_whitelist_allows_all() {
        let mut config = WatchConfig::new();
        config.clear_whitelist();

        // 白名单为空时允许所有文件（除了黑名单）
        assert!(config.is_allowed(Path::new("notes/test.md")));
        assert!(config.is_allowed(Path::new("docs/guide.txt")));
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("*.md", "test.md"));
        assert!(matches_pattern("*.md", "document.md"));
        assert!(!matches_pattern("*.md", "test.txt"));

        assert!(matches_pattern("notes/*.md", "notes/test.md"));
        assert!(!matches_pattern("notes/*.md", "docs/test.md"));
    }
}
