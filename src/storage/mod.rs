use anyhow::Result;
use oxigraph::store::Store;
use std::path::{Path, PathBuf};

pub struct Storage {
    root: PathBuf,
    graph_store: Store,
}

impl Storage {
    pub fn open(root: &Path) -> Result<Self> {
        let graph_path = root.join(".memexia/graph");
        std::fs::create_dir_all(&graph_path)?;
        let graph_store = Store::open(&graph_path)?;
        
        Ok(Self {
            root: root.to_path_buf(),
            graph_store,
        })
    }

    pub fn init(root: &Path) -> Result<Self> {
        let memexia_dir = root.join(".memexia");
        std::fs::create_dir_all(memexia_dir.join("graph"))?;
        std::fs::create_dir_all(memexia_dir.join("objects"))?;
        std::fs::create_dir_all(memexia_dir.join("config"))?;
        
        // Create notes directory if it doesn't exist
        std::fs::create_dir_all(root.join("notes"))?;

        Self::open(root)
    }
    
    pub fn get_graph(&self) -> &Store {
        &self.graph_store
    }
}
