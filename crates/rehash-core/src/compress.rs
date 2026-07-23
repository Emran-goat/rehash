use std::path::Path;
use crate::error::{Error, Result};

pub struct Compressor {
    level: i32,
}

impl Compressor {
    pub fn new(level: i32) -> Self {
        Self { level }
    }

    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(data, self.level)
            .map_err(|e| Error::Compress(e.to_string()))
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data)
            .map_err(|e| Error::Compress(e.to_string()))
    }

    pub async fn compress_to_file(&self, path: &Path, data: &[u8]) -> Result<()> {
        let compressed = self.compress(data)?;
        tokio::fs::write(path, &compressed).await?;
        Ok(())
    }

    pub async fn decompress_from_file(&self, path: &Path) -> Result<Vec<u8>> {
        let data = tokio::fs::read(path).await?;
        self.decompress(&data)
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new(3)
    }
}
