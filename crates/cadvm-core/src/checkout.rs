//! Working-tree mutation: `checkout`, `switch` and `revert`.
//!
//! All three share a single, conservative restore engine that never deletes
//! untracked files and never overwrites a locally modified file without
//! `--force`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use cadvm_store::ObjectId;

use crate::error::{CoreError, Result};
use crate::model::{CommitBody, Manifest};
use crate::repo::{Head, Repository};
use crate::revision;
use crate::status::working_tree_status;
use crate::worktree;

/// Summary of files changed on disk by a restore operation.
#[derive(Debug, Clone, Default)]
pub struct RestoreOutcome {
    pub written: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

/// Hash every tracked-format file currently on disk.
fn working_hashes(repo: &Repository) -> Result<BTreeMap<PathBuf, ObjectId>> {
    let mut map = BTreeMap::new();
    for rel in worktree::scan_step_files(repo)? {
        map.insert(rel.clone(), worktree::hash_working_file(repo, &rel)?);
    }
    Ok(map)
}

/// Restore the working tree from `baseline` (currently tracked) to `target`.
///
/// When `only` is `Some`, the restore is *path-scoped*: only the listed paths
/// are written and **nothing is deleted** (matching `git checkout <rev> -- f`).
///
/// Safety rules (unless `force`):
/// * a file whose on-disk content differs from `baseline` (locally modified or
///   untracked) is never overwritten or deleted;
/// * untracked files (absent from `baseline`) are never deleted.
fn restore(
    repo: &Repository,
    baseline: &Manifest,
    target: &Manifest,
    force: bool,
    only: Option<&BTreeSet<PathBuf>>,
) -> Result<RestoreOutcome> {
    let working = working_hashes(repo)?;
    let selected = |path: &PathBuf| only.is_none_or(|set| set.contains(path));

    // First pass: detect conflicts without touching the disk.
    if !force {
        // Files we would write.
        for (path, entry) in &target.files {
            if !selected(path) {
                continue;
            }
            let on_disk = working.get(path);
            if on_disk == Some(&entry.raw_hash) {
                continue; // already correct
            }
            if let Some(disk_hash) = on_disk {
                let baseline_hash = baseline.files.get(path).map(|e| &e.raw_hash);
                if Some(disk_hash) != baseline_hash {
                    // Locally modified or untracked content would be clobbered.
                    return Err(CoreError::WouldOverwriteDirtyFile(path.clone()));
                }
            }
        }
        // Files we would delete (tracked by baseline, absent from target).
        // Path-scoped restores never delete.
        if only.is_none() {
            for (path, entry) in &baseline.files {
                if target.files.contains_key(path) {
                    continue;
                }
                if let Some(disk_hash) = working.get(path) {
                    if disk_hash != &entry.raw_hash {
                        return Err(CoreError::WouldOverwriteDirtyFile(path.clone()));
                    }
                }
            }
        }
    }

    // Second pass: apply.
    let mut outcome = RestoreOutcome::default();

    for (path, entry) in &target.files {
        if !selected(path) {
            continue;
        }
        if working.get(path) == Some(&entry.raw_hash) {
            continue;
        }
        let content = repo.store().read_file_content(&entry.blob_ref)?;
        let abs = worktree::abs_path(repo, path);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }
        std::fs::write(&abs, &content).map_err(|e| CoreError::io(&abs, e))?;
        outcome.written.push(path.clone());
    }

