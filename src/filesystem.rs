use crate::crypto::{encrypt_file, decrypt_file};
use anyhow::{Result, Context};
use std::path::PathBuf;
use dirs::home_dir;

pub struct FileSystem {
    pub dirs: Vec<PathBuf>,
    encrypted: Vec<bool>,
}

impl FileSystem {
    pub fn new() -> Result<Self> {
        let home = home_dir().context("Could not find home directory")?;
        let dirs = std::fs::read_dir(&home)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect::<Vec<_>>();
        let encrypted = vec![false; dirs.len()];
        Ok(FileSystem { dirs, encrypted })
    }

    pub fn get_files(&self, index: usize) -> Result<Vec<String>, anyhow::Error> {
        if index >= self.dirs.len() {
            return Err(anyhow::anyhow!("Invalid directory index"));
        }
        let dir = &self.dirs[index];
        Ok(std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
            .collect())
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
        self.encrypted.push(false);
        Ok(())
    }

    pub fn mark_encrypted(&mut self, index: usize, encrypted: bool) {
        if index < self.encrypted.len() {
            self.encrypted[index] = encrypted;
        }
    }

    pub fn is_encrypted(&self, index: usize) -> bool {
        index < self.encrypted.len() && self.encrypted[index]
    }
}