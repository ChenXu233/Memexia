//! Git 引擎封装
//!
//! 使用 libgit2 (git2 crate) 内嵌 Git 功能

use std::path::{Path, PathBuf};
use anyhow::{Result, Context, anyhow};
use git2::{Repository, Oid, Signature};
use chrono::{DateTime, Utc, TimeZone};
use crate::vcs::CommitInfo;

/// Git 引擎
pub struct GitEngine {
    /// Git 仓库
    repo: Repository,
    /// 仓库路径
    path: PathBuf,
}

impl GitEngine {
    /// 初始化新的 Git 仓库
    pub fn init(root: &Path) -> Result<Self> {
        let git_path = root.join(".git");

        // 如果 .git 不存在，则初始化
        if !git_path.exists() {
            Repository::init(root)
                .with_context(|| format!("Failed to init git repo at {:?}", root))?;
        }

        let repo = Repository::open(root)
            .with_context(|| format!("Failed to open git repo at {:?}", root))?;

        Ok(Self {
            repo,
            path: root.to_path_buf(),
        })
    }

    /// 打开已存在的 Git 仓库
    pub fn open(root: &Path) -> Result<Self> {
        let repo = Repository::open(root)
            .with_context(|| format!("Failed to open git repo at {:?}", root))?;

        Ok(Self {
            repo,
            path: root.to_path_buf(),
        })
    }

    /// 获取仓库路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取 Git 配置
    pub fn config(&self) -> Result<git2::Config> {
        self.repo.config().map_err(|e| anyhow!("Failed to get git config: {}", e))
    }

    /// 暂存文件
    pub fn add(&self, files: &[PathBuf]) -> Result<()> {
        let mut index = self.repo.index()?;

        // 添加文件到索引
        let paths: Vec<_> = files.iter()
            .map(|f| {
                // 转换为相对路径
                if let Ok(rel) = f.strip_prefix(&self.path) {
                    rel.to_string_lossy().into_owned()
                } else {
                    f.to_string_lossy().into_owned()
                }
            })
            .collect();

        if paths.is_empty() {
            return Ok(());
        }

        index.add_all(&paths, git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        Ok(())
    }

    /// 创建提交
    pub fn commit(&self, message: &str, author: &str) -> Result<Oid> {
        let signature = parse_signature(author)?;
        let tree_oid = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;
        let parent = self.get_head_oid();

        let commit = if let Some(parent_oid) = parent {
            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&self.repo.find_commit(parent_oid)?],
            )?
        } else {
            // 首次提交
            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[],
            )?
        };

        Ok(commit)
    }

    /// 修改最后一次提交
    pub fn amend(&self, message: &str, author: &str) -> Result<Oid> {
        let signature = parse_signature(author)?;

        // 获取当前 HEAD 提交
        let head_commit = self.repo.head()?.peel_to_commit()?;
        let tree_oid = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(tree_oid)?;

        // 获取父提交（用于新提交）
        let parents: Vec<_> = head_commit.parents().collect();

        // 获取分支名称
        let branch_name = if self.repo.head()?.is_branch() {
            self.repo.head()?.name().map(|n| {
                n.strip_prefix("refs/heads/").unwrap_or(n).to_string()
            })
        } else {
            None
        };

        // 创建新提交，不通过 HEAD 而是通过直接引用
        let commit = if let Some(first_parent) = parents.first() {
            // 创建新提交，parent 是父提交
            self.repo.commit(
                None, // 不通过 HEAD
                &signature,
                &signature,
                message,
                &tree,
                &[first_parent],
            )?
        } else {
            // 首次提交的情况
            self.repo.commit(
                None, // 不通过 HEAD
                &signature,
                &signature,
                message,
                &tree,
                &[],
            )?
        };

        // 更新分支引用指向新提交
        if let Some(name) = branch_name {
            let branch_ref = format!("refs/heads/{}", name);
            // 删除旧分支引用
            if let Ok(mut ref_) = self.repo.find_reference(&branch_ref) {
                ref_.delete()?;
            }
            // 创建新的分支引用指向新提交
            self.repo.reference(&branch_ref, commit, true, "amend")?;
            // 更新 HEAD
            self.repo.set_head(&branch_ref)?;
        } else {
            // 如果没有分支，设置 HEAD 为 detached
            self.repo.set_head_detached(commit)?;
        }

        Ok(commit)
    }

    /// 获取提交历史
    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        let mut commits = Vec::new();
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push_head()?;
        // 使用 TIME 排序（从新到旧），因为 revwalk 默认从 HEAD 向前追溯
        revwalk.set_sorting(git2::Sort::TIME)?;

        for oid in revwalk.take(limit) {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;

            let timestamp = format_timestamp(commit.time());

            commits.push(CommitInfo {
                oid: oid.to_string(),
                message: commit.message().unwrap_or("").to_string(),
                graph_hash: None, // 需要从外部补充
                author: commit.author().to_string(),
                timestamp,
            });
        }

        Ok(commits)
    }

    /// 获取 HEAD OID
    pub fn get_head_oid(&self) -> Option<Oid> {
        Some(self.repo.head().ok()?.peel_to_commit().ok()?.id())
    }

    /// 获取当前 HEAD 的提交信息
    pub fn head_info(&self) -> Result<Option<CommitInfo>> {
        // 尝试获取 HEAD 引用，如果不存在（无提交）则返回 None
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(e) => {
                // 没有提交时，HEAD 不存在，返回 None
                if e.code() == git2::ErrorCode::UnbornBranch ||
                   e.code() == git2::ErrorCode::NotFound {
                    return Ok(None);
                }
                return Err(e.into());
            }
        };

        if !head.is_branch() {
            return Ok(None);
        }

        let commit = match head.peel_to_commit() {
            Ok(c) => c,
            Err(e) => {
                // 同样处理解引用失败的情况
                if e.code() == git2::ErrorCode::NotFound {
                    return Ok(None);
                }
                return Err(e.into());
            }
        };

        let oid = commit.id();
        let author = commit.author().to_string();
        let message = commit.message().unwrap_or("").to_string();
        let timestamp = format_timestamp(commit.time());

        Ok(Some(CommitInfo {
            oid: oid.to_string(),
            message,
            graph_hash: None,
            author,
            timestamp,
        }))
    }

    /// 获取父提交 OID
    pub fn get_parent_oids(&self) -> Vec<Oid> {
        if let Ok(commit) = self.repo.head().and_then(|h| h.peel_to_commit()) {
            commit.parents().map(|c| c.id()).collect()
        } else {
            Vec::new()
        }
    }

    /// 检查是否有未提交的变更
    pub fn has_changes(&self) -> Result<bool> {
        let status = self.repo.statuses(None)?;
        Ok(!status.is_empty())
    }

    /// 获取状态信息
    pub fn status(&self) -> Result<String> {
        let statuses = self.repo.statuses(None)?;

        if statuses.is_empty() {
            return Ok("nothing to commit, working tree clean".to_string());
        }

        let mut output = String::new();
        for s in statuses.iter() {
            if let Some(path) = s.path() {
                output.push_str(&format!("{:?} {}\n", s.status(), path));
            }
        }

        Ok(output.trim().to_string())
    }
}

