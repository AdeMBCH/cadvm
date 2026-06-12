//! Error type shared across `cadvm-core`.

use std::path::PathBuf;

use cadvm_store::StoreError;

/// Errors produced by repository operations.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("not a cadvm repository (no .cadvm directory found in `{0}` or any parent)")]
    NotARepository(PathBuf),

    #[error("a cadvm repository already exists at `{0}`")]
    AlreadyInitialized(PathBuf),

    #[error("the current branch `{0}` has no commits yet")]
    EmptyBranch(String),

    #[error("HEAD has no parent commit")]
    NoParent,

    #[error("could not resolve revision `{0}`")]
    UnknownRevision(String),

    #[error("ambiguous short hash `{prefix}` matches {count} commits")]
    AmbiguousRevision { prefix: String, count: usize },

    #[error("branch `{0}` already exists")]
    BranchExists(String),

    #[error("branch `{0}` does not exist")]
    NoSuchBranch(String),

    #[error("invalid branch name `{0}`")]
    InvalidBranchName(String),

    #[error("cannot delete the currently checked-out branch `{0}`")]
    CannotDeleteCurrentBranch(String),

    #[error("path `{path}` does not exist in revision `{rev}`")]
    PathNotInRevision { path: PathBuf, rev: String },

    #[error("cannot {action}: working tree has uncommitted changes (use --force to override)")]
    DirtyWorkingTree { action: String },

    #[error(
        "refusing to overwrite locally modified file `{0}` (use --force to discard local changes)"
    )]
    WouldOverwriteDirtyFile(PathBuf),

    #[error("only reverting HEAD is supported in V1 (requested `{0}`)")]
    RevertNonHead(String),

    #[error(
        "geometry helper `{0}` not found — build it (see cpp/build.sh) and set CADVM_GEOM_BIN"
    )]
    GeomBinaryNotFound(PathBuf),

    #[error("geometry helper failed: {0}")]
    GeomFailed(String),

    #[error("i/o error at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Store(#[from] StoreError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CoreError::Io {
            path: path.into(),
            source,
        }
    }
}

/// Convenient result alias.
pub type Result<T> = std::result::Result<T, CoreError>;
