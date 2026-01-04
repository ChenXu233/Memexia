//! 版本控制模块
//!
//! 混合架构：使用 libgit2 管理文件版本，使用自定义系统追踪图历史

pub mod git_engine;
pub mod graph_history;
pub mod rollback;

pub use graph_history::{
    GraphHistory,
    NodeSnapshot,
    NodeHistoryEntry,
    DerivationEntry,
};

pub use rollback::{
    RollbackManager,
    RollbackResult,
    RollbackPreview,
};

use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::storage::Storage;

/// 版本控制管理器
pub struct Vcs {
    /// Git 引擎
    git: git_engine::GitEngine,
    /// 图历史追踪器（对外可见以支持历史查询）
    pub graph_history: graph_history::GraphHistory,
}

impl Vcs {
    /// 初始化新的 Vcs 实例（同时初始化 Git 仓库）
    pub fn init(root: &Path) -> Result<Self> {
        let git = git_engine::GitEngine::init(root)?;
        let graph_history = graph_history::GraphHistory::init(root)?;

        Ok(Self {
            git,
            graph_history,
        })
    }

    /// 打开已存在的 Vcs 实例
    pub fn open(root: &Path) -> Result<Self> {
        let git = git_engine::GitEngine::open(root)?;
        let graph_history = graph_history::GraphHistory::open(root)?;

        Ok(Self {
            git,
            graph_history,
        })
    }

    /// 提交变更
    ///
    /// 流程：
    /// 1. 导出当前图为 N-Quads 并计算哈希
    /// 2. 使用 git2 暂存文件
    /// 3. 创建 Git 提交
    /// 4. 记录图历史
    /// 5. 返回提交 OID
    pub fn commit(
        &mut self,
        message: &str,
        files: &[PathBuf],
        storage: &Storage,
    ) -> Result<String> {
        // 1. 导出图快照并计算哈希
        let graph_hash = self.graph_history.snapshot(storage)?;

        // 2. git add 暂存文件
        self.git.add(files)?;

        // 3. git commit 创建提交
        let author = self.get_default_author()?;
        let oid = self.git.commit(message, &author)?;

        // 4. 记录图历史（提交哈希关联图快照哈希）
        self.graph_history.record(&oid.to_string(), &graph_hash)?;

        // 5. 返回 commit hash
        Ok(oid.to_string())
    }

    /// 修改最后一次提交
    pub fn amend(&mut self, message: &str, storage: &Storage) -> Result<()> {
        // 导出新的图快照
        let graph_hash = self.graph_history.snapshot(storage)?;

        // git commit --amend
        let author = self.get_default_author()?;
        let oid = self.git.amend(message, &author)?;

        // 更新图历史引用
        self.graph_history.record(&oid.to_string(), &graph_hash)?;

        Ok(())
    }

    /// 获取提交历史
    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        self.git.log(limit)
    }

    /// 获取当前 HEAD 的提交信息
    pub fn head_info(&self) -> Result<Option<CommitInfo>> {
        self.git.head_info()
    }

    /// 获取默认作者信息
    fn get_default_author(&self) -> Result<String> {
        // 尝试从 git config 读取用户信息
        if let Ok(config) = self.git.config() {
            if let (Ok(name), Ok(email)) = (
                config.get_string("user.name"),
                config.get_string("user.email"),
            ) {
                return Ok(format!("{} <{}>", name, email));
            }
        }

        // 默认值
        Ok("Memexia User <user@memexia.local>".to_string())
    }
}

/// 提交信息
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// Git OID
    pub oid: String,
    /// 提交消息
    pub message: String,
    /// 图快照哈希
    pub graph_hash: Option<String>,
    /// 作者
    pub author: String,
    /// 时间戳
    pub timestamp: String,
}

impl CommitInfo {
    /// 简短格式（用于 --oneline）
    pub fn to_short(&self) -> String {
        let short_oid = &self.oid[..7];
        format!("{} - {}", short_oid, self.message)
    }
}
