//! End-to-end tests for the cadvm version-control engine.

use std::fs;
use std::path::Path;

use cadvm_core::checkout;
use cadvm_core::diff;
use cadvm_core::revision;
use cadvm_core::snapshot;
use cadvm_core::status::working_tree_status;
use cadvm_core::{step, CoreError, Repository};

const CUBE_HOLE5: &str = include_str!("../../../tests/fixtures/cube_hole5.step");
const CUBE_HOLE8: &str = include_str!("../../../tests/fixtures/cube_hole8.step");
const TWO_HOLES: &str = include_str!("../../../tests/fixtures/two_holes.stp");

/// A monotonically increasing fake clock so commits are deterministic.
fn ts(n: i64) -> i64 {
    1_700_000_000 + n
}

fn setup() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    (dir, repo)
}

fn write_file(repo: &Repository, name: &str, content: &str) {
    let path = repo.workdir().join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn read_file(repo: &Repository, name: &str) -> String {
    fs::read_to_string(repo.workdir().join(name)).unwrap()
}

#[test]
fn init_creates_repo_structure() {
    let (_d, repo) = setup();
    let root = repo.cadvm_dir();
    for sub in [
        "objects/blobs",
        "objects/chunks",
        "objects/manifests",
        "objects/commits",
        "refs/heads",
        "tmp",
    ] {
        assert!(root.join(sub).is_dir(), "expected directory {sub} to exist");
    }
    assert!(root.join("HEAD").is_file());
    assert!(root.join("index.json").is_file());
    assert!(root.join("refs/heads/main").is_file());

    let head = fs::read_to_string(root.join("HEAD")).unwrap();
    assert_eq!(head.trim(), "ref: refs/heads/main");
}

#[test]
fn snapshot_creates_commit() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    let out = snapshot::snapshot(&repo, "Cube avec trou 5", ts(1)).unwrap();
    assert_eq!(out.file_count, 1);

    let head = repo.head_commit_id().unwrap().expect("HEAD has a commit");
    assert_eq!(head, out.commit_id);
    let commit = repo.read_commit(&head).unwrap();
    assert_eq!(commit.message, "Cube avec trou 5");
    assert!(commit.parents.is_empty());
}

#[test]
fn snapshot_tracks_cad_formats_only() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    write_file(&repo, "plate.stp", TWO_HOLES);
    write_file(&repo, "notes.txt", "not a CAD file");
    write_file(
        &repo,
        "mesh.stl",
        "solid foo\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid foo\n",
    );

    let out = snapshot::snapshot(&repo, "track CAD formats", ts(1)).unwrap();
    // STEP, STP and STL are tracked; the plain text file is not.
    assert_eq!(out.file_count, 3);

    let manifest = repo.head_manifest().unwrap();
    assert!(manifest.files.contains_key(Path::new("piece.step")));
    assert!(manifest.files.contains_key(Path::new("plate.stp")));
    assert!(manifest.files.contains_key(Path::new("mesh.stl")));
    assert!(!manifest.files.contains_key(Path::new("notes.txt")));

    // The STL carries mesh metadata (one triangle), not STEP metadata.
    let stl = &manifest.files[Path::new("mesh.stl")];
    assert!(stl.step_metadata.is_none());
    assert_eq!(stl.mesh_metadata.as_ref().unwrap().triangles, Some(1));
}

#[test]
fn status_detects_clean_tree() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    let status = working_tree_status(&repo).unwrap();
    assert!(status.is_clean(), "status: {status:?}");
    assert_eq!(status.branch.as_deref(), Some("main"));
}

#[test]
fn status_detects_modified_file() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    write_file(&repo, "piece.step", CUBE_HOLE8);
    let status = working_tree_status(&repo).unwrap();
    assert_eq!(status.modified, vec![Path::new("piece.step").to_path_buf()]);
    assert!(status.new.is_empty() && status.deleted.is_empty());
}

#[test]
fn status_detects_new_file() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    write_file(&repo, "plate.stp", TWO_HOLES);
    let status = working_tree_status(&repo).unwrap();
    assert_eq!(status.new, vec![Path::new("plate.stp").to_path_buf()]);
}

#[test]
fn status_detects_deleted_file() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    write_file(&repo, "plate.stp", TWO_HOLES);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    fs::remove_file(repo.workdir().join("plate.stp")).unwrap();
    let status = working_tree_status(&repo).unwrap();
    assert_eq!(status.deleted, vec![Path::new("plate.stp").to_path_buf()]);
}