/// 格式化 git2::Time 为字符串
fn format_timestamp(time: git2::Time) -> String {
    let datetime: DateTime<Utc> = Utc.timestamp_opt(time.seconds(), 0).single().unwrap_or_default();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 解析作者字符串为 Signature
fn parse_signature(author: &str) -> Result<Signature<'static>> {
    // 使用当前时间
    let now = chrono::Utc::now();
    let time = git2::Time::new(now.timestamp(), 0);

    // 尝试解析 "Name <email>" 格式
    if let Some((name, email)) = author.split_once('<') {
        let name = name.trim();
        let email = email.trim_end_matches('>');
        return Ok(Signature::new(name, email, &time)?);
    }

    // 尝试直接使用
    Ok(Signature::new(author, author, &time)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_init_and_open() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        // 初始化
        let _git = GitEngine::init(path).unwrap();
        assert!(path.join(".git").exists());

        // 打开
        let git2 = GitEngine::open(path).unwrap();
        assert_eq!(git2.path(), path);
    }

    #[test]
    fn test_commit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        // 初始化
        let git = GitEngine::init(path).unwrap();

        // 创建一个文件
        fs::write(path.join("test.txt"), "hello").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();

        // 提交
        let oid = git.commit("Initial commit", "Test <test@example.com>").unwrap();
        assert!(!oid.to_string().is_empty());

        // 检查历史
        let log = git.log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].message.contains("Initial"));
    }

    #[test]
    fn test_multiple_commits() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 第一次提交
        fs::write(path.join("a.txt"), "content a").unwrap();
        git.add(&[path.join("a.txt")]).unwrap();
        git.commit("Add a.txt", "Test <test@example.com>").unwrap();

        // 第二次提交
        fs::write(path.join("b.txt"), "content b").unwrap();
        git.add(&[path.join("b.txt")]).unwrap();
        git.commit("Add b.txt", "Test <test@example.com>").unwrap();

        // 检查历史 - 最先提交的应该在最后（ oldest first with REVERSE）
        let log = git.log(10).unwrap();
        assert_eq!(log.len(), 2);
        // REVERSE 排序下，最新的提交在前
        assert!(log[0].message.contains("Add b.txt"));
        assert!(log[1].message.contains("Add a.txt"));
    }

    #[test]
    fn test_amend() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 创建并提交
        fs::write(path.join("test.txt"), "content").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();
        let original_oid = git.commit("Original message", "Test <test@example.com>").unwrap();

        // 修改提交 - 使用 amend 方法
        let amended_oid = git.amend("Amended message", "Test <test@example.com>").unwrap();

        // OID 应该不同
        assert_ne!(original_oid, amended_oid);

        // 历史中应该只有一条（被替换）
        let log = git.log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].message.contains("Amended"));
    }

    #[test]
    fn test_log_limit() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 创建多个提交
        for i in 0..5 {
            fs::write(path.join(format!("{}.txt", i)), format!("content {}", i)).unwrap();
            git.add(&[path.join(format!("{}.txt", i))]).unwrap();
            git.commit(&format!("Commit {}", i), "Test <test@example.com>").unwrap();
        }

        // 限制为 3 条
        let log = git.log(3).unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_head_info() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 初始状态（无提交）
        let head = git.head_info().unwrap();
        assert!(head.is_none());

        // 创建提交
        fs::write(path.join("test.txt"), "content").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();
        git.commit("Test commit", "Test <test@example.com>").unwrap();

        // 检查 HEAD 信息
        let head = git.head_info().unwrap();
        assert!(head.is_some());
        let head = head.unwrap();
        assert!(head.message.contains("Test commit"));
        assert!(head.author.contains("Test"));
        assert!(!head.oid.is_empty());
    }

    #[test]
    fn test_get_head_oid() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 初始无 HEAD
        assert!(git.get_head_oid().is_none());

        // 创建提交
        fs::write(path.join("test.txt"), "content").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();
        git.commit("Test", "Test <test@example.com>").unwrap();

        // 应该有 HEAD
        assert!(git.get_head_oid().is_some());
    }

    #[test]
    fn test_status() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 初始干净状态
        let status = git.status().unwrap();
        assert!(status.contains("clean"));

        // 添加文件（未提交）
        fs::write(path.join("new.txt"), "content").unwrap();
        git.add(&[path.join("new.txt")]).unwrap();

        let status = git.status().unwrap();
        assert!(status.contains("new.txt"));
    }

    #[test]
    fn test_author_format() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 标准格式
        fs::write(path.join("a.txt"), "content").unwrap();
        git.add(&[path.join("a.txt")]).unwrap();
        let oid = git.commit("Test", "User Name <user@example.com>").unwrap();
        assert!(!oid.to_string().is_empty());

        // 简化格式
        fs::write(path.join("b.txt"), "content").unwrap();
        git.add(&[path.join("b.txt")]).unwrap();
        let oid = git.commit("Test", "SimpleUser").unwrap();
        assert!(!oid.to_string().is_empty());
    }

    #[test]
    fn test_get_parent_oids() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        // 无提交时
        assert!(git.get_parent_oids().is_empty());

        // 首次提交 - 没有父提交
        fs::write(path.join("test.txt"), "content").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();
        git.commit("First commit", "Test <test@example.com>").unwrap();

        // 首次提交的父提交数量为 0
        assert_eq!(git.get_parent_oids().len(), 0);

        // 第二次提交 - 有 1 个父提交
        fs::write(path.join("test2.txt"), "content2").unwrap();
        git.add(&[path.join("test2.txt")]).unwrap();
        git.commit("Second commit", "Test <test@example.com>").unwrap();

        // 第二次提交应该有 1 个父提交
        let parents = git.get_parent_oids();
        assert_eq!(parents.len(), 1);
    }

    #[test]
    fn test_timestamp_format() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();
        let git = GitEngine::init(path).unwrap();

        fs::write(path.join("test.txt"), "content").unwrap();
        git.add(&[path.join("test.txt")]).unwrap();
        git.commit("Test", "Test <test@example.com>").unwrap();

        let log = git.log(1).unwrap();
        assert!(!log[0].timestamp.is_empty());
        // 验证时间格式 YYYY-MM-DD HH:MM:SS
        assert!(log[0].timestamp.contains("-"));
        assert!(log[0].timestamp.contains(":"));
    }
}
