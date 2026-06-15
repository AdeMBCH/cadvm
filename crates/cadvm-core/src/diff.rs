//! Manifest-level diffs (added / removed / modified files + metadata deltas).
//!
//! This is a *textual / metadata* diff only — it never compares geometry. The
//! future Open CASCADE stage will add added/removed/common B-Rep diffing.

use std::path::PathBuf;

use cadvm_store::ObjectId;
use serde::Serialize;

use crate::model::{FileEntry, Manifest};

/// Per-file metadata changes for a modified file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub size_bytes: (u64, u64),
    pub raw_hash: (ObjectId, ObjectId),
    pub line_count: (Option<u64>, Option<u64>),
    pub schema: (Option<String>, Option<String>),
    pub entity_count: (Option<u64>, Option<u64>),
    /// Mesh triangle counts (STL/OBJ), when applicable.
    pub triangles: (Option<u64>, Option<u64>),
    /// Mesh vertex counts (STL/OBJ), when applicable.
    pub vertices: (Option<u64>, Option<u64>),
}

impl FileDiff {
    fn between(a: &FileEntry, b: &FileEntry) -> Self {
        let schema = (
            a.step_metadata.as_ref().and_then(|m| m.file_schema.clone()),
            b.step_metadata.as_ref().and_then(|m| m.file_schema.clone()),
        );
        let entity_count = (
            a.step_metadata.as_ref().and_then(|m| m.entity_count),
            b.step_metadata.as_ref().and_then(|m| m.entity_count),
        );
        let triangles = (
            a.mesh_metadata.as_ref().and_then(|m| m.triangles),
            b.mesh_metadata.as_ref().and_then(|m| m.triangles),
        );
        let vertices = (
            a.mesh_metadata.as_ref().and_then(|m| m.vertices),
            b.mesh_metadata.as_ref().and_then(|m| m.vertices),
        );
        FileDiff {
            path: a.path.clone(),
            size_bytes: (a.size_bytes, b.size_bytes),
            raw_hash: (a.raw_hash.clone(), b.raw_hash.clone()),
            line_count: (a.line_count, b.line_count),
            schema,
            entity_count,
            triangles,
            vertices,
        }
    }
}

/// The full diff between two manifests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ManifestDiff {
    pub added: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
    pub modified: Vec<FileDiff>,
}

impl ManifestDiff {
    /// Whether the two manifests are identical.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Diff manifest `a` (left/old) against manifest `b` (right/new).
pub fn diff_manifests(a: &Manifest, b: &Manifest) -> ManifestDiff {
    let mut diff = ManifestDiff::default();

    for (path, entry_b) in &b.files {
        match a.files.get(path) {
            None => diff.added.push(path.clone()),
            Some(entry_a) if entry_a.raw_hash != entry_b.raw_hash => {
                diff.modified.push(FileDiff::between(entry_a, entry_b));
            }
            Some(_) => {}
        }
    }

    for path in a.files.keys() {
        if !b.files.contains_key(path) {
            diff.removed.push(path.clone());
        }
    }

    diff.added.sort();
    diff.removed.sort();
    diff.modified.sort_by(|x, y| x.path.cmp(&y.path));
    diff
}
