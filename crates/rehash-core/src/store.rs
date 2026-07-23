use std::collections::HashMap;
use crate::cache::{CacheKey, CacheEntry};
use crate::error::Result;

pub struct MetaStore {
    db_dir: std::path::PathBuf,
    entries: HashMap<CacheKey, CacheEntry>,
    max_entries: usize,
}

impl MetaStore {
    pub fn new(db_dir: std::path::PathBuf, max_entries: usize) -> Self {
        Self { db_dir, entries: HashMap::new(), max_entries }
    }

    pub async fn load(&mut self) -> Result<()> {
        let path = self.db_dir.join("index.json");
        if !path.exists() {
            return Ok(());
        }
        let data = tokio::fs::read_to_string(&path).await?;
        self.entries = serde_json::from_str(&data)?;
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.db_dir).await?;
        let path = self.db_dir.join("index.json");
        let data = serde_json::to_string_pretty(&self.entries)?;
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn insert(&mut self, entry: CacheEntry) -> Result<()> {
        self.entries.insert(entry.key, entry);
        while self.entries.len() > self.max_entries {
            let oldest = self.entries.iter()
                .min_by_key(|(_, e)| e.created_at)
                .map(|(k, _)| *k);
            if let Some(key) = oldest {
                self.entries.remove(&key);
            }
        }
        Ok(())
    }

    pub fn get(&self, key: &CacheKey) -> Option<&CacheEntry> {
        self.entries.get(key)
    }

    pub fn remove(&mut self, key: &CacheKey) -> Option<CacheEntry> {
        self.entries.remove(key)
    }

    pub fn entries(&self) -> impl Iterator<Item = &CacheEntry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn total_size(&self) -> u64 {
        self.entries.values().map(|e| e.compressed_size).sum()
    }
}
