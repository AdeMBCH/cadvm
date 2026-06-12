//! `cadvm-store` — content-addressed storage for cadvm.
//!
//! The store is a thin, safe wrapper around an on-disk object directory. Every
//! object is addressed by the BLAKE3 hash of its content ([`ObjectId`]) and
//! written into a two-level sharded directory layout:
//!
//! ```text
//! <objects>/<category>/ab/cd/<full-hex>
//! ```
//!
//! Four categories live side by side: raw `blobs`, fixed-size `chunks`,
//! serialized `manifests` and `commits`. Because addressing is purely
//! content-based, writing the same bytes twice is automatically deduplicated.
//!
//! File content is stored **chunk-only** (V2): a tracked file is split into
//! fixed 256 KiB chunks (the `chunks` category) and reconstructed from them; the
//! whole-file hash is kept as an identity but the full file is not duplicated as
//! a standalone blob. The `blobs` category therefore holds only legacy V1
//! objects, which `gc` reclaims.
//!
//! Writes are atomic: bytes are streamed to a temp file inside the object
//! directory and then `rename`d into place, so a crash can never leave a
//! partially written (and therefore corrupt) object.

mod object_id;

pub use object_id::{ObjectId, ObjectIdParseError, ALGO_PREFIX};

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Fixed chunk size used by level-2 deduplication: 256 KiB.
pub const CHUNK_SIZE: usize = 256 * 1024;

/// Errors that can occur while interacting with the object store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("object not found: {0}")]
    NotFound(ObjectId),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, StoreError>;

fn io_err(path: impl Into<PathBuf>, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.into(),
        source,
    }
}

/// The kind of object being stored. Each maps to a sub-directory under
/// `<objects>/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Blob,
    Chunk,
    Manifest,
    Commit,
}

impl Category {
    /// Directory name under `<objects>/` for this category.
    pub fn dir_name(self) -> &'static str {
        match self {
            Category::Blob => "blobs",
            Category::Chunk => "chunks",
            Category::Manifest => "manifests",
            Category::Commit => "commits",
        }
    }

    /// All categories, in their on-disk creation order.
    pub fn all() -> [Category; 4] {
        [
            Category::Blob,
            Category::Chunk,
            Category::Manifest,
            Category::Commit,
        ]
    }
}

/// A reference to a stored chunk: its hash plus where it sits in the source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkRef {
    pub hash: ObjectId,
    pub offset: u64,
    pub size: u64,
}

/// How a tracked file is laid out in the store: the whole-file raw hash plus the
/// ordered list of fixed-size chunks that reconstruct it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    /// Hash of the complete file content (level-1 dedup key).
    pub raw_hash: ObjectId,
    /// Total file size in bytes.
    pub size_bytes: u64,
    /// Ordered chunks (level-2 dedup), each 256 KiB except possibly the last.
    pub chunks: Vec<ChunkRef>,
}

/// A content-addressed object store rooted at a `.cadvm/objects` directory.
#[derive(Debug, Clone)]
pub struct Store {
    objects_root: PathBuf,
}

impl Store {
    /// Open a store rooted at the given `objects` directory. The directory and
    /// its category sub-directories are created if missing.
    pub fn open(objects_root: impl Into<PathBuf>) -> Result<Self> {
        let store = Store {
            objects_root: objects_root.into(),
        };
        for cat in Category::all() {
            let dir = store.objects_root.join(cat.dir_name());
            std::fs::create_dir_all(&dir).map_err(|e| io_err(&dir, e))?;
        }
        Ok(store)
    }

    /// Path at which an object of the given category and id is (or would be) stored.
    fn object_path(&self, category: Category, id: &ObjectId) -> PathBuf {
        self.objects_root
            .join(category.dir_name())
            .join(id.shard1())
            .join(id.shard2())
            .join(id.hex())
    }

    /// Whether an object with this id already exists in the given category.
    pub fn has(&self, category: Category, id: &ObjectId) -> bool {
        self.object_path(category, id).exists()
    }

    /// Convenience: does this blob exist?
    pub fn has_object(&self, id: &ObjectId) -> bool {
        self.has(Category::Blob, id)
    }