    if only.is_none() {
        for path in baseline.files.keys() {
            if target.files.contains_key(path) {
                continue;
            }
            // Only delete files that are still present and clean (or --force).
            if !working.contains_key(path) {
                continue;
            }
            let abs = worktree::abs_path(repo, path);
            match std::fs::remove_file(&abs) {
                Ok(()) => outcome.deleted.push(path.clone()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(CoreError::io(&abs, e)),
            }
        }
    }

    outcome.written.sort();
    outcome.deleted.sort();
    Ok(outcome)
}

/// Result of `checkout`.
#[derive(Debug, Clone)]
pub struct CheckoutOutcome {
    pub commit_id: ObjectId,
    pub restore: RestoreOutcome,
}

/// Restore the working tree to a revision **without moving the current branch**
/// (V1 "restore-like" checkout). HEAD stays attached to its branch.
///
/// If `paths` is non-empty, only those files are restored from the revision
/// (and nothing is deleted); each must exist in that revision.
pub fn checkout(
    repo: &Repository,
    rev: &str,
    paths: &[PathBuf],
    force: bool,
) -> Result<CheckoutOutcome> {
    let commit_id = revision::resolve(repo, rev)?;
    let target = repo.manifest_of_commit(&commit_id)?;
    let baseline = repo.head_manifest()?;

    let only: Option<BTreeSet<PathBuf>> = if paths.is_empty() {
        None
    } else {
        let set: BTreeSet<PathBuf> = paths.iter().cloned().collect();
        for path in &set {
            if !target.files.contains_key(path) {
                return Err(CoreError::PathNotInRevision {
                    path: path.clone(),
                    rev: rev.to_string(),
                });
            }
        }
        Some(set)
    };

    let restore = restore(repo, &baseline, &target, force, only.as_ref())?;
    Ok(CheckoutOutcome { commit_id, restore })
}

/// Result of `switch`.
#[derive(Debug, Clone)]
pub struct SwitchOutcome {
    pub branch: String,
    pub commit_id: Option<ObjectId>,
    pub restore: RestoreOutcome,
}

/// Switch HEAD to another branch, restoring its files. Refuses a dirty working
/// tree unless `force`.
pub fn switch(repo: &Repository, branch: &str, force: bool) -> Result<SwitchOutcome> {
    if !repo.branch_exists(branch) {
        return Err(CoreError::NoSuchBranch(branch.to_string()));
    }

    if !force && !working_tree_status(repo)?.is_clean() {
        return Err(CoreError::DirtyWorkingTree {
            action: "switch".to_string(),
        });
    }

    let baseline = repo.head_manifest()?;
    let commit_id = repo.read_ref(branch)?;
    let target = match &commit_id {
        Some(id) => repo.manifest_of_commit(id)?,
        None => Manifest::empty(),
    };
    let restore = restore(repo, &baseline, &target, force, None)?;
    repo.write_head(&Head::Branch(branch.to_string()))?;

    Ok(SwitchOutcome {
        branch: branch.to_string(),
        commit_id,
        restore,
    })
}

/// Result of `revert`.
#[derive(Debug, Clone)]
pub struct RevertOutcome {
    pub new_commit_id: ObjectId,
    pub reverted_commit_id: ObjectId,
    pub restore: RestoreOutcome,
    pub branch: Option<String>,
}

/// Revert HEAD: create a new commit that restores the state of HEAD's parent.
///
/// V1 only supports reverting HEAD itself. Refuses a dirty working tree unless
/// `force`.
pub fn revert(
    repo: &Repository,
    rev: &str,
    force: bool,
    timestamp_unix: i64,
) -> Result<RevertOutcome> {
    let target_id = revision::resolve(repo, rev)?;
    let head_id = repo
        .head_commit_id()?
        .ok_or_else(|| CoreError::UnknownRevision("HEAD".to_string()))?;
    if target_id != head_id {
        return Err(CoreError::RevertNonHead(rev.to_string()));
    }

    if !force && !working_tree_status(repo)?.is_clean() {
        return Err(CoreError::DirtyWorkingTree {
            action: "revert".to_string(),
        });
    }

    let head_commit = repo.read_commit(&head_id)?;
    let parent_id = head_commit
        .parents
        .first()
        .cloned()
        .ok_or(CoreError::NoParent)?;
    let parent_commit = repo.read_commit(&parent_id)?;

    // Restore working tree to the parent's manifest.
    let baseline = repo.read_manifest(&head_commit.manifest)?;
    let target = repo.read_manifest(&parent_commit.manifest)?;
    let restore = restore(repo, &baseline, &target, force, None)?;

    // Create the revert commit, reusing the parent's manifest, with HEAD as parent.
    let body = CommitBody {
        parents: vec![head_id.clone()],
        manifest: parent_commit.manifest.clone(),
        message: format!("Revert \"{}\"", head_commit.message),
        timestamp_unix,
        author: Some(crate::config::resolve_author(repo)?),
    };
    let new_commit_id = repo.write_commit(&body)?;

    let branch = match repo.read_head()? {
        Head::Branch(name) => {
            repo.write_ref(&name, &new_commit_id)?;
            Some(name)
        }
        Head::Detached(_) => {
            repo.write_head(&Head::Detached(new_commit_id.clone()))?;
            None
        }
    };

    Ok(RevertOutcome {
        new_commit_id,
        reverted_commit_id: head_id,
        restore,
        branch,
    })
}
