//! `cadvm-core` — the version-control engine for cadvm.
//!
//! This crate owns the repository model: commits, manifests, refs, branches and
//! HEAD, plus the operations over them (snapshot, status, diff, checkout,
//! switch, revert, gc). It is deliberately UI-free — all terminal formatting
//! lives in `cadvm-cli`.
//!
//! Everything here is pure Rust. There is no geometry, no CAD kernel and no FFI;
//! STEP files are treated as opaque text with light metadata scanning (see
//! [`step`]).

pub mod checkout;
pub mod config;
pub mod diff;
pub mod error;
pub mod format;
pub mod gc;
pub mod geom;
pub mod ignore;
pub mod index;
pub mod mesh;
pub mod meshdiff;
pub mod model;
pub mod repo;
pub mod revision;
pub mod snapshot;
pub mod status;
pub mod step;
pub mod verify;
pub mod worktree;

pub use config::Config;
pub use error::{CoreError, Result};
pub use format::CadFormat;
pub use model::{Author, Commit, CommitBody, FileEntry, Manifest, MANIFEST_VERSION};
pub use repo::{Head, Repository, DEFAULT_BRANCH, REPO_DIR};
pub use status::{working_tree_status, WorkingTreeStatus};
pub use step::{EntityTypeCount, StepMetadata};

// Re-export storage primitives so downstream crates can depend on `cadvm-core`
// alone for the common types.
pub use cadvm_store::{BlobRef, Category, ChunkRef, ObjectId, Store};
