use std::path::Path;
use crate::error::Result;

pub struct Hasher {
    inner: blake3::Hasher,
}

impl Hasher {
    pub fn new() -> Self {
        Self { inner: blake3::Hasher::new() }
    }

    pub async fn update(&mut self, path: &Path) -> Result<()> {
        let data = tokio::fs::read(path).await?;
        self.inner.update(&data);
        Ok(())
    }

    pub fn update_str(&mut self, s: &str) {
        self.inner.update(s.as_bytes());
    }

    pub fn finalize(&self) -> [u8; 32] {
        self.inner.finalize().into()
    }

    pub fn finalize_hex(&self) -> String {
        self.inner.finalize().to_hex().to_string()
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}
