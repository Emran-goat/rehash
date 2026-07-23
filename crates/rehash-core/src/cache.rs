use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use crate::error::{Error, Result};
use crate::compress::Compressor;
use crate::store::MetaStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    pub fn new(hash: [u8; 32]) -> Self {
        Self(hash)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut buf = String::with_capacity(64);
        for &byte in &self.0 {
            buf.push(HEX[(byte >> 4) as usize] as char);
            buf.push(HEX[(byte & 0x0f) as usize] as char);
        }
        buf
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        if s.len() != 64 {
            return Err(Error::Corrupt(format!("hex string must be 64 chars, got {}", s.len())));
        }
        let mut hash = [0u8; 32];
        for i in 0..32 {
            hash[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|e| Error::Corrupt(format!("invalid hex: {}", e)))?;
        }
        Ok(Self(hash))
    }
}

impl Serialize for CacheKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for CacheKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub output_paths: Vec<PathBuf>,
    pub compressed_size: u64,
    pub original_size: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub toolchain: String,
    pub duration_ms: u64,
}

pub struct CacheEngine {
    cache_dir: PathBuf,
    compressor: Compressor,
    max_size: u64,
    meta_store: MetaStore,
    loaded: bool,
}

impl CacheEngine {
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> Self {
        Self {
            meta_store: MetaStore::new(cache_dir.join("meta"), 10_000),
            cache_dir,
            compressor: Compressor::new(3),
            max_size: max_size_mb * 1024 * 1024,
            loaded: false,
        }
    }

    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.meta_store.load().await?;
            self.loaded = true;
        }
        Ok(())
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn object_path(&self, key: &CacheKey) -> PathBuf {
        self.cache_dir.join("objects").join(format!("{}.zst", key.to_hex()))
    }

    pub async fn contains(&mut self, key: &CacheKey) -> Result<bool> {
        self.ensure_loaded().await?;
        Ok(self.object_path(key).exists() && self.meta_store.get(key).is_some())
    }

    pub async fn get(&mut self, key: &CacheKey) -> Result<CacheEntry> {
        self.ensure_loaded().await?;
        self.meta_store.get(key)
            .cloned()
            .ok_or_else(|| Error::CacheMiss(key.to_hex()))
    }

    pub async fn restore(&mut self, key: &CacheKey, target_dir: &Path) -> Result<()> {
        let object_path = self.object_path(key);
        let compressed = tokio::fs::read(&object_path).await?;
        let decompressed = self.compressor.decompress(&compressed)?;
        let mut archive = tar::Archive::new(decompressed.as_slice());
        archive.unpack(target_dir).map_err(Error::Io)?;
        Ok(())
    }

    pub async fn store(&mut self, key: &CacheKey, outputs: &[PathBuf], toolchain: &str, duration_ms: u64) -> Result<()> {
        self.ensure_loaded().await?;

        let objects_dir = self.cache_dir.join("objects");
        tokio::fs::create_dir_all(&objects_dir).await?;

        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for path in outputs {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| Error::Corrupt(format!("invalid output path: {}", path.display())))?;
                builder.append_path_with_name(path, name).map_err(Error::Io)?;
            }
            builder.finish().map_err(Error::Io)?;
        }

        let original_size = tar_bytes.len() as u64;
        let compressed = self.compressor.compress(&tar_bytes)?;
        let compressed_size = compressed.len() as u64;

        let object_path = self.object_path(key);
        tokio::fs::write(&object_path, &compressed).await?;

        let entry = CacheEntry {
            key: *key,
            output_paths: outputs.to_vec(),
            compressed_size,
            original_size,
            created_at: chrono::Utc::now(),
            toolchain: toolchain.to_string(),
            duration_ms,
        };

        self.meta_store.insert(entry).await?;
        self.meta_store.save().await?;

        Ok(())
    }

    pub async fn evict(&mut self) -> Result<u64> {
        self.ensure_loaded().await?;

        let total = self.meta_store.total_size();
        if total <= self.max_size {
            return Ok(0);
        }

        let mut entries: Vec<(CacheKey, chrono::DateTime<chrono::Utc>)> = self.meta_store.entries()
            .map(|e| (e.key, e.created_at))
            .collect();
        entries.sort_by_key(|(_, c)| *c);

        let mut freed = 0u64;
        let mut current = total;

        for (key, _) in &entries {
            if current <= self.max_size {
                break;
            }
            if let Some(entry) = self.meta_store.remove(key) {
                current = current.saturating_sub(entry.compressed_size);
                freed += entry.compressed_size;
                let obj_path = self.object_path(key);
                let _ = tokio::fs::remove_file(&obj_path).await;
            }
        }

        self.meta_store.save().await?;
        Ok(freed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_restore() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let mut engine = CacheEngine::new(cache_dir, 100);

        let out_dir = tmp.path().join("out");
        tokio::fs::create_dir_all(&out_dir).await.unwrap();
        let test_file = out_dir.join("test.txt");
        tokio::fs::write(&test_file, b"hello world").await.unwrap();

        let key = CacheKey::new([1; 32]);
        engine.store(&key, &[test_file.clone()], "test-toolchain", 42).await.unwrap();

        assert!(engine.contains(&key).await.unwrap());

        let restore_dir = tmp.path().join("restore");
        tokio::fs::create_dir_all(&restore_dir).await.unwrap();
        engine.restore(&key, &restore_dir).await.unwrap();

        let restored = restore_dir.join("test.txt");
        let content = tokio::fs::read_to_string(&restored).await.unwrap();
        assert_eq!(content, "hello world");

        let entry = engine.get(&key).await.unwrap();
        assert_eq!(entry.toolchain, "test-toolchain");
        assert_eq!(entry.duration_ms, 42);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let tmp = TempDir::new().unwrap();
        let mut engine = CacheEngine::new(tmp.path().join("cache"), 100);
        let key = CacheKey::new([2; 32]);
        let result = engine.get(&key).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::CacheMiss(_))));
    }

    #[tokio::test]
    async fn test_eviction() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let mut engine = CacheEngine::new(cache_dir.clone(), 0);

        let out_dir = tmp.path().join("out");
        tokio::fs::create_dir_all(&out_dir).await.unwrap();
        let test_file = out_dir.join("test.bin");
        let data = vec![0u8; 1024];
        tokio::fs::write(&test_file, &data).await.unwrap();

        let key = CacheKey::new([3; 32]);
        engine.store(&key, &[test_file], "test", 0).await.unwrap();

        let freed = engine.evict().await.unwrap();
        assert!(freed > 0);
        assert!(!engine.contains(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_key_hex_roundtrip() {
        let original = CacheKey::new([0xab; 32]);
        let hex = original.to_hex();
        assert_eq!(hex.len(), 64);
        let decoded = CacheKey::from_hex(&hex).unwrap();
        assert_eq!(original, decoded);
    }

    #[tokio::test]
    async fn test_persistence() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");

        let out_dir = tmp.path().join("out");
        tokio::fs::create_dir_all(&out_dir).await.unwrap();
        let test_file = out_dir.join("data.txt");
        tokio::fs::write(&test_file, b"persist test").await.unwrap();

        let key = CacheKey::new([4; 32]);
        {
            let mut engine = CacheEngine::new(cache_dir.clone(), 100);
            engine.store(&key, &[test_file], "persist", 10).await.unwrap();
        }

        {
            let mut engine = CacheEngine::new(cache_dir, 100);
            assert!(engine.contains(&key).await.unwrap());
            let entry = engine.get(&key).await.unwrap();
            assert_eq!(entry.toolchain, "persist");
        }
    }
}
