use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use crate::storage::{Storage, Node, NodeType, Edge, RelationType};
use crate::core::{object, parser};

pub struct Repository {
    root: PathBuf,
    storage: Storage,
}

impl Repository {
    pub fn init(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        #[cfg(windows)]
        let root = {
            let p = root.to_string_lossy();
            // 移除 Windows 长路径前缀 \\?\
            if let Some(without_prefix) = p.strip_prefix("\\\\?\\") {
                PathBuf::from(without_prefix)
            } else {
                root
            }
        };

        if root.join(".memexia").exists() {
            anyhow::bail!("Repository already exists at {:?}", root);
        }

        let storage = Storage::init(&root)?;

        Ok(Self {
            root,
            storage,
        })
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut current = Some(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));

        #[cfg(windows)]
        {
            // 规范化 Windows 路径，移除 \\?\ 前缀
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

        Ok(Self {
            root,
            storage,
        })
    }

    /// 获取存储后端
    ///
    /// # Returns
    ///
    /// 存储后端引用
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

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
            // Make path relative to root
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

    pub fn commit(&self, message: &str) -> Result<()> {
        let index_path = self.root.join(".memexia/index");
        if !index_path.exists() {
            anyhow::bail!("Nothing to commit");
        }

        let content = fs::read_to_string(&index_path)?;
        if content.is_empty() {
            anyhow::bail!("Nothing to commit");
        }

        let index: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        for path_str in index {
            let path = self.root.join(&path_str);
            if !path.exists() {
                continue;
            }

            let file_content = fs::read(&path)?;
            let _hash = object::write_object(&self.root, &file_content)?;

            let content_str = String::from_utf8_lossy(&file_content);
            let parsed = parser::parse_markdown(&content_str, &path_str);

            // Create node in graph
            let node = parsed.to_node();
            self.storage.graph().add_node(&node)?;

            // Create edges for links
            for link in &parsed.wiki_links {
                let target_id = format!("urn:memexia:file:{}", link.target.replace(" ", "_"));

                // Check if target node exists, if not create it
                if !self.storage.graph().node_exists(&target_id)? {
                    let target_node = Node::new(&target_id, NodeType::Concept, &link.target);
                    self.storage.graph().add_node(&target_node)?;
                }

                let edge = link.to_edge(&node.id);
                self.storage.graph().add_edge(&edge)?;
            }
        }

        println!("Committed with message: {}", message);

        // Clear index
        fs::File::create(index_path)?;

        Ok(())
    }

    pub fn query_graph(&self, _query: &str) -> Result<Vec<String>> {
        // SPARQL 查询暂不支持内存存储
        // 返回所有节点作为结果
        let nodes = self.storage.graph().list_nodes()?;
        let mut rows = Vec::new();

        for node in nodes {
            rows.push(format!("Node: {} - {}", node.id, node.title));
        }

        Ok(rows)
    }
}
