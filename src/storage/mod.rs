use anyhow::{Context, Result};
use oxigraph::store::Store;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Memexia 仓库元数据
#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryMeta {
    pub version: String,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
}

impl Default for RepositoryMeta {
    fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            version: "0.1.0".to_string(),
            created_at: now.clone(),
            updated_at: now,
            name: "Untitled Repository".to_string(),
        }
    }
}

pub struct Storage {
    /// 仓库根路径
    root: PathBuf,
    /// Oxigraph 图数据库
    graph_store: Store,
}

impl Storage {
    /// 打开已有仓库
    pub fn open(root: &Path) -> Result<Self> {
        let graph_path = root.join(".memexia/graph");
        if !graph_path.exists() {
            anyhow::bail!("Not a valid Memexia repository: missing .memexia/graph");
        }
        let graph_store = Store::open(&graph_path)?;

        Ok(Self {
            root: root.to_path_buf(),
            graph_store,
        })
    }

    /// 初始化新仓库
    pub fn init(root: &Path) -> Result<Self> {
        let memexia_dir = root.join(".memexia");

        // 检查是否已存在
        if memexia_dir.exists() {
            anyhow::bail!("Memexia repository already exists at {:?}", root);
        }

        // 创建目录结构
        std::fs::create_dir_all(&memexia_dir)?;
        std::fs::create_dir_all(memexia_dir.join("graph"))?;
        std::fs::create_dir_all(memexia_dir.join("objects"))?;
        std::fs::create_dir_all(memexia_dir.join("config"))?;
        std::fs::create_dir_all(memexia_dir.join("index"))?;

        // 创建 notes 目录
        std::fs::create_dir_all(root.join("notes"))?;

        // 创建 meta.json 元数据文件
        let meta = RepositoryMeta::default();
        let meta_path = memexia_dir.join("meta.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("Failed to write {:?}", meta_path))?;

        // 创建空的 N-Quads 图文件 (符合验收标准)
        let nq_path = memexia_dir.join("graph.nq");
        std::fs::write(&nq_path, "").with_context(|| format!("Failed to create {:?}", nq_path))?;

        // 初始化 Oxigraph 存储
        let graph_store = Store::open(memexia_dir.join("graph").as_path())?;

        Ok(Self {
            root: root.to_path_buf(),
            graph_store,
        })
    }

    /// 获取图数据库引用
    pub fn get_graph(&self) -> &Store {
        &self.graph_store
    }

    /// 获取仓库根路径
    pub fn root(&self) -> &Path {
        &self.root
    }
}
