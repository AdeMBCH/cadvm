//! Revision resolution: `HEAD`, `HEAD~N`, branch names, full and short hashes.

use cadvm_store::{Category, ObjectId};

use crate::error::{CoreError, Result};
use crate::repo::Repository;

/// Resolve a revision spec to a concrete commit id.
///
/// Supported forms, tried in this order:
/// * `HEAD`, `HEAD~N` — current commit, walking `N` first-parents back
/// * a branch name
/// * a full 64-char hash (with or without the `blake3:` prefix)
/// * an unambiguous short hash prefix
pub fn resolve(repo: &Repository, spec: &str) -> Result<ObjectId> {
    let spec = spec.trim();

    // HEAD / HEAD~N.
    if spec == "HEAD" || spec.starts_with("HEAD~") || spec.starts_with("HEAD^") {
        return resolve_head_relative(repo, spec);
    }

    // Branch name.
    if repo.branch_exists(spec) {
        return repo
            .read_ref(spec)?
            .ok_or_else(|| CoreError::EmptyBranch(spec.to_string()));
    }

    // Full or short hash.
    resolve_hash(repo, spec)
}

fn resolve_head_relative(repo: &Repository, spec: &str) -> Result<ObjectId> {
    let n: usize = if spec == "HEAD" {
        0
    } else if let Some(rest) = spec.strip_prefix("HEAD~") {
        rest.parse()
            .map_err(|_| CoreError::UnknownRevision(spec.to_string()))?
    } else if let Some(rest) = spec.strip_prefix("HEAD^") {
        // `HEAD^` == `HEAD~1`; `HEAD^N` is treated the same way for V1.
        if rest.is_empty() {
            1
        } else {
            rest.parse()
                .map_err(|_| CoreError::UnknownRevision(spec.to_string()))?
        }
    } else {
        return Err(CoreError::UnknownRevision(spec.to_string()));
    };

    let mut current = repo
        .head_commit_id()?
        .ok_or_else(|| CoreError::UnknownRevision("HEAD".to_string()))?;
    for _ in 0..n {
        let commit = repo.read_commit(&current)?;
        current = commit
            .parents
            .into_iter()
            .next()
            .ok_or(CoreError::NoParent)?;
    }
    Ok(current)
}

fn resolve_hash(repo: &Repository, spec: &str) -> Result<ObjectId> {
    // Strip an optional `blake3:` prefix for short-hash matching.
    let hex = spec.strip_prefix("blake3:").unwrap_or(spec);
    let hex_lower = hex.to_ascii_lowercase();

    if !hex_lower.bytes().all(|b| b.is_ascii_hexdigit()) || hex_lower.is_empty() {
        return Err(CoreError::UnknownRevision(spec.to_string()));
    }

    // Exact full hash.
    if hex_lower.len() == 64 {
        if let Ok(id) = spec.parse::<ObjectId>() {
            if repo.store().has(Category::Commit, &id) {
                return Ok(id);
            }
        }
        return Err(CoreError::UnknownRevision(spec.to_string()));
    }

    // Short-hash prefix match over all commits.
    let matches: Vec<ObjectId> = repo
        .store()
        .list(Category::Commit)?
        .into_iter()
        .filter(|id| id.hex().starts_with(&hex_lower))
        .collect();

    match matches.len() {
        0 => Err(CoreError::UnknownRevision(spec.to_string())),
        1 => Ok(matches.into_iter().next().unwrap()),
        count => Err(CoreError::AmbiguousRevision {
            prefix: spec.to_string(),
            count,
        }),
    }
}

/// Walk the first-parent commit chain starting at `head` (newest first).
pub fn commit_history(repo: &Repository, head: &ObjectId) -> Result<Vec<crate::model::Commit>> {
    let mut out = Vec::new();
    let mut current = Some(head.clone());
    while let Some(id) = current {
        let commit = repo.read_commit(&id)?;
        current = commit.parents.first().cloned();
        out.push(commit);
    }
    Ok(out)
}
