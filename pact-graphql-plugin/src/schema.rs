use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRef {
    pub hash: String,
    pub encoding: String,
}

pub struct SchemaRegistry {
    root: PathBuf,
}

impl SchemaRegistry {
    pub fn new(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).with_context(|| {
            format!(
                "failed to create schema registry root at {}",
                root.display()
            )
        })?;
        Ok(Self { root })
    }

    pub fn store(&self, sdl: &str) -> anyhow::Result<SchemaRef> {
        let trimmed = sdl.trim();
        if trimmed.is_empty() {
            bail!("schema SDL must not be empty");
        }

        let canonical = format!("{}\n", trimmed);
        let hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
        let path = self.root.join(format!("{}.graphql", hash));

        if !path.exists() {
            fs::write(&path, canonical)
                .with_context(|| format!("failed to write schema file {}", path.display()))?;
        }

        Ok(SchemaRef {
            hash,
            encoding: "utf-8".to_string(),
        })
    }

    pub fn inline_schema(&self, hash: &str) -> anyhow::Result<String> {
        validate_hash(hash)?;
        let path = self.root.join(format!("{}.graphql", hash));
        fs::read_to_string(&path)
            .with_context(|| format!("failed to read schema file {}", path.display()))
    }
}

fn validate_hash(hash: &str) -> anyhow::Result<()> {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("invalid schema hash: {}", hash);
    }

    Ok(())
}
