//! 仓库管理模块
//!
//! 管理 Memexia 仓库的生命周期

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use crate::storage::{Storage, Node, NodeType};
use crate::core::{object, parser};
use crate::vcs::{Vcs, CommitInfo};

/// Memexia 仓库
pub struct Repository {
    /// 仓库根路径
    root: PathBuf,
    /// 存储后端
    storage: Storage,
    /// 版本控制
    vcs: Vcs,
}

impl Repository {
    /// 初始化新仓库
    pub fn init(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        #[cfg(windows)]
        let root = {
            let p = root.to_string_lossy();
            if let Some(without_prefix) = p.strip_prefix("\\\\?\\") {
                PathBuf::from(without_prefix)
            } else {
                root
            }
        };

        if root.join(".memexia").exists() {
            anyhow::bail!("Repository already exists at {:?}", root);
        }

        // 初始化存储
        let storage = Storage::init(&root)?;

        // 初始化 VCS（同时初始化 Git 仓库）
        let vcs = Vcs::init(&root)?;

        Ok(Self {
            root,
            storage,
            vcs,
        })
    }

    /// 打开已有仓库
    pub fn open(path: &Path) -> Result<Self> {
        let mut current = Some(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));

        #[cfg(windows)]
        {
            if let Some(ref mut p) = current {
                let p_str = p.to_string_lossy();
                if let Some(without_prefix) = p_str.strip_prefix("\\\\?\\") {
                    *p = PathBuf::from(without_prefix);
                }
            }
        }

        // 向上查找 .memexia 目录
        let mut found_root: Option<PathBuf> = None;
        while let Some(path) = current {
            if path.join(".memexia").exists() {
                found_root = Some(path);
                break;
            }
            current = path.parent().map(|p| p.to_path_buf());
        }

        let root = match found_root {
            Some(r) => r,
            None => anyhow::bail!(
                "Not a Memexia repository (or any of the parent directories): .memexia"
            ),
        };

        let storage = Storage::open(&root)?;
        let vcs = Vcs::open(&root)?;

        Ok(Self {
            root,
            storage,
            vcs,
        })
    }

    /// 获取存储后端
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// 获取版本控制
    pub fn vcs(&self) -> &Vcs {
        &self.vcs
    }

    /// 获取仓库根路径
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// 添加文件到暂存区
    pub fn add(&self, files: &[PathBuf]) -> Result<()> {
        let index_path = self.root.join(".memexia/index");
        let mut index = if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            content.lines().map(|s| s.to_string()).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        for file in files {
            let abs_path = fs::canonicalize(file).context("File not found")?;
            let rel_path = pathdiff::diff_paths(&abs_path, &self.root)
                .context("File is outside repository")?;

            let path_str = rel_path.to_string_lossy().to_string();
            if !index.contains(&path_str) {
                index.push(path_str);
            }
        }

        let mut file = fs::File::create(index_path)?;
        for line in index {
            writeln!(file, "{}", line)?;
        }

        Ok(())
    }

    /// 查看暂存区状态
    pub fn status(&self) -> Result<String> {
        let index_path = self.root.join(".memexia/index");
        if !index_path.exists() {
            return Ok("No changes staged.".to_string());
        }

        let content = fs::read_to_string(index_path)?;
        if content.is_empty() {
            return Ok("No changes staged.".to_string());
        }

        Ok(format!("Staged files:\n{}", content))
    }

    /// 提交变更
    ///
    /// 流程：
    /// 1. 读取暂存区文件列表
    /// 2. 处理每个文件，更新图数据库
    /// 3. 调用 VCS 创建 Git 提交
    /// 4. 记录图历史
    pub fn commit(&mut self, message: &str) -> Result<String> {
        let index_path = self.root.join(".memexia/index");
        if !index_path.exists() {
            anyhow::bail!("Nothing to commit");
        }

        let content = fs::read_to_string(&index_path)?;
        if content.is_empty() {
            anyhow::bail!("Nothing to commit");
        }

        let index: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // 收集要提交的文件路径
        let mut files: Vec<PathBuf> = Vec::new();

        for path_str in &index {
            let path = self.root.join(path_str);
            if !path.exists() {
                continue;
            }

            files.push(path.clone());

            let file_content = fs::read(&path)?;
            let _hash = object::write_object(&self.root, &file_content)?;

            let content_str = String::from_utf8_lossy(&file_content);
            let parsed = parser::parse_markdown(&content_str, path_str);

            // Create node in graph
            let node = parsed.to_node();
            self.storage.graph().add_node(&node)?;

            // Create edges for links
            for link in &parsed.wiki_links {
                let target_id = format!("urn:memexia:file:{}", link.target.replace(" ", "_"));

                if !self.storage.graph().node_exists(&target_id)? {
                    let target_node = Node::new(&target_id, NodeType::Concept, &link.target);
                    self.storage.graph().add_node(&target_node)?;
                }

                let edge = link.to_edge(&node.id);
                self.storage.graph().add_edge(&edge)?;
            }
        }

        // 调用 VCS 提交
        let commit_hash = self.vcs.commit(message, &files, &self.storage)?;

        println!("Committed: {}", commit_hash);

        // Clear index
        fs::File::create(index_path)?;

        Ok(commit_hash)
    }

    /// 修改最后一次提交
    pub fn amend(&mut self, message: &str) -> Result<()> {
        self.vcs.amend(message, &self.storage)?;
        println!("Commit amended successfully");
        Ok(())
    }

    /// 查看提交历史
    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        let mut commits = self.vcs.log(limit)?;

        // 补充图快照哈希信息
        for commit in &mut commits {
            if let Ok(Some(graph_hash)) = self.vcs.graph_history.get_commit_graph_hash(&commit.oid) {
                commit.graph_hash = Some(graph_hash);
            }
        }

        Ok(commits)
    }

    /// 查看最后一次提交
    pub fn last_commit(&self) -> Result<Option<CommitInfo>> {
        let mut commit = self.vcs.head_info()?;

        if let Some(ref mut c) = commit {
            if let Ok(Some(graph_hash)) = self.vcs.graph_history.get_commit_graph_hash(&c.oid) {
                c.graph_hash = Some(graph_hash);
            }
        }

        Ok(commit)
    }

    /// SPARQL 查询
    pub fn query_graph(&self, _query: &str) -> Result<Vec<String>> {
        let nodes = self.storage.graph().list_nodes()?;
        let mut rows = Vec::new();

        for node in nodes {
            rows.push(format!("Node: {} - {}", node.id, node.title));
        }

        Ok(rows)
    }

    /// 导出图为 N-Quads
    pub fn export_nquads(&self) -> Result<String> {
        self.storage.graph().export_nquads()
    }

    /// 获取图历史
    pub fn graph_history(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let entries = self.vcs.graph_history.get_history(limit)?;
        Ok(entries.iter()
            .map(|e| (e.commit_hash.clone(), e.graph_hash.clone()))
            .collect())
    }
}