#[test]
fn log_walks_parent_chain() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "first", ts(1)).unwrap();
    write_file(&repo, "piece.step", CUBE_HOLE8);
    snapshot::snapshot(&repo, "second", ts(2)).unwrap();

    let head = repo.head_commit_id().unwrap().unwrap();
    let history = revision::commit_history(&repo, &head).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].message, "second");
    assert_eq!(history[1].message, "first");
    assert_eq!(history[0].parents, vec![history[1].id.clone()]);
}

#[test]
fn diff_detects_added_removed_modified() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    write_file(&repo, "gone.stp", TWO_HOLES);
    snapshot::snapshot(&repo, "A", ts(1)).unwrap();

    // Modify one, remove one, add one.
    write_file(&repo, "piece.step", CUBE_HOLE8);
    fs::remove_file(repo.workdir().join("gone.stp")).unwrap();
    write_file(&repo, "added.step", TWO_HOLES);
    snapshot::snapshot(&repo, "B", ts(2)).unwrap();

    let a = revision::resolve(&repo, "HEAD~1").unwrap();
    let b = revision::resolve(&repo, "HEAD").unwrap();
    let d = diff::diff_manifests(
        &repo.manifest_of_commit(&a).unwrap(),
        &repo.manifest_of_commit(&b).unwrap(),
    );

    assert_eq!(d.added, vec![Path::new("added.step").to_path_buf()]);
    assert_eq!(d.removed, vec![Path::new("gone.stp").to_path_buf()]);
    assert_eq!(d.modified.len(), 1);
    let m = &d.modified[0];
    assert_eq!(m.path, Path::new("piece.step"));
    assert_ne!(m.raw_hash.0, m.raw_hash.1);
    // hole5 has 5 entities, hole8 has 6.
    assert_eq!(m.entity_count, (Some(5), Some(6)));
}

#[test]
fn branch_create_and_list() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();

    let head = repo.head_commit_id().unwrap().unwrap();
    repo.create_branch("second-hole", &head).unwrap();

    let mut branches = repo.list_branches().unwrap();
    branches.sort();
    assert_eq!(
        branches,
        vec!["main".to_string(), "second-hole".to_string()]
    );
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));

    // Creating it again fails.
    assert!(matches!(
        repo.create_branch("second-hole", &head),
        Err(CoreError::BranchExists(_))
    ));
    // Invalid names are rejected.
    assert!(matches!(
        repo.create_branch("bad name", &head),
        Err(CoreError::InvalidBranchName(_))
    ));
}

#[test]
fn switch_refuses_dirty_tree() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    let head = repo.head_commit_id().unwrap().unwrap();
    repo.create_branch("other", &head).unwrap();

    // Make the working tree dirty.
    write_file(&repo, "piece.step", CUBE_HOLE8);

    let err = checkout::switch(&repo, "other", false).unwrap_err();
    assert!(matches!(err, CoreError::DirtyWorkingTree { .. }));

    // --force allows it.
    checkout::switch(&repo, "other", true).unwrap();
    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("other"));
}

#[test]
fn checkout_refuses_to_overwrite_dirty_file() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();

    // Locally modify without committing.
    write_file(&repo, "piece.step", CUBE_HOLE8);

    let err = checkout::checkout(&repo, "HEAD", &[], false).unwrap_err();
    assert!(matches!(err, CoreError::WouldOverwriteDirtyFile(_)));

    // --force discards the local change and restores HEAD content.
    checkout::checkout(&repo, "HEAD", &[], true).unwrap();
    assert_eq!(read_file(&repo, "piece.step"), CUBE_HOLE5);
}

#[test]
fn revert_head_restores_parent_state() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "trou 5", ts(1)).unwrap();
    write_file(&repo, "piece.step", CUBE_HOLE8);
    snapshot::snapshot(&repo, "trou 8", ts(2)).unwrap();

    let out = checkout::revert(&repo, "HEAD", false, ts(3)).unwrap();

    // Working tree is back to the 5mm hole.
    assert_eq!(read_file(&repo, "piece.step"), CUBE_HOLE5);

    // A new commit was created on top of HEAD with a Revert message.
    let new_commit = repo.read_commit(&out.new_commit_id).unwrap();
    assert_eq!(new_commit.message, "Revert \"trou 8\"");
    let history = revision::commit_history(&repo, &out.new_commit_id).unwrap();
    assert_eq!(history.len(), 3);
    assert!(working_tree_status(&repo).unwrap().is_clean());
}

