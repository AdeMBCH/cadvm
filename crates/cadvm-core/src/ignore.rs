//! `.cadvmignore` support: a small, dependency-free pattern matcher.
//!
//! The file lives at the repository root, one pattern per line. Syntax is a
//! deliberately small subset of the usual ignore-file conventions:
//!
//! * blank lines and lines starting with `#` are ignored;
//! * `*` matches any run of characters, `?` matches a single character;
//! * a pattern ending in `/` matches a directory and everything beneath it;
//! * a pattern containing `/` is matched against the whole repo-relative path,
//!   otherwise it is matched against the file name only;
//! * a leading `/` anchors the pattern to the repository root.
//!
//! The `.cadvm` directory is always ignored, independently of this file.

use std::path::Path;

use crate::error::{CoreError, Result};
use crate::repo::Repository;

/// Name of the ignore file at the repository root.
pub const IGNORE_FILE: &str = ".cadvmignore";

/// A compiled set of ignore patterns.
#[derive(Debug, Default, Clone)]
pub struct IgnoreList {
    patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
struct Pattern {
    /// Glob text (without a trailing `/`).
    glob: String,
    /// Match against the whole relative path rather than just the file name.
    full_path: bool,
    /// Directory pattern (ends with `/`): also matches everything beneath it.
    dir: bool,
}

impl IgnoreList {
    /// Load the ignore list from the repository root, if a `.cadvmignore` exists.
    pub fn load(repo: &Repository) -> Result<IgnoreList> {
        let path = repo.workdir().join(IGNORE_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(IgnoreList::default()),
            Err(e) => return Err(CoreError::io(&path, e)),
        };
        Ok(IgnoreList::parse(&text))
    }

    /// Parse ignore patterns from raw text.
    pub fn parse(text: &str) -> IgnoreList {
        let mut patterns = Vec::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let dir = line.ends_with('/');
            let body = line.trim_end_matches('/');
            // A leading `/` only anchors to root; we already match full paths
            // from the root, so simply strip it.
            let body = body.strip_prefix('/').unwrap_or(body);
            let full_path = body.contains('/');
            patterns.push(Pattern {
                glob: body.to_string(),
                full_path,
                dir,
            });
        }
        IgnoreList { patterns }
    }

    /// Whether a repo-relative path should be ignored.
    pub fn is_ignored(&self, rel: &Path) -> bool {
        let rel_str = normalize(rel);
        let file_name = rel
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        self.patterns.iter().any(|p| {
            if p.dir {
                // Match the directory itself or anything beneath it.
                rel_str == p.glob || rel_str.starts_with(&format!("{}/", p.glob))
            } else if p.full_path {
                glob_match(&p.glob, &rel_str)
            } else {
                glob_match(&p.glob, &file_name)
            }
        })
    }
}

/// Normalize a path to forward-slash separated string.
fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Match `text` against a glob `pattern` supporting `*` (any run) and `?` (one
/// char). Iterative backtracking; treats all characters (including `/`)
/// uniformly, which is sufficient for our small pattern set.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ign(s: &str) -> IgnoreList {
        IgnoreList::parse(s)
    }

    #[test]
    fn basename_glob() {
        let l = ign("*.bak\ndraft_*.step\n");
        assert!(l.is_ignored(&PathBuf::from("a/b/old.bak")));
        assert!(l.is_ignored(&PathBuf::from("draft_v1.step")));
        assert!(!l.is_ignored(&PathBuf::from("final.step")));
    }

    #[test]
    fn directory_pattern() {
        let l = ign("build/\n");
        assert!(l.is_ignored(&PathBuf::from("build/part.step")));
        assert!(l.is_ignored(&PathBuf::from("build")));
        assert!(!l.is_ignored(&PathBuf::from("builder/part.step")));
    }

    #[test]
    fn anchored_full_path() {
        let l = ign("/secret/old.step\n");
        assert!(l.is_ignored(&PathBuf::from("secret/old.step")));
        assert!(!l.is_ignored(&PathBuf::from("sub/secret/old.step")));
    }

    #[test]
    fn comments_and_blanks_skipped() {
        let l = ign("# a comment\n\n  \n*.tmp\n");
        assert_eq!(l.patterns.len(), 1);
    }
}