    /// Store raw bytes in the given category and return their content id.
    ///
    /// Writing is atomic and idempotent: if the object already exists the write
    /// is skipped entirely.
    pub fn put(&self, category: Category, bytes: &[u8]) -> Result<ObjectId> {
        let id = ObjectId::hash_bytes(bytes);
        let dest = self.object_path(category, &id);
        if dest.exists() {
            return Ok(id);
        }
        let parent = dest.parent().expect("object path always has a parent");
        std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;

        // Atomic write: temp file in the same directory, then rename into place.
        let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| io_err(parent, e))?;
        tmp.write_all(bytes).map_err(|e| io_err(parent, e))?;
        tmp.flush().map_err(|e| io_err(parent, e))?;
        tmp.persist(&dest).map_err(|e| io_err(&dest, e.error))?;
        Ok(id)
    }

    /// Store a raw blob and return its id (level-1 dedup).
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<ObjectId> {
        self.put(Category::Blob, bytes)
    }

    /// Read the raw bytes of a stored object.
    pub fn get(&self, category: Category, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self.object_path(category, id);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(StoreError::NotFound(id.clone()))
            }
            Err(e) => Err(io_err(&path, e)),
        }
    }

    /// Read a stored blob.
    pub fn get_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        self.get(Category::Blob, id)
    }

    /// Store a JSON-serializable object content-addressed and return its id.
    pub fn put_json<T: Serialize>(&self, category: Category, value: &T) -> Result<ObjectId> {
        let bytes = serde_json::to_vec_pretty(value)?;
        self.put(category, &bytes)
    }

    /// Read and deserialize a JSON object.
    pub fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        category: Category,
        id: &ObjectId,
    ) -> Result<T> {
        let bytes = self.get(category, id)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Write a file's content into the store, producing a [`BlobRef`].
    ///
    /// **V2 chunk-only storage.** The file is split into fixed 256 KiB chunks,
    /// each stored content-addressed so identical chunks across files/versions
    /// are shared. `raw_hash` is the BLAKE3 hash of the *whole* file and serves
    /// purely as a content identity (used for dedup keys and status comparisons)
    /// — the full file is **not** stored as a standalone blob, so there is no
    /// on-disk duplication between the raw blob and its chunks.
    ///
    /// This is backward compatible with repositories written by V1 (which stored
    /// both): V1 always wrote the chunks too, so reconstruction still works, and
    /// `gc --prune` reclaims the now-redundant raw blobs.
    pub fn put_file_content(&self, content: &[u8]) -> Result<BlobRef> {
        let raw_hash = ObjectId::hash_bytes(content);

        let mut chunks = Vec::new();
        let mut offset: u64 = 0;
        for chunk in content.chunks(CHUNK_SIZE) {
            let hash = self.put(Category::Chunk, chunk)?;
            chunks.push(ChunkRef {
                hash,
                offset,
                size: chunk.len() as u64,
            });
            offset += chunk.len() as u64;
        }

        Ok(BlobRef {
            raw_hash,
            size_bytes: content.len() as u64,
            chunks,
        })
    }

    /// Reconstruct a file's content from a [`BlobRef`] by concatenating its
    /// chunks (the V2 reconstruction path).
    pub fn read_file_content(&self, blob_ref: &BlobRef) -> Result<Vec<u8>> {
        self.read_file_content_from_chunks(blob_ref)
    }

    /// Reconstruct a file's content from its chunks. Equivalent to
    /// [`read_file_content`](Self::read_file_content); kept as an explicit name
    /// for tests that exercise the chunk path directly.
    pub fn read_file_content_from_chunks(&self, blob_ref: &BlobRef) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(blob_ref.size_bytes as usize);
        for chunk in &blob_ref.chunks {
            out.extend_from_slice(&self.get(Category::Chunk, &chunk.hash)?);
        }
        Ok(out)
    }

    /// Iterate over the ids of every stored object in a category.
    pub fn list(&self, category: Category) -> Result<Vec<ObjectId>> {
        let root = self.objects_root.join(category.dir_name());
        let mut ids = Vec::new();
        Self::collect_ids(&root, &mut ids)?;
        Ok(ids)
    }

    fn collect_ids(dir: &Path, ids: &mut Vec<ObjectId>) -> Result<()> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(io_err(dir, e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| io_err(dir, e))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|e| io_err(&path, e))?;
            if file_type.is_dir() {
                Self::collect_ids(&path, ids)?;
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(id) = name.parse::<ObjectId>() {
                    ids.push(id);
                }
            }
        }
        Ok(())
    }

    /// Delete an object. Used by `gc --prune`; returns `Ok(false)` if absent.
    pub fn remove(&self, category: Category, id: &ObjectId) -> Result<bool> {
        let path = self.object_path(category, id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err(&path, e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("objects")).unwrap();
        (dir, store)
    }

    #[test]
    fn put_bytes_is_content_addressed_and_dedups() {
        let (_d, store) = temp_store();
        let a = store.put_bytes(b"identical content").unwrap();
        let b = store.put_bytes(b"identical content").unwrap();
        assert_eq!(a, b);
        assert!(store.has_object(&a));
        assert_eq!(store.list(Category::Blob).unwrap().len(), 1);
    }

    #[test]
    fn file_content_roundtrips_via_raw_and_chunks() {
        let (_d, store) = temp_store();
        let content: Vec<u8> = (0..(CHUNK_SIZE * 2 + 17))
            .map(|i| (i % 251) as u8)
            .collect();
        let blob_ref = store.put_file_content(&content).unwrap();
        assert_eq!(blob_ref.size_bytes, content.len() as u64);
        assert_eq!(blob_ref.chunks.len(), 3);
        assert_eq!(store.read_file_content(&blob_ref).unwrap(), content);
        assert_eq!(
            store.read_file_content_from_chunks(&blob_ref).unwrap(),
            content
        );
    }
}
