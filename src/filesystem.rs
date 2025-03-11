use crate::crypto::{encrypt_file, decrypt_file};
use anyhow::{Result, Context};
use std::path::PathBuf;
use dirs::home_dir;

pub struct FileSystem {
    pub dirs: Vec<PathBuf>,
}

impl FileSystem {
    pub fn new() -> Result<Self> {
        let home = home_dir().context("Could not find home directory")?;
        let dirs = std::fs::read_dir(&home)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        Ok(FileSystem { dirs })
    }

    pub fn get_files(&self, index: usize) -> Vec<String> {
        if index >= self.dirs.len() {
            return vec!["Error: Invalid directory index".to_string()];
        }
        let dir = &self.dirs[index];
        match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
                .collect(),
            Err(_) => vec![format!("Error reading directory: {:?}", dir)],
        }
    }

    pub fn encrypt_dir(&self, index: usize, key: &str) -> Result<()> {
        if index >= self.dirs.len() {
            return Err(anyhow::anyhow!("Invalid directory index"));
        }
        let dir = &self.dirs[index];
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                encrypt_file(&path, key)?;
            }
        }
        Ok(())
    }

    pub fn decrypt_dir(&self, index: usize, key: &str) -> Result<()> {
        if index >= self.dirs.len() {
            return Err(anyhow::anyhow!("Invalid directory index"));
        }
        let dir = &self.dirs[index];
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file() {
                decrypt_file(&path, key)?;
            }
        }
        Ok(())
    }

    pub fn create_folder(&mut self, name: &str) -> Result<()> {
        let home = home_dir().context("Could not find home directory")?;
        let new_path = home.join(name);
        std::fs::create_dir(&new_path)?;
        self.dirs.push(new_path);
        Ok(())
    }
}