#[test]
fn branch_delete_refuses_current_and_removes_other() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    let head = repo.head_commit_id().unwrap().unwrap();
    repo.create_branch("tmp", &head).unwrap();

    // Cannot delete the branch HEAD is on.
    assert!(matches!(
        repo.delete_branch("main"),
        Err(CoreError::CannotDeleteCurrentBranch(_))
    ));
    // Deleting another branch works.
    repo.delete_branch("tmp").unwrap();
    assert!(!repo.branch_exists("tmp"));
    // Deleting a missing branch errors.
    assert!(matches!(
        repo.delete_branch("nope"),
        Err(CoreError::NoSuchBranch(_))
    ));
}

#[test]
fn checkout_single_file_only() {
    let (_d, repo) = setup();
    write_file(&repo, "a.step", CUBE_HOLE5);
    write_file(&repo, "b.step", TWO_HOLES);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();

    // Locally change both files.
    write_file(&repo, "a.step", CUBE_HOLE8);
    write_file(&repo, "b.step", CUBE_HOLE8);

    let only = [std::path::PathBuf::from("a.step")];

    // Without --force the dirty-guard still protects the named file.
    assert!(matches!(
        checkout::checkout(&repo, "HEAD", &only, false),
        Err(CoreError::WouldOverwriteDirtyFile(_))
    ));

    // With --force, only a.step is restored; b.step keeps its local change and
    // is never deleted by a path-scoped checkout.
    checkout::checkout(&repo, "HEAD", &only, true).unwrap();
    assert_eq!(read_file(&repo, "a.step"), CUBE_HOLE5);
    assert_eq!(read_file(&repo, "b.step"), CUBE_HOLE8);

    // Restoring a path absent from the revision errors.
    let missing = [std::path::PathBuf::from("ghost.step")];
    assert!(matches!(
        checkout::checkout(&repo, "HEAD", &missing, false),
        Err(CoreError::PathNotInRevision { .. })
    ));
}

#[test]
fn cadvmignore_excludes_matching_files() {
    let (_d, repo) = setup();
    write_file(&repo, "keep.step", CUBE_HOLE5);
    write_file(&repo, "scratch.step", TWO_HOLES);
    write_file(&repo, "build/gen.step", TWO_HOLES);
    write_file(&repo, ".cadvmignore", "scratch.step\nbuild/\n");

    let out = snapshot::snapshot(&repo, "with ignore", ts(1)).unwrap();
    assert_eq!(out.file_count, 1);
    let manifest = repo.head_manifest().unwrap();
    assert!(manifest.files.contains_key(Path::new("keep.step")));
    assert!(!manifest.files.contains_key(Path::new("scratch.step")));
    assert!(!manifest.files.contains_key(Path::new("build/gen.step")));
}

#[test]
fn store_is_chunk_only_no_raw_blob() {
    use cadvm_core::{Category, Store};
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    snapshot::snapshot(&repo, "init", ts(1)).unwrap();

    // No raw blob is written; content lives in chunks and round-trips.
    let store = Store::open(repo.cadvm_dir().join("objects")).unwrap();
    assert_eq!(store.list(Category::Blob).unwrap().len(), 0);
    let manifest = repo.head_manifest().unwrap();
    let entry = manifest.files.values().next().unwrap();
    let content = store.read_file_content(&entry.blob_ref).unwrap();
    assert_eq!(content, CUBE_HOLE5.as_bytes());
}

#[test]
fn hash_cache_reuses_and_invalidates() {
    use cadvm_core::index::HashCache;
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);

    let mut cache = HashCache::load(&repo);
    let h1 = cache.hash(&repo, Path::new("piece.step")).unwrap();
    let h2 = cache.hash(&repo, Path::new("piece.step")).unwrap();
    assert_eq!(h1, h2, "repeat hash is stable");

    // Different content (and size) invalidates the cache.
    write_file(&repo, "piece.step", CUBE_HOLE8);
    let h3 = cache.hash(&repo, Path::new("piece.step")).unwrap();
    assert_ne!(h1, h3);

    // Persisted and reloaded cache stays consistent.
    cache.save(&repo).unwrap();
    let mut reloaded = HashCache::load(&repo);
    assert_eq!(reloaded.hash(&repo, Path::new("piece.step")).unwrap(), h3);
}

