//! Creating snapshots (commits) from the working tree.

use cadvm_store::ObjectId;

use crate::config;
use crate::error::Result;
use crate::format::CadFormat;
use crate::model::{CommitBody, FileEntry, Manifest};
use crate::repo::{Head, Repository};
use crate::step;
use crate::worktree;

/// Result of a successful snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotOutcome {
    pub commit_id: ObjectId,
    pub manifest_id: ObjectId,
    pub file_count: usize,
    /// The branch the snapshot advanced, if HEAD was attached.
    pub branch: Option<String>,
}

/// Build a manifest from the current working tree (no commit written).
pub fn build_manifest(repo: &Repository) -> Result<Manifest> {
    let mut manifest = Manifest::empty();
    for rel in worktree::scan_step_files(repo)? {
        let content = worktree::read_working_file(repo, &rel)?;
        let format = CadFormat::from_path(&rel).expect("scan only yields tracked formats");
        let blob_ref = repo.store().put_file_content(&content)?;
        let raw_hash = blob_ref.raw_hash.clone();
        let line_count = Some(count_lines(&content));
        let step_metadata = step::extract(&content);

        let entry = FileEntry {
            path: rel.clone(),
            format,
            raw_hash,
            blob_ref,
            size_bytes: content.len() as u64,
            line_count,
            step_metadata,
        };
        manifest.files.insert(rel, entry);
    }
    Ok(manifest)
}

/// Create a snapshot: scan the working tree, store content, write a manifest and
/// commit, and advance the current branch (or detached HEAD).
///
/// `timestamp_unix` is supplied by the caller (the CLI passes the current time)
/// so the core stays deterministic and easily testable.
pub fn snapshot(repo: &Repository, message: &str, timestamp_unix: i64) -> Result<SnapshotOutcome> {
    let manifest = build_manifest(repo)?;
    let file_count = manifest.file_count();
    let manifest_id = repo.write_manifest(&manifest)?;

    let parents = match repo.head_commit_id()? {
        Some(parent) => vec![parent],
        None => Vec::new(),
    };

    let body = CommitBody {
        parents,
        manifest: manifest_id.clone(),
        message: message.to_string(),
        timestamp_unix,
        author: Some(config::resolve_author(repo)?),
    };
    let commit_id = repo.write_commit(&body)?;

    // Advance whatever HEAD points at.
    let branch = match repo.read_head()? {
        Head::Branch(name) => {
            repo.write_ref(&name, &commit_id)?;
            Some(name)
        }
        Head::Detached(_) => {
            repo.write_head(&Head::Detached(commit_id.clone()))?;
            None
        }
    };

    Ok(SnapshotOutcome {
        commit_id,
        manifest_id,
        file_count,
        branch,
    })
}

/// Count lines in file content (number of `\n`, plus one if the last line is
/// unterminated). Empty content has zero lines.
fn count_lines(content: &[u8]) -> u64 {
    if content.is_empty() {
        return 0;
    }
    let newlines = content.iter().filter(|&&b| b == b'\n').count() as u64;
    if content.last() == Some(&b'\n') {
        newlines
    } else {
        newlines + 1
    }
}
