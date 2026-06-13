//! Working-tree scanning helpers (find tracked-format files, hash them).

use std::path::{Path, PathBuf};

use cadvm_store::ObjectId;
use walkdir::WalkDir;

use crate::error::{CoreError, Result};
use crate::format::CadFormat;
use crate::ignore::IgnoreList;
use crate::repo::{Repository, REPO_DIR};

/// Scan the working tree for STEP/STP files, returning their repo-relative paths.
///
/// The `.cadvm` directory is always skipped. Other dot-directories are also
/// skipped (they are not expected to hold CAD sources and scanning them is
/// surprising), but dot-*files* are still considered if they carry a tracked
/// extension. Paths matching `.cadvmignore` are excluded.
pub fn scan_step_files(repo: &Repository) -> Result<Vec<PathBuf>> {
    let root = repo.workdir();
    let ignore = IgnoreList::load(repo)?;
    let mut files = Vec::new();

    let walker = WalkDir::new(root).into_iter().filter_entry(|entry| {
        // Always descend into the root itself.
        if entry.depth() == 0 {
            return true;
        }
        let name = entry.file_name().to_string_lossy();
        if entry.file_type().is_dir() {
            // Skip the repo dir, any hidden directory, and ignored directories.
            if name == REPO_DIR || name.starts_with('.') {
                return false;
            }
            match entry.path().strip_prefix(root) {
                Ok(rel) => !ignore.is_ignored(rel),
                Err(_) => true,
            }
        } else {
            true
        }
    });

    for entry in walker {
        let entry = entry.map_err(|e| {
            CoreError::io(
                e.path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| root.to_path_buf()),
                e.into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("walk error")),
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if CadFormat::from_path(path).is_none() {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        if ignore.is_ignored(&rel) {
            continue;
        }
        files.push(rel);
    }

    files.sort();
    Ok(files)
}

/// Absolute path of a repo-relative working-tree path.
pub fn abs_path(repo: &Repository, rel: &Path) -> PathBuf {
    repo.workdir().join(rel)
}

/// Read the bytes of a working-tree file.
pub fn read_working_file(repo: &Repository, rel: &Path) -> Result<Vec<u8>> {
    let path = abs_path(repo, rel);
    std::fs::read(&path).map_err(|e| CoreError::io(&path, e))
}

/// Hash a working-tree file by content (level-1 raw hash), streaming the file in
/// fixed blocks so memory stays constant even for very large STEP files.
pub fn hash_working_file(repo: &Repository, rel: &Path) -> Result<ObjectId> {
    let path = abs_path(repo, rel);
    let file = std::fs::File::open(&path).map_err(|e| CoreError::io(&path, e))?;
    ObjectId::hash_reader(std::io::BufReader::new(file)).map_err(|e| CoreError::io(&path, e))
}
