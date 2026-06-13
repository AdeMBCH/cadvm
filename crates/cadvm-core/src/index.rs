//! Working-tree hash cache (`.cadvm/index.json`).
//!
//! Hashing every tracked file on each `status` is wasteful on large STEP files.
//! This cache remembers, per path, the file's size + modification time and the
//! content hash computed last time. When a file's size and mtime are unchanged,
//! its hash is reused instead of re-reading and re-hashing the whole file.
//!
//! The cache is a transparent optimization: a miss simply re-hashes. It never
//! affects correctness — a stale entry can only occur if a file is rewritten
//! with the exact same size *and* mtime, which the filesystem's nanosecond mtime
//! makes effectively impossible in practice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use cadvm_store::ObjectId;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::repo::Repository;
use crate::worktree;

const INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    size: u64,
    mtime_ns: i64,
    hash: ObjectId,
}

/// A size+mtime keyed cache of working-file content hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashCache {
    version: u32,
    entries: BTreeMap<PathBuf, CacheEntry>,
}

impl Default for HashCache {
    fn default() -> Self {
        HashCache {
            version: INDEX_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

fn mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

impl HashCache {
    /// Load the cache, tolerating a missing or legacy/placeholder file (returns
    /// an empty cache rather than failing).
    pub fn load(repo: &Repository) -> HashCache {
        let path = repo.cadvm_dir().join("index.json");
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => HashCache::default(),
        }
    }

    /// Persist the cache to `.cadvm/index.json`.
    pub fn save(&self, repo: &Repository) -> Result<()> {
        let path = repo.cadvm_dir().join("index.json");
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        std::fs::write(&path, bytes).map_err(|e| CoreError::io(&path, e))
    }

    /// Content hash of a working file, reusing the cache when size+mtime match
    /// and otherwise stream-hashing the file and updating the cache.
    pub fn hash(&mut self, repo: &Repository, rel: &Path) -> Result<ObjectId> {
        let path = worktree::abs_path(repo, rel);
        let meta = std::fs::metadata(&path).map_err(|e| CoreError::io(&path, e))?;
        let (size, mtime) = (meta.len(), mtime_ns(&meta));

        if let Some(e) = self.entries.get(rel) {
            if e.size == size && e.mtime_ns == mtime {
                return Ok(e.hash.clone());
            }
        }

        let hash = worktree::hash_working_file(repo, rel)?;
        self.entries.insert(
            rel.to_path_buf(),
            CacheEntry {
                size,
                mtime_ns: mtime,
                hash: hash.clone(),
            },
        );
        Ok(hash)
    }

    /// Record an already-known hash for a file (stats it for size+mtime). Used
    /// after a snapshot so the next `status` is a cache hit.
    pub fn record(&mut self, repo: &Repository, rel: &Path, hash: ObjectId) -> Result<()> {
        let path = worktree::abs_path(repo, rel);
        let meta = std::fs::metadata(&path).map_err(|e| CoreError::io(&path, e))?;
        self.entries.insert(
            rel.to_path_buf(),
            CacheEntry {
                size: meta.len(),
                mtime_ns: mtime_ns(&meta),
                hash,
            },
        );
        Ok(())
    }

    /// Drop entries for paths no longer present (keeps the file tidy).
    pub fn retain(&mut self, keep: &BTreeSet<PathBuf>) {
        self.entries.retain(|p, _| keep.contains(p));
    }
}
