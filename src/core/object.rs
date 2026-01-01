use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::io::Write;

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

pub fn write_object(root: &Path, content: &[u8]) -> Result<String> {
    let hash = hash_content(content);
    let objects_dir = root.join(".memexia/objects");
    
    // Use first 2 chars for subdirectory (like git)
    let (dir_name, file_name) = hash.split_at(2);
    let object_dir = objects_dir.join(dir_name);
    fs::create_dir_all(&object_dir)?;
    
    let object_path = object_dir.join(file_name);
    if !object_path.exists() {
        let mut file = fs::File::create(object_path)?;
        file.write_all(content)?;
    }
    
    Ok(hash)
}

pub fn read_object(root: &Path, hash: &str) -> Result<Vec<u8>> {
    if hash.len() < 2 {
        anyhow::bail!("Invalid hash");
    }
    let (dir_name, file_name) = hash.split_at(2);
    let object_path = root.join(".memexia/objects").join(dir_name).join(file_name);
    
    let content = fs::read(object_path)?;
    Ok(content)
}
