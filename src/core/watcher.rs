//! 文件监听模块
//!
//! 使用 notify 库监听文件系统变化
//!
//! ## 使用方法
//!
//! ```rust,ignore
//! use memexia::core::{FileWatcher, FileWatcherConfig};
//! use std::path::Path;
//!
//! let config = FileWatcherConfig::default();
//! let mut watcher = FileWatcher::new(config).unwrap();
//!
//! // 监听目录
//! watcher.watch(Path::new("/path/to/notes")).unwrap();
//!
//! // 注意：run_watcher() 会阻塞当前线程
//! // watcher.run().unwrap();
//! ```

use crate::core::watch_config::WatchConfig;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// 文件变化事件类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileEvent {
    /// 文件被创建
    Created(String),
    /// 文件被修改
    Modified(String),
    /// 文件被删除
    Deleted(String),
    /// 文件被重命名
    Renamed(String, String),
}

impl FileEvent {
    /// 获取事件涉及的路径
    pub fn path(&self) -> &str {
        match self {
            FileEvent::Created(p) => p,
            FileEvent::Modified(p) => p,
            FileEvent::Deleted(p) => p,
            FileEvent::Renamed(_, p) => p,
        }
    }
}

/// 文件监听器配置
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    /// 监听配置
    pub watch_config: WatchConfig,
    /// 事件去重窗口（毫秒）
    pub debounce_ms: u64,
    /// 是否递归监听子目录
    pub recursive: bool,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            watch_config: WatchConfig::new(),
            debounce_ms: 100,
            recursive: true,
        }
    }
}

/// 文件监听器
///
/// 包装 notify 库，提供文件变化事件流
pub struct FileWatcher {
    config: FileWatcherConfig,
    /// 事件发送端
    tx: mpsc::Sender<FileEvent>,
    /// 事件接收端
    rx: mpsc::Receiver<FileEvent>,
    /// 监听器实例
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// 创建新的文件监听器
    ///
    /// # Arguments
    ///
    /// * `config` - 监听器配置
    ///
    /// # Returns
    ///
    /// 监听器实例
    pub fn new(config: FileWatcherConfig) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let tx_clone = tx.clone();
        let config_clone = config.clone();

        // 创建 watcher
        let watcher = RecommendedWatcher::new(
            move |result: notify::Result<notify::Event>| {
                match result {
                    Ok(event) => {
                        // 过滤并转换事件
                        if let Some(file_event) = convert_event(&event, &config_clone.watch_config) {
                            let _ = tx_clone.send(file_event);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Watch error: {:?}", e);
                    }
                }
            },
            notify::Config::default()
                .with_poll_interval(Duration::from_millis(config.debounce_ms))
                .with_compare_contents(true),
        )?;

        Ok(Self {
            config,
            tx,
            rx,
            _watcher: watcher,
        })
    }

    /// 创建使用默认配置的监听器
    ///
    /// # Returns
    ///
    /// 监听器实例
    pub fn with_defaults() -> notify::Result<Self> {
        Self::new(FileWatcherConfig::default())
    }

    /// 开始监听目录
    ///
    /// # Arguments
    ///
    /// * `path` - 要监听的目录路径
    ///
    /// # Returns
    ///
    /// 操作结果
    pub fn watch(&mut self, path: &Path) -> notify::Result<()> {
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self._watcher.watch(path, mode)?;
        tracing::info!("Started watching: {:?}", path);
        Ok(())
    }

    /// 停止监听
    pub fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self._watcher.unwatch(path)?;
        tracing::info!("Stopped watching: {:?}", path);
        Ok(())
    }

    /// 获取事件接收端
    ///
    /// # Returns
    ///
    /// 事件接收端
    pub fn receiver(&self) -> &mpsc::Receiver<FileEvent> {
        &self.rx
    }
}

/// 将 notify 事件转换为内部事件类型
fn convert_event(event: &notify::Event, config: &WatchConfig) -> Option<FileEvent> {
    // 获取主要路径
    let get_path = |path: &Path| -> Option<String> {
        if !config.is_allowed(path) {
            return None;
        }
        Some(path.to_string_lossy().into_owned())
    };

    match event.kind {
        notify::EventKind::Create(_) => {
            if let Some(path) = event.paths.first() {
                get_path(path).map(FileEvent::Created)
            } else {
                None
            }
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
            if let Some(path) = event.paths.first() {
                get_path(path).map(FileEvent::Modified)
            } else {
                None
            }
        }
        notify::EventKind::Remove(_) => {
            if let Some(path) = event.paths.first() {
                get_path(path).map(FileEvent::Deleted)
            } else {
                None
            }
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::To)) => {
            // 重命名事件 - notify 6.x 使用这种方式
            if event.paths.len() >= 2 {
                let from = &event.paths[0];
                let to = &event.paths[1];
                get_path(to).map(|to_path| FileEvent::Renamed(
                    from.to_string_lossy().into_owned(),
                    to_path,
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 运行监听循环
///
/// 阻塞并处理文件变化事件
///
/// # Arguments
///
/// * `watcher` - 文件监听器
/// * `handler` - 事件处理函数
pub fn run_watcher<F>(watcher: &FileWatcher, mut handler: F)
where
    F: FnMut(FileEvent),
{
    let rx = watcher.receiver();

    for event in rx.iter() {
        handler(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let config = FileWatcherConfig::default();
        let watcher = FileWatcher::new(config).unwrap();
        assert!(watcher.receiver().try_recv().is_err()); // 初始应该没有事件
    }

    #[test]
    fn test_watcher_watch() {
        let temp_dir = TempDir::new().unwrap();
        let config = FileWatcherConfig::default();
        let mut watcher = FileWatcher::new(config).unwrap();

        // 监听临时目录
        watcher.watch(temp_dir.path()).unwrap();

        // 等待一小会儿让 watcher 初始化
        std::thread::sleep(Duration::from_millis(50));

        // 清理
        let _ = watcher.unwatch(temp_dir.path());
    }

    #[test]
    fn test_file_event_path() {
        let event = FileEvent::Created("/path/to/file.md".to_string());
        assert_eq!(event.path(), "/path/to/file.md");

        let event = FileEvent::Renamed("old.md".to_string(), "new.md".to_string());
        assert_eq!(event.path(), "new.md");
    }

    #[test]
    fn test_convert_event_created() {
        let config = WatchConfig::new();
        let event = notify::Event {
            kind: notify::EventKind::Create(notify::event::CreateKind::File),
            paths: vec![std::path::Path::new("notes/test.md").to_path_buf()],
            ..Default::default()
        };

        let result = convert_event(&event, &config);
        assert!(matches!(result, Some(FileEvent::Created(_))));
        assert_eq!(result.unwrap().path(), "notes/test.md");
    }

    #[test]
    fn test_convert_event_blacklisted() {
        let config = WatchConfig::new();
        let event = notify::Event {
            kind: notify::EventKind::Create(notify::event::CreateKind::File),
            paths: vec![std::path::Path::new(".git/config").to_path_buf()],
            ..Default::default()
        };

        let result = convert_event(&event, &config);
        assert!(result.is_none()); // 黑名单中的文件应该被过滤
    }
}