#[test]
fn config_records_commit_author() {
    use cadvm_core::config::{self, Config};
    let (_d, repo) = setup();

    // Configure an author, then snapshot.
    let mut cfg = Config::load(&repo).unwrap();
    cfg.set(config::USER_NAME, "Mat");
    cfg.set(config::USER_EMAIL, "mat@enchanted.tools");
    cfg.save(&repo).unwrap();
    assert_eq!(
        Config::load(&repo).unwrap().get(config::USER_NAME),
        Some("Mat")
    );

    write_file(&repo, "piece.step", CUBE_HOLE5);
    let out = snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    let commit = repo.read_commit(&out.commit_id).unwrap();
    let author = commit.author.expect("commit has an author");
    assert_eq!(author.name, "Mat");
    assert_eq!(author.display(), "Mat <mat@enchanted.tools>");
}

#[test]
fn author_falls_back_when_unconfigured() {
    let (_d, repo) = setup();
    write_file(&repo, "piece.step", CUBE_HOLE5);
    let out = snapshot::snapshot(&repo, "init", ts(1)).unwrap();
    let commit = repo.read_commit(&out.commit_id).unwrap();
    // With no config (and assuming no CADVM_AUTHOR_* env), the name falls back.
    let author = commit.author.expect("commit has an author");
    assert!(!author.name.is_empty());
}

/// Real geometric diff via the C++/OCCT helper. Runs only when `CADVM_GEOM_BIN`
/// points at a built `cadvm-geom`; otherwise it skips (no OCCT on the machine).
#[test]
fn geom_diff_on_real_solids_when_helper_available() {
    use cadvm_core::geom;
    use std::path::Path;

    let bin = match std::env::var_os(geom::ENV_GEOM_BIN) {
        Some(b) if Path::new(&b).exists() => std::path::PathBuf::from(b),
        _ => {
            eprintln!("skipping: CADVM_GEOM_BIN not set to an existing binary");
            return;
        }
    };

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let a = fixtures.join("bloc1.step");
    let b = fixtures.join("bloc2.step");
    if !a.exists() || !b.exists() {
        eprintln!("skipping: bloc fixtures not present");
        return;
    }

    let diff = geom::diff_files_with(&bin, &a, &b).unwrap();
    assert!(diff.is_ok(), "geom helper error: {:?}", diff.error);
    assert!(diff.a.as_ref().unwrap().volume > 0.0);
    assert!(diff.b.as_ref().unwrap().volume > 0.0);
    assert!(diff.a.as_ref().unwrap().faces > 0);
    // The two blocs differ, so there is some added and removed material.
    assert!(diff.added.unwrap().volume > 0.0);
    assert!(diff.removed.unwrap().volume > 0.0);
    // Topological face diff is reported.
    let ft = diff.faces_topo.expect("faces_topo present");
    assert!(ft.added + ft.removed > 0);

    // Meshing produces the full input shapes plus boolean pieces.
    let out = std::env::temp_dir().join("cadvm-mesh-test.json");
    let mesh = geom::mesh_files_with(&bin, &a, &b, &out).unwrap();
    let _ = std::fs::remove_file(&out);
    assert!(mesh.is_ok());
    assert!(mesh.total_triangles() > 0);
    let layers = mesh.layers.unwrap();
    // The two blocs differ, so there are added and removed faces.
    assert!(layers.added.triangle_count() > 0);
    assert!(layers.removed.triangle_count() > 0);
}

#[test]
fn step_metadata_extracts_schema_and_entity_counts() {
    let md = step::extract(CUBE_HOLE5.as_bytes()).unwrap();
    assert_eq!(md.file_schema.as_deref(), Some("AUTOMOTIVE_DESIGN"));
    assert_eq!(md.entity_count, Some(5));
    assert!(md.data_line_count.unwrap() >= 5);
    let types: Vec<&str> = md
        .top_entity_types
        .iter()
        .map(|e| e.entity_type.as_str())
        .collect();
    assert!(types.contains(&"CARTESIAN_POINT"));
    assert!(types.contains(&"CYLINDRICAL_SURFACE"));
}
