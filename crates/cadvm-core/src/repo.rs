//! The [`Repository`] handle: on-disk layout, refs, HEAD and object I/O.

use std::path::{Path, PathBuf};

use cadvm_store::{Category, ObjectId, Store};

use crate::error::{CoreError, Result};
use crate::model::{Commit, CommitBody, Manifest};

/// Name of the repository metadata directory.
pub const REPO_DIR: &str = ".cadvm";
/// The default branch created by `init`.
pub const DEFAULT_BRANCH: &str = "main";

/// Where HEAD currently points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    /// Attached to a branch by name (the normal case).
    Branch(String),
    /// Detached, pointing directly at a commit.
    Detached(ObjectId),
}

/// A handle to an on-disk cadvm repository.
#[derive(Debug, Clone)]
pub struct Repository {
    workdir: PathBuf,
    cadvm_dir: PathBuf,
    store: Store,
}

impl Repository {
    /// Initialize a brand-new repository rooted at `workdir`.
    pub fn init(workdir: impl AsRef<Path>) -> Result<Repository> {
        let workdir = workdir.as_ref().to_path_buf();
        let cadvm_dir = workdir.join(REPO_DIR);
        if cadvm_dir.exists() {
            return Err(CoreError::AlreadyInitialized(cadvm_dir));
        }

        // objects/<category> dirs are created by Store::open below.
        let store = Store::open(cadvm_dir.join("objects"))?;
        for sub in ["refs/heads", "tmp"] {
            let dir = cadvm_dir.join(sub);
            std::fs::create_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;
        }

        let repo = Repository {
            workdir,
            cadvm_dir,
            store,
        };

        // Empty default branch ref (no commits yet).
        std::fs::write(repo.ref_path(DEFAULT_BRANCH), b"")
            .map_err(|e| CoreError::io(repo.ref_path(DEFAULT_BRANCH), e))?;
        repo.write_head(&Head::Branch(DEFAULT_BRANCH.to_string()))?;
        repo.write_index_placeholder()?;

        // Empty config so the file is discoverable; `config` populates it.
        std::fs::write(repo.config_path(), b"{}\n")
            .map_err(|e| CoreError::io(repo.config_path(), e))?;

        Ok(repo)
    }

