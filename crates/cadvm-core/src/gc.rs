//! Garbage collection: drop objects unreachable from any ref.
//!
//! Conservative by design — `plan` only *reports* what is unreferenced; objects
//! are removed only when `prune` is explicitly requested.

use std::collections::HashSet;

use cadvm_store::{Category, ObjectId};

use crate::error::Result;
use crate::repo::{Head, Repository};

/// What GC found: the unreferenced objects in each category.
#[derive(Debug, Clone, Default)]
pub struct GcPlan {
    pub commits: Vec<ObjectId>,
    pub manifests: Vec<ObjectId>,
    pub blobs: Vec<ObjectId>,
    pub chunks: Vec<ObjectId>,
}

impl GcPlan {
    /// Total number of objects that would be removed.
    pub fn total(&self) -> usize {
        self.commits.len() + self.manifests.len() + self.blobs.len() + self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// The set of objects reachable from every ref (and a detached HEAD).
#[derive(Debug, Default)]
struct Reachable {
    commits: HashSet<ObjectId>,
    manifests: HashSet<ObjectId>,
    blobs: HashSet<ObjectId>,
    chunks: HashSet<ObjectId>,
}

fn reachable(repo: &Repository) -> Result<Reachable> {
    let mut seen = Reachable::default();

    // Seed from all branch tips and a detached HEAD.
    let mut frontier: Vec<ObjectId> = Vec::new();
    for branch in repo.list_branches()? {
        if let Some(id) = repo.read_ref(&branch)? {
            frontier.push(id);
        }
    }
    if let Head::Detached(id) = repo.read_head()? {
        frontier.push(id);
    }

    // Walk the full commit DAG (all parents), collecting referenced objects.
    while let Some(commit_id) = frontier.pop() {
        if !seen.commits.insert(commit_id.clone()) {
            continue;
        }
        let commit = repo.read_commit(&commit_id)?;
        seen.manifests.insert(commit.manifest.clone());
        let manifest = repo.read_manifest(&commit.manifest)?;
        for entry in manifest.files.values() {
            // V2 stores file content chunk-only: `raw_hash` is just an identity,
            // not a stored blob, so it is deliberately NOT treated as a live blob.
            // This lets gc reclaim raw blobs written by the legacy V1 scheme.
            for chunk in &entry.blob_ref.chunks {
                seen.chunks.insert(chunk.hash.clone());
            }
        }
        for parent in commit.parents {
            frontier.push(parent);
        }
    }

    Ok(seen)
}

/// Compute which stored objects are unreferenced.
pub fn plan(repo: &Repository) -> Result<GcPlan> {
    let live = reachable(repo)?;
    let store = repo.store();

    let unref = |cat: Category, live: &HashSet<ObjectId>| -> Result<Vec<ObjectId>> {
        Ok(store
            .list(cat)?
            .into_iter()
            .filter(|id| !live.contains(id))
            .collect())
    };

    Ok(GcPlan {
        commits: unref(Category::Commit, &live.commits)?,
        manifests: unref(Category::Manifest, &live.manifests)?,
        blobs: unref(Category::Blob, &live.blobs)?,
        chunks: unref(Category::Chunk, &live.chunks)?,
    })
}

/// Delete every object named in `plan`. Returns the number removed.
pub fn prune(repo: &Repository, plan: &GcPlan) -> Result<usize> {
    let store = repo.store();
    let mut removed = 0usize;
    for id in &plan.commits {
        removed += store.remove(Category::Commit, id)? as usize;
    }
    for id in &plan.manifests {
        removed += store.remove(Category::Manifest, id)? as usize;
    }
    for id in &plan.blobs {
        removed += store.remove(Category::Blob, id)? as usize;
    }
    for id in &plan.chunks {
        removed += store.remove(Category::Chunk, id)? as usize;
    }
    Ok(removed)
}
