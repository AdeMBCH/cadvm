//! Core data types: commits, manifests and file entries.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cadvm_store::{BlobRef, ObjectId};
use serde::{Deserialize, Serialize};

use crate::format::CadFormat;
use crate::mesh::MeshMetadata;
use crate::step::StepMetadata;

/// Current manifest schema version.
pub const MANIFEST_VERSION: u32 = 1;

/// The author of a commit (name + optional email).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Author {
    /// Human-friendly `Name <email>` form (drops the brackets if no email).
    pub fn display(&self) -> String {
        if self.email.is_empty() {
            self.name.clone()
        } else {
            format!("{} <{}>", self.name, self.email)
        }
    }
}

/// A single tracked file inside a [`Manifest`].
///
/// (Not `Eq`: mesh metadata carries floating-point bounds.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relative to the repository root.
    pub path: PathBuf,
    pub format: CadFormat,
    /// Hash of the full file content (level-1 dedup key); mirrors `blob_ref.raw_hash`.
    pub raw_hash: ObjectId,
    /// Storage layout of the file (raw blob + chunks).
    pub blob_ref: BlobRef,
    pub size_bytes: u64,
    pub line_count: Option<u64>,
    /// B-Rep (STEP) metadata, when applicable.
    #[serde(default)]
    pub step_metadata: Option<StepMetadata>,
    /// Mesh (STL/OBJ) metadata, when applicable.
    #[serde(default)]
    pub mesh_metadata: Option<MeshMetadata>,
}

/// A point-in-time snapshot of every tracked file, keyed by relative path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub files: BTreeMap<PathBuf, FileEntry>,
}

impl Manifest {
    /// Create an empty manifest at the current schema version.
    pub fn empty() -> Self {
        Manifest {
            version: MANIFEST_VERSION,
            files: BTreeMap::new(),
        }
    }

    /// Number of tracked files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// A commit: an immutable snapshot pointer plus history metadata.
///
/// `id` is the content hash of the serialized commit body (everything *except*
/// the id itself) and is therefore not persisted inside the commit object — it
/// is recovered from the storage key on read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: ObjectId,
    pub parents: Vec<ObjectId>,
    pub manifest: ObjectId,
    pub message: String,
    pub timestamp_unix: i64,
    /// Commit author. `None` for legacy commits written before authors existed.
    pub author: Option<Author>,
}

/// The on-disk, serialized form of a commit (without its self-referential id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitBody {
    pub parents: Vec<ObjectId>,
    pub manifest: ObjectId,
    pub message: String,
    pub timestamp_unix: i64,
    /// Author; `#[serde(default)]` keeps old author-less commits readable.
    #[serde(default)]
    pub author: Option<Author>,
}

impl Commit {
    /// Split a commit into its serializable body.
    pub fn body(&self) -> CommitBody {
        CommitBody {
            parents: self.parents.clone(),
            manifest: self.manifest.clone(),
            message: self.message.clone(),
            timestamp_unix: self.timestamp_unix,
            author: self.author.clone(),
        }
    }

    /// Reassemble a full commit from a stored body and its content id.
    pub fn from_body(id: ObjectId, body: CommitBody) -> Self {
        Commit {
            id,
            parents: body.parents,
            manifest: body.manifest,
            message: body.message,
            timestamp_unix: body.timestamp_unix,
            author: body.author,
        }
    }
}
