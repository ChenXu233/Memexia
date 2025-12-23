use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use crate::storage::Storage;
use crate::core::{object, parser};
use oxigraph::model::*;

pub struct Repository {
    root: PathBuf,
    storage: Storage,
}

impl Repository {
    pub fn init(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        #[cfg(windows)]
        let root = {
            let p = root.to_string_lossy().to_string();
            if p.starts_with("\\\\?\\") {
                PathBuf::from(&p[4..])
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
        let root = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        #[cfg(windows)]
        let root = {
            let p = root.to_string_lossy().to_string();
            if p.starts_with("\\\\?\\") {
                PathBuf::from(&p[4..])
            } else {
                root
            }
        };

        if !root.join(".memexia").exists() {
            anyhow::bail!("Not a Memexia repository (or any of the parent directories): .memexia");
        }
        
        let storage = Storage::open(&root)?;
        
        Ok(Self {
            root,
            storage,
        })
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
        let graph = self.storage.get_graph();

        for path_str in index {
            let path = self.root.join(&path_str);
            if !path.exists() {
                continue; 
            }
            
            let content = fs::read(&path)?;
            let hash = object::write_object(&self.root, &content)?;
            
            let content_str = String::from_utf8_lossy(&content);
            let parsed = parser::parse_content(&content_str);
            
            // Update Graph
            // Use a simple URI scheme: urn:memexia:file:<path> relative to repo root
            let subject_uri = format!("urn:memexia:file:{}", path_str.replace("\\", "/"));
            let subject = NamedNode::new(subject_uri)?;
            
            println!("Inserting triples for {}", path_str);

            // Add title
            if let Some(title) = parsed.title {
                graph.insert(&Quad::new(
                    subject.clone(),
                    NamedNode::new("http://purl.org/dc/elements/1.1/title")?,
                    Literal::new_simple_literal(title),
                    GraphName::DefaultGraph
                ))?;
            }
            
            // Add links
            for link in parsed.links {
                // Normalize link to a potential file URI
                let target_uri = format!("urn:memexia:file:{}.md", link.replace(" ", "_"));
                let target = NamedNode::new(target_uri)?;
                graph.insert(&Quad::new(
                    subject.clone(),
                    NamedNode::new("http://memexia.org/schema/linksTo")?,
                    target,
                    GraphName::DefaultGraph
                ))?;
            }
            
            // Add tags
            for tag in parsed.tags {
                graph.insert(&Quad::new(
                    subject.clone(),
                    NamedNode::new("http://memexia.org/schema/hasTag")?,
                    Literal::new_simple_literal(tag),
                    GraphName::DefaultGraph
                ))?;
            }
            
            // Add content hash
             graph.insert(&Quad::new(
                subject.clone(),
                NamedNode::new("http://memexia.org/schema/contentHash")?,
                Literal::new_simple_literal(hash),
                GraphName::DefaultGraph
            ))?;
        }
        
        println!("Committed with message: {}", message);
        
        // Clear index
        fs::File::create(index_path)?;
        
        Ok(())
    }

    pub fn query_graph(&self, query: &str) -> Result<Vec<String>> {
        use oxigraph::sparql::QueryResults;
        
        let store = self.storage.get_graph();
        let results = store.query(query)?;
        
        let mut rows = Vec::new();
        if let QueryResults::Solutions(solutions) = results {
            for solution in solutions {
                let solution = solution?;
                let row = solution.iter()
                    .map(|(var, val)| format!("{} = {}", var, val))
                    .collect::<Vec<_>>()
                    .join(", ");
                rows.push(row);
            }
        } else if let QueryResults::Graph(triples) = results {
             for triple in triples {
                let triple = triple?;
                rows.push(format!("{} {} {}", triple.subject, triple.predicate, triple.object));
            }
        }
        Ok(rows)
    }
}