    /// Open the repository containing `start`, searching upward for `.cadvm`.
    pub fn discover(start: impl AsRef<Path>) -> Result<Repository> {
        let start = start.as_ref();
        let start_abs = if start.is_absolute() {
            start.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| CoreError::io(start, e))?
                .join(start)
        };
        let mut dir = start_abs.as_path();
        loop {
            if dir.join(REPO_DIR).is_dir() {
                return Repository::open(dir);
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => return Err(CoreError::NotARepository(start_abs.clone())),
            }
        }
    }

    /// Open a repository whose root (the directory containing `.cadvm`) is known.
    pub fn open(workdir: impl AsRef<Path>) -> Result<Repository> {
        let workdir = workdir.as_ref().to_path_buf();
        let cadvm_dir = workdir.join(REPO_DIR);
        if !cadvm_dir.is_dir() {
            return Err(CoreError::NotARepository(workdir));
        }
        let store = Store::open(cadvm_dir.join("objects"))?;
        Ok(Repository {
            workdir,
            cadvm_dir,
            store,
        })
    }

    // --- paths --------------------------------------------------------------

    /// Repository root (the directory that contains `.cadvm`).
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// The `.cadvm` directory.
    pub fn cadvm_dir(&self) -> &Path {
        &self.cadvm_dir
    }

    /// The shared object store.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn head_path(&self) -> PathBuf {
        self.cadvm_dir.join("HEAD")
    }

    fn refs_heads_dir(&self) -> PathBuf {
        self.cadvm_dir.join("refs/heads")
    }

    fn ref_path(&self, branch: &str) -> PathBuf {
        self.refs_heads_dir().join(branch)
    }

    fn index_path(&self) -> PathBuf {
        self.cadvm_dir.join("index.json")
    }

    /// Path to the repository config file.
    pub fn config_path(&self) -> PathBuf {
        self.cadvm_dir.join("config.json")
    }

    /// The `tmp/` scratch directory.
    pub fn tmp_dir(&self) -> PathBuf {
        self.cadvm_dir.join("tmp")
    }

    fn write_index_placeholder(&self) -> Result<()> {
        // index.json is reserved for a future staging area; V1 snapshots the
        // whole working tree, so we just keep a stable, valid JSON placeholder.
        let path = self.index_path();
        std::fs::write(&path, b"{\n  \"version\": 1,\n  \"entries\": []\n}\n")
            .map_err(|e| CoreError::io(&path, e))
    }

    // --- HEAD & refs --------------------------------------------------------

    /// Read the current HEAD.
    pub fn read_head(&self) -> Result<Head> {
        let path = self.head_path();
        let raw = std::fs::read_to_string(&path).map_err(|e| CoreError::io(&path, e))?;
        let raw = raw.trim();
        if let Some(rest) = raw.strip_prefix("ref:") {
            let refname = rest.trim();
            let branch = refname
                .strip_prefix("refs/heads/")
                .unwrap_or(refname)
                .to_string();
            Ok(Head::Branch(branch))
        } else {
            let id = raw
                .parse::<ObjectId>()
                .map_err(|_| CoreError::UnknownRevision(raw.to_string()))?;
            Ok(Head::Detached(id))
        }
    }

    /// Overwrite HEAD.
    pub fn write_head(&self, head: &Head) -> Result<()> {
        let path = self.head_path();
        let contents = match head {
            Head::Branch(name) => format!("ref: refs/heads/{name}\n"),
            Head::Detached(id) => format!("{}\n", id.canonical()),
        };
        std::fs::write(&path, contents).map_err(|e| CoreError::io(&path, e))
    }

    /// The branch name HEAD is attached to, if any.
    pub fn current_branch(&self) -> Result<Option<String>> {
        match self.read_head()? {
            Head::Branch(name) => Ok(Some(name)),
            Head::Detached(_) => Ok(None),
        }
    }

    /// Read a branch ref. Returns `None` if the branch has no commit yet.
    pub fn read_ref(&self, branch: &str) -> Result<Option<ObjectId>> {
        let path = self.ref_path(branch);
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let s = s.trim();
                if s.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(s.parse::<ObjectId>().map_err(|_| {
                        CoreError::UnknownRevision(format!("refs/heads/{branch}"))
                    })?))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NoSuchBranch(branch.to_string()))
            }
            Err(e) => Err(CoreError::io(&path, e)),
        }
    }

    /// Point a branch ref at a commit (creating the ref file if needed).
    pub fn write_ref(&self, branch: &str, commit: &ObjectId) -> Result<()> {
        let path = self.ref_path(branch);
        std::fs::write(&path, format!("{}\n", commit.canonical()))
            .map_err(|e| CoreError::io(&path, e))
    }

    /// Whether a branch exists.
    pub fn branch_exists(&self, branch: &str) -> bool {
        self.ref_path(branch).exists()
    }

    /// List all branch names, sorted.
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let dir = self.refs_heads_dir();
        let mut names = Vec::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| CoreError::io(&dir, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| CoreError::io(&dir, e))?;
            if entry
                .file_type()
                .map_err(|e| CoreError::io(entry.path(), e))?
                .is_file()
            {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Create a new branch pointing at `commit`. Errors if it already exists or
    /// the name is invalid.
    pub fn create_branch(&self, name: &str, commit: &ObjectId) -> Result<()> {
        validate_branch_name(name)?;
        if self.branch_exists(name) {
            return Err(CoreError::BranchExists(name.to_string()));
        }
        self.write_ref(name, commit)
    }

    /// Delete a branch ref. Refuses to delete the branch HEAD is on.
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        if !self.branch_exists(name) {
            return Err(CoreError::NoSuchBranch(name.to_string()));
        }
        if self.current_branch()?.as_deref() == Some(name) {
            return Err(CoreError::CannotDeleteCurrentBranch(name.to_string()));
        }
        let path = self.ref_path(name);
        std::fs::remove_file(&path).map_err(|e| CoreError::io(&path, e))
    }

    // --- HEAD commit resolution --------------------------------------------

    /// The commit id HEAD currently resolves to, if any.
    pub fn head_commit_id(&self) -> Result<Option<ObjectId>> {
        match self.read_head()? {
            Head::Branch(name) => match self.read_ref(&name) {
                Ok(opt) => Ok(opt),
                // A fresh repo's default branch ref exists but is empty; treat a
                // missing ref the same as "no commit yet".
                Err(CoreError::NoSuchBranch(_)) => Ok(None),
                Err(e) => Err(e),
            },
            Head::Detached(id) => Ok(Some(id)),
        }
    }

    // --- object I/O ---------------------------------------------------------

    /// Persist a commit body and return its content id.
    pub fn write_commit(&self, body: &CommitBody) -> Result<ObjectId> {
        Ok(self.store.put_json(Category::Commit, body)?)
    }

    /// Read a commit by id.
    pub fn read_commit(&self, id: &ObjectId) -> Result<Commit> {
        let body: CommitBody = self.store.get_json(Category::Commit, id)?;
        Ok(Commit::from_body(id.clone(), body))
    }

    /// Persist a manifest and return its content id.
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<ObjectId> {
        Ok(self.store.put_json(Category::Manifest, manifest)?)
    }

    /// Read a manifest by id.
    pub fn read_manifest(&self, id: &ObjectId) -> Result<Manifest> {
        Ok(self.store.get_json(Category::Manifest, id)?)
    }

    /// Read the manifest belonging to a commit.
    pub fn manifest_of_commit(&self, commit: &ObjectId) -> Result<Manifest> {
        let commit = self.read_commit(commit)?;
        self.read_manifest(&commit.manifest)
    }

    /// The manifest HEAD points to, or an empty manifest if there are no commits.
    pub fn head_manifest(&self) -> Result<Manifest> {
        match self.head_commit_id()? {
            Some(id) => self.manifest_of_commit(&id),
            None => Ok(Manifest::empty()),
        }
    }
}

/// Validate a branch name: non-empty, no path separators, whitespace, or
/// leading dash, and not a dotted special name.
pub fn validate_branch_name(name: &str) -> Result<()> {
    let invalid = name.is_empty()
        || name.starts_with('-')
        || name == "."
        || name == ".."
        || name.contains("..")
        || name.contains(|c: char| c.is_whitespace() || c == '/' || c == '\\' || c == ':');
    if invalid {
        Err(CoreError::InvalidBranchName(name.to_string()))
    } else {
        Ok(())
    }
}
