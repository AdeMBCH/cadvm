//! Working-tree status: compare the working tree against a manifest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use cadvm_store::ObjectId;

use crate::error::Result;
use crate::index::HashCache;
use crate::model::Manifest;
use crate::repo::Repository;
use crate::worktree;

/// The difference between the working tree and a reference manifest (HEAD).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingTreeStatus {
    /// Branch HEAD is on, if attached.
    pub branch: Option<String>,
    /// Files present in the working tree but not in the manifest.
    pub new: Vec<PathBuf>,
    /// Files present in both but with different content.
    pub modified: Vec<PathBuf>,
    /// Files in the manifest but missing from the working tree.
    pub deleted: Vec<PathBuf>,
}

impl WorkingTreeStatus {
    /// Whether the working tree matches the manifest exactly.
    pub fn is_clean(&self) -> bool {
        self.new.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }
}

/// Compute working-tree status against the HEAD manifest.
pub fn working_tree_status(repo: &Repository) -> Result<WorkingTreeStatus> {
    let manifest = repo.head_manifest()?;
    status_against(repo, &manifest)
}

/// Compute working-tree status against an arbitrary manifest.
pub fn status_against(repo: &Repository, manifest: &Manifest) -> Result<WorkingTreeStatus> {
    let branch = repo.current_branch()?;

    // Hash every tracked-format file currently on disk, reusing the size+mtime
    // cache so unchanged (often large) files are not re-read.
    let mut cache = HashCache::load(repo);
    let mut working: BTreeMap<PathBuf, ObjectId> = BTreeMap::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for rel in worktree::scan_step_files(repo)? {
        let hash = cache.hash(repo, &rel)?;
        seen.insert(rel.clone());
        working.insert(rel, hash);
    }
    cache.retain(&seen);
    let _ = cache.save(repo); // best-effort: a cache write must never fail status

    let mut status = WorkingTreeStatus {
        branch,
        ..Default::default()
    };

    // New / modified.
    for (path, hash) in &working {
        match manifest.files.get(path) {
            None => status.new.push(path.clone()),
            Some(entry) if &entry.raw_hash != hash => status.modified.push(path.clone()),
            Some(_) => {}
        }
    }

    // Deleted.
    for path in manifest.files.keys() {
        if !working.contains_key(path) {
            status.deleted.push(path.clone());
        }
    }

    status.new.sort();
    status.modified.sort();
    status.deleted.sort();
    Ok(status)
}
