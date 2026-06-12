//! `cadvm` — the CAD Version Manager command-line interface.
//!
//! This binary is a thin presentation layer over `cadvm-core`: it parses
//! arguments with clap, calls into the engine, and formats results for the
//! terminal. All repository logic lives in the core crate.

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use clap::{CommandFactory, Parser, Subcommand};

use cadvm_core::checkout;
use cadvm_core::config::Config;
use cadvm_core::diff::{self, ManifestDiff};
use cadvm_core::gc;
use cadvm_core::geom;
use cadvm_core::model::FileEntry;
use cadvm_core::revision;
use cadvm_core::snapshot;
use cadvm_core::status::working_tree_status;
use cadvm_core::Repository;

mod ui;
mod viewer;

/// CAD Version Manager — local-first version control for STEP/STP files.
#[derive(Parser, Debug)]
#[command(name = "cadvm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize a new repository in the current directory.
    Init,

    /// Record a snapshot of the working tree.
    Snapshot {
        /// Commit message.
        #[arg(short, long)]
        message: String,
    },

    /// Show working-tree status relative to HEAD.
    Status,

    /// Show the commit history of HEAD.
    Log,

    /// Show the details of a single commit (metadata of each file).
    Show {
        /// Revision to show (defaults to HEAD).
        #[arg(default_value = "HEAD")]
        rev: String,
    },

    /// Show changes between two revisions (default: HEAD~1..HEAD).
    Diff {
        /// Left/old revision.
        rev_a: Option<String>,
        /// Right/new revision.
        rev_b: Option<String>,
    },

    /// Restore the working tree to a revision (does not move the branch).
    Checkout {
        /// Revision to restore (hash, short hash, branch, HEAD, HEAD~N).
        rev: String,
        /// Restrict the restore to these files (after `--`); nothing is deleted.
        #[arg(last = true)]
        paths: Vec<PathBuf>,
        /// Overwrite locally modified files.
        #[arg(long)]
        force: bool,
    },

    /// List branches, create a new branch, or delete one with `-d`.
    Branch {
        /// Name of the branch to create (or delete with `-d`). Omit to list.
        name: Option<String>,
        /// Delete the named branch instead of creating it.
        #[arg(short = 'd', long = "delete")]
        delete: bool,
    },

    /// Switch to another branch, restoring its files.
    Switch {
        /// Branch to switch to.
        name: String,
        /// Switch even if the working tree is dirty.
        #[arg(long)]
        force: bool,
    },

    /// Revert HEAD by creating a commit that restores its parent's state.
    Revert {
        /// Revision to revert (V1: must be HEAD).
        rev: String,
        /// Revert even if the working tree is dirty.
        #[arg(long)]
        force: bool,
    },

    /// Remove objects unreachable from any ref.
    Gc {
        /// Show what would be removed without deleting anything (default).
        #[arg(long)]
        dry_run: bool,
        /// Actually delete the unreferenced objects.
        #[arg(long)]
        prune: bool,
    },

    /// Geometric diff between two revisions (requires the cadvm-geom helper).
    GeomDiff {
        /// Left/old revision.
        rev_a: String,
        /// Right/new revision.
        rev_b: String,
        /// Restrict to these files (after `--`); default: all modified files.
        #[arg(last = true)]
        paths: Vec<PathBuf>,
    },

    /// Generate a standalone 3D HTML viewer of the geometric diff (needs cadvm-geom).
    View {
        /// Left/old revision.
        rev_a: String,
        /// Right/new revision.
        rev_b: String,
        /// Output HTML path (default: cadvm-view.html).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Open the result in the default browser.
        #[arg(long)]
        open: bool,
        /// File to view (after `--`); required if several files changed.
        #[arg(last = true)]
        paths: Vec<PathBuf>,
    },

    /// Launch the interactive terminal dashboard.
    Ui,

    /// Print a shell completion script (bash, zsh, fish, elvish, powershell).
    Completions {
        /// Target shell.
        shell: clap_complete::Shell,
    },

    /// Get, set or list repository config (e.g. user.name, user.email).
    Config {
        /// Config key (e.g. `user.name`). Omit to list all settings.
        key: Option<String>,
        /// Value to set. Omit to read the key.
        value: Option<String>,
    },
}

fn main() {
    reset_sigpipe();
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

/// Restore the default `SIGPIPE` disposition on Unix.
///
/// Rust ignores `SIGPIPE` by default, which turns a closed downstream pipe
/// (e.g. `cadvm log | head`) into a panic on the next write. Resetting to
/// `SIG_DFL` makes cadvm behave like a normal Unix tool: it exits quietly when
/// the reader goes away.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: installing the default handler for SIGPIPE is a well-defined,
    // process-wide operation with no memory implications.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init => cmd_init(),
        Command::Snapshot { message } => cmd_snapshot(&message),
        Command::Status => cmd_status(),
        Command::Log => cmd_log(),
        Command::Show { rev } => cmd_show(&rev),
        Command::Diff { rev_a, rev_b } => cmd_diff(rev_a, rev_b),
        Command::Checkout { rev, paths, force } => cmd_checkout(&rev, &paths, force),
        Command::Branch { name, delete } => cmd_branch(name, delete),
        Command::Switch { name, force } => cmd_switch(&name, force),
        Command::Revert { rev, force } => cmd_revert(&rev, force),
        Command::Gc { dry_run, prune } => cmd_gc(dry_run, prune),
        Command::GeomDiff {
            rev_a,
            rev_b,
            paths,
        } => cmd_geom_diff(&rev_a, &rev_b, &paths),
        Command::View {
            rev_a,
            rev_b,
            output,
            open,
            paths,
        } => cmd_view(&rev_a, &rev_b, output, open, &paths),
        Command::Ui => ui::run(open_repo()?),
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "cadvm", &mut std::io::stdout());
            Ok(())
        }
        Command::Config { key, value } => cmd_config(key, value),
    }
}

/// Open the repository containing the current directory.
fn open_repo() -> Result<Repository> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;
    Ok(Repository::discover(&cwd)?)
}

fn cmd_init() -> Result<()> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;
    match Repository::init(&cwd) {
        Ok(repo) => {
            println!(
                "Initialized empty cadvm repository in {}",
                repo.cadvm_dir().display()
            );
            Ok(())
        }
        Err(cadvm_core::CoreError::AlreadyInitialized(path)) => {
            println!("A cadvm repository already exists at {}", path.display());
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn cmd_snapshot(message: &str) -> Result<()> {
    let repo = open_repo()?;
    let now = Utc::now().timestamp();
    let outcome = snapshot::snapshot(&repo, message, now)?;
    if outcome.file_count == 0 {
        println!("Snapshot created with 0 tracked files.");
    } else {
        let noun = if outcome.file_count == 1 {
            "file"
        } else {
            "files"
        };
        println!(
            "Snapshot created with {} tracked {}.",
            outcome.file_count, noun
        );
    }
    println!("  commit {}", outcome.commit_id.short());
    if let Some(branch) = outcome.branch {
        println!("  branch {branch}");
    }
    Ok(())
}

fn cmd_status() -> Result<()> {
    let repo = open_repo()?;
    let status = working_tree_status(&repo)?;

    match &status.branch {
        Some(branch) => println!("On branch {branch}"),
        None => {
            if let Some(id) = repo.head_commit_id()? {
                println!("HEAD detached at {}", id.short());
            }
        }
    }
    println!();

    if !status.new.is_empty() {
        println!("New:");
        for path in &status.new {
            println!("  {}", path.display());
        }
        println!();
    }
    if !status.modified.is_empty() {
        println!("Modified:");
        for path in &status.modified {
            println!("  {}", path.display());
        }
        println!();
    }
    if !status.deleted.is_empty() {
        println!("Deleted:");
        for path in &status.deleted {
            println!("  {}", path.display());
        }
        println!();
    }
    if status.is_clean() {
        println!("Clean working tree.");
    }
    Ok(())
}

fn cmd_log() -> Result<()> {
    let repo = open_repo()?;
    let head = match repo.head_commit_id()? {
        Some(id) => id,
        None => {
            println!("No commits yet.");
            return Ok(());
        }
    };

    let history = revision::commit_history(&repo, &head)?;
    for (i, commit) in history.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let manifest = repo.read_manifest(&commit.manifest)?;
        println!("commit {}", commit.id.short());
        if let Some(author) = &commit.author {
            println!("Author: {}", author.display());
        }
        println!("Date: {}", format_timestamp(commit.timestamp_unix));
        println!("Message: {}", commit.message);
        println!("Files: {}", manifest.file_count());
    }
    Ok(())
}

fn cmd_show(rev: &str) -> Result<()> {
    let repo = open_repo()?;
    let id = revision::resolve(&repo, rev).with_context(|| format!("resolving `{rev}`"))?;
    let commit = repo.read_commit(&id)?;
    let manifest = repo.read_manifest(&commit.manifest)?;

    println!("commit {}", commit.id.canonical());
    if !commit.parents.is_empty() {
        let parents: Vec<String> = commit
            .parents
            .iter()
            .map(|p| p.short().to_string())
            .collect();
        println!("Parents: {}", parents.join(", "));
    }
    if let Some(author) = &commit.author {
        println!("Author: {}", author.display());
    }
    println!("Date: {}", format_timestamp(commit.timestamp_unix));
    println!("Message: {}", commit.message);
    println!("Files: {}", manifest.file_count());

    for entry in manifest.files.values() {
        println!();
        println!("  {}", entry.path.display());
        println!("    format: {}", entry.format.extension());
        println!("    size: {} bytes", entry.size_bytes);
        println!("    raw_hash: {}", entry.raw_hash.short());
        println!("    chunks: {}", entry.blob_ref.chunks.len());
        if let Some(lines) = entry.line_count {
            println!("    lines: {lines}");
        }
        if let Some(md) = &entry.step_metadata {
            println!("    schema: {}", opt_str(&md.file_schema));
            println!("    entities: {}", opt_u64(md.entity_count));
            if !md.top_entity_types.is_empty() {
                let top: Vec<String> = md
                    .top_entity_types
                    .iter()
                    .take(5)
                    .map(|t| format!("{}×{}", t.entity_type, t.count))
                    .collect();
                println!("    top types: {}", top.join(", "));
            }
        }
    }
    Ok(())
}

fn cmd_diff(rev_a: Option<String>, rev_b: Option<String>) -> Result<()> {
    let repo = open_repo()?;

    let (a_spec, b_spec) = match (rev_a, rev_b) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, "HEAD".to_string()),
        (None, _) => ("HEAD~1".to_string(), "HEAD".to_string()),
    };

    let a_id =
        revision::resolve(&repo, &a_spec).with_context(|| format!("resolving `{a_spec}`"))?;
    let b_id =
        revision::resolve(&repo, &b_spec).with_context(|| format!("resolving `{b_spec}`"))?;

    let manifest_a = repo.manifest_of_commit(&a_id)?;
    let manifest_b = repo.manifest_of_commit(&b_id)?;
    let d = diff::diff_manifests(&manifest_a, &manifest_b);

    println!("Diff {}..{}", a_id.short(), b_id.short());
    print_diff(&d);
    Ok(())
}

fn print_diff(d: &ManifestDiff) {
    if d.is_empty() {
        println!("\nNo changes.");
        return;
    }
    if !d.added.is_empty() {
        println!("\nAdded:");
        for path in &d.added {
            println!("  {}", path.display());
        }
    }
    if !d.removed.is_empty() {
        println!("\nRemoved:");
        for path in &d.removed {
            println!("  {}", path.display());
        }
    }
    if !d.modified.is_empty() {
        println!("\nModified:");
        for f in &d.modified {
            println!("  {}", f.path.display());
            println!("    size: {} -> {}", f.size_bytes.0, f.size_bytes.1);
            println!(
                "    raw_hash: {} -> {}",
                f.raw_hash.0.short(),
                f.raw_hash.1.short()
            );
            println!(
                "    lines: {} -> {}",
                opt_u64(f.line_count.0),
                opt_u64(f.line_count.1)
            );
            println!(
                "    schema: {} -> {}",
                opt_str(&f.schema.0),
                opt_str(&f.schema.1)
            );
            println!(
                "    entities: {} -> {}",
                opt_u64(f.entity_count.0),
                opt_u64(f.entity_count.1)
            );
        }
    }
}

fn cmd_checkout(rev: &str, paths: &[PathBuf], force: bool) -> Result<()> {
    let repo = open_repo()?;
    let outcome = checkout::checkout(&repo, rev, paths, force)?;
    if paths.is_empty() {
        println!("Restored working tree to {}", outcome.commit_id.short());
    } else {
        println!(
            "Restored {} file(s) from {}",
            paths.len(),
            outcome.commit_id.short()
        );
    }
    print_restore(&outcome.restore);
    println!("(HEAD and the current branch are unchanged — this is a restore-like checkout.)");
    Ok(())
}

fn cmd_branch(name: Option<String>, delete: bool) -> Result<()> {
    let repo = open_repo()?;
    if delete {
        let name = name.context("`branch -d` requires a branch name")?;
        repo.delete_branch(&name)?;
        println!("Deleted branch {name}");
        return Ok(());
    }
    match name {
        None => {
            let current = repo.current_branch()?;
            let branches = repo.list_branches()?;
            if branches.is_empty() {
                println!("No branches yet.");
            }
            for branch in branches {
                let marker = if Some(&branch) == current.as_ref() {
                    "*"
                } else {
                    " "
                };
                println!("{marker} {branch}");
            }
            Ok(())
        }
        Some(name) => {
            let head = repo
                .head_commit_id()?
                .context("cannot create a branch before the first commit")?;
            repo.create_branch(&name, &head)?;
            println!("Created branch {name} at {}", head.short());
            Ok(())
        }
    }
}

fn cmd_switch(name: &str, force: bool) -> Result<()> {
    let repo = open_repo()?;
    let outcome = checkout::switch(&repo, name, force)?;
    match outcome.commit_id {
        Some(id) => println!("Switched to branch {} at {}", outcome.branch, id.short()),
        None => println!("Switched to branch {} (no commits yet)", outcome.branch),
    }
    print_restore(&outcome.restore);
    Ok(())
}

fn cmd_revert(rev: &str, force: bool) -> Result<()> {
    let repo = open_repo()?;
    let now = Utc::now().timestamp();
    let outcome = checkout::revert(&repo, rev, force, now)?;
    println!(
        "Reverted {} -> new commit {}",
        outcome.reverted_commit_id.short(),
        outcome.new_commit_id.short()
    );
    print_restore(&outcome.restore);
    Ok(())
}

fn cmd_gc(dry_run: bool, prune: bool) -> Result<()> {
    let repo = open_repo()?;
    let plan = gc::plan(&repo)?;

    if plan.is_empty() {
        println!("Nothing to collect; the object store is fully referenced.");
        return Ok(());
    }

    println!("Unreferenced objects:");
    println!("  commits:   {}", plan.commits.len());
    println!("  manifests: {}", plan.manifests.len());
    println!("  blobs:     {}", plan.blobs.len());
    println!("  chunks:    {}", plan.chunks.len());
    println!("  total:     {}", plan.total());

    if prune && !dry_run {
        let removed = gc::prune(&repo, &plan)?;
        println!("Pruned {removed} objects.");
    } else {
        println!("\nThis was a dry run. Re-run with `--prune` to delete these objects.");
    }
    Ok(())
}

fn cmd_geom_diff(rev_a: &str, rev_b: &str, paths: &[PathBuf]) -> Result<()> {
    let repo = open_repo()?;
    let a_id = revision::resolve(&repo, rev_a).with_context(|| format!("resolving `{rev_a}`"))?;
    let b_id = revision::resolve(&repo, rev_b).with_context(|| format!("resolving `{rev_b}`"))?;
    let manifest_a = repo.manifest_of_commit(&a_id)?;
    let manifest_b = repo.manifest_of_commit(&b_id)?;

    // Default to the set of modified files; otherwise the explicit paths.
    let targets: Vec<PathBuf> = if paths.is_empty() {
        diff::diff_manifests(&manifest_a, &manifest_b)
            .modified
            .into_iter()
            .map(|f| f.path)
            .collect()
    } else {
        paths.to_vec()
    };

    println!("Geometric diff {}..{}", a_id.short(), b_id.short());
    if targets.is_empty() {
        println!("No modified STEP files to compare.");
        return Ok(());
    }

    let tmp = repo.tmp_dir();
    for (i, path) in targets.iter().enumerate() {
        println!("\n  {}", path.display());
        match (manifest_a.files.get(path), manifest_b.files.get(path)) {
            (Some(entry_a), Some(entry_b)) => {
                let file_a = extract_version(&repo, &tmp, entry_a, &format!("a{i}"))?;
                let file_b = extract_version(&repo, &tmp, entry_b, &format!("b{i}"))?;
                let result = geom::diff_files(&file_a, &file_b);
                let _ = std::fs::remove_file(&file_a);
                let _ = std::fs::remove_file(&file_b);
                print_geom_result(result?);
            }
            _ => println!("    present on only one side — geometric diff skipped"),
        }
    }
    Ok(())
}

fn cmd_view(
    rev_a: &str,
    rev_b: &str,
    output: Option<PathBuf>,
    open: bool,
    paths: &[PathBuf],
) -> Result<()> {
    let repo = open_repo()?;
    let a_id = revision::resolve(&repo, rev_a).with_context(|| format!("resolving `{rev_a}`"))?;
    let b_id = revision::resolve(&repo, rev_b).with_context(|| format!("resolving `{rev_b}`"))?;
    let manifest_a = repo.manifest_of_commit(&a_id)?;
    let manifest_b = repo.manifest_of_commit(&b_id)?;

    // Pick the single file to view.
    let modified: Vec<PathBuf> = diff::diff_manifests(&manifest_a, &manifest_b)
        .modified
        .into_iter()
        .map(|f| f.path)
        .collect();
    let file = match paths {
        [] => match modified.as_slice() {
            [one] => one.clone(),
            [] => anyhow::bail!("no modified STEP files between {rev_a} and {rev_b}"),
            many => {
                let list = many
                    .iter()
                    .map(|p| format!("  {}", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                anyhow::bail!("several files changed; pick one with `-- <file>`:\n{list}");
            }
        },
        [one] => one.clone(),
        _ => anyhow::bail!("the 3D viewer handles one file at a time"),
    };

    let entry_a = manifest_a
        .files
        .get(&file)
        .with_context(|| format!("`{}` not present in {rev_a}", file.display()))?;
    let entry_b = manifest_b
        .files
        .get(&file)
        .with_context(|| format!("`{}` not present in {rev_b}", file.display()))?;

    let tmp = repo.tmp_dir();
    let path_a = extract_version(&repo, &tmp, entry_a, "view-a")?;
    let path_b = extract_version(&repo, &tmp, entry_b, "view-b")?;
    let out_json = tmp.join("view-mesh.json");

    let mesh = geom::mesh_files(&path_a, &path_b, &out_json);
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
    let mesh = mesh?;
    if !mesh.is_ok() {
        let _ = std::fs::remove_file(&out_json);
        anyhow::bail!(
            "geometry helper error: {}",
            mesh.error.as_deref().unwrap_or("unknown")
        );
    }

    let mesh_json = std::fs::read_to_string(&out_json)
        .with_context(|| format!("reading {}", out_json.display()))?;
    let _ = std::fs::remove_file(&out_json);

    let title = format!("{}  ({}..{})", file.display(), a_id.short(), b_id.short());
    let html = viewer::render(&title, &mesh_json);

    let out_path = output.unwrap_or_else(|| PathBuf::from("cadvm-view.html"));
    std::fs::write(&out_path, html).with_context(|| format!("writing {}", out_path.display()))?;

    println!(
        "Wrote 3D viewer to {} ({} triangles).",
        out_path.display(),
        mesh.total_triangles()
    );
    if mesh.total_triangles() == 0 {
        println!("(meshes are empty — the two versions are geometrically identical)");
    }

    if open {
        open_in_browser(&out_path);
    } else {
        println!("Open it in a browser to explore (drag to rotate, scroll to zoom).");
    }
    Ok(())
}

/// Best-effort: open a file in the platform's default application.
fn open_in_browser(path: &std::path::Path) {
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmd = "";

    if cmd.is_empty() {
        return;
    }
    match std::process::Command::new(cmd).arg(path).spawn() {
        Ok(_) => println!("Opening {} …", path.display()),
        Err(e) => eprintln!("warning: could not open browser ({e})"),
    }
}

/// Materialize a manifest entry's content to a temp file for the helper to read.
fn extract_version(
    repo: &Repository,
    tmp: &std::path::Path,
    entry: &FileEntry,
    tag: &str,
) -> Result<PathBuf> {
    let content = repo.store().read_file_content(&entry.blob_ref)?;
    let ext = entry.format.extension();
    let dest = tmp.join(format!("geom-{tag}.{ext}"));
    std::fs::write(&dest, content)
        .with_context(|| format!("writing temp file {}", dest.display()))?;
    Ok(dest)
}

fn print_geom_result(result: geom::GeomDiff) {
    if !result.is_ok() {
        println!(
            "    geometry error: {}",
            result.error.as_deref().unwrap_or("unknown")
        );
        return;
    }
    let (a, b) = match (&result.a, &result.b) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            println!("    (no metrics reported)");
            return;
        }
    };

    println!("    volume:  {:.3} -> {:.3}", a.volume, b.volume);
    println!("    area:    {:.3} -> {:.3}", a.area, b.area);
    // Prefer solids, but fall back to shells (sewn-shell bodies report 0 solids).
    let body = |m: &geom::ShapeMetrics| {
        if m.solids > 0 {
            format!("{} solids", m.solids)
        } else {
            format!("{} shells", m.shells)
        }
    };
    println!("    bodies:  {} -> {}", body(a), body(b));
    println!("    faces:   {} -> {}", a.faces, b.faces);
    if let (Some(ba), Some(bb)) = (&a.bbox, &b.bbox) {
        println!(
            "    bbox:    {} -> {}",
            fmt_size(ba.size()),
            fmt_size(bb.size())
        );
    }
    if let Some(c) = &result.common {
        println!("    common:  vol {:.3} ({} faces)", c.volume, c.faces);
    }
    if let Some(ad) = &result.added {
        println!("    added:   vol {:.3} ({} faces)", ad.volume, ad.faces);
    }
    if let Some(rm) = &result.removed {
        println!("    removed: vol {:.3} ({} faces)", rm.volume, rm.faces);
    }
    if let Some(ft) = &result.faces_topo {
        println!(
            "    faces (topo): {} common, {} added, {} removed",
            ft.common, ft.added, ft.removed
        );
    }
}

fn fmt_size(s: [f64; 3]) -> String {
    format!("{:.2}×{:.2}×{:.2}", s[0], s[1], s[2])
}

fn cmd_config(key: Option<String>, value: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    match (key, value) {
        // List all settings.
        (None, _) => {
            let config = Config::load(&repo)?;
            let mut any = false;
            for (k, v) in config.entries() {
                println!("{k}={v}");
                any = true;
            }
            if !any {
                println!("No config set. Try: cadvm config user.name \"Your Name\"");
            }
        }
        // Read a single key.
        (Some(key), None) => {
            let config = Config::load(&repo)?;
            match config.get(&key) {
                Some(v) => println!("{v}"),
                None => anyhow::bail!("config key `{key}` is not set"),
            }
        }
        // Set a key.
        (Some(key), Some(value)) => {
            let mut config = Config::load(&repo)?;
            config.set(&key, &value);
            config.save(&repo)?;
            println!("Set {key}={value}");
        }
    }
    Ok(())
}

// --- formatting helpers -----------------------------------------------------

fn print_restore(restore: &checkout::RestoreOutcome) {
    if !restore.written.is_empty() {
        println!("  Restored:");
        for path in &restore.written {
            println!("    {}", path.display());
        }
    }
    if !restore.deleted.is_empty() {
        println!("  Removed:");
        for path in &restore.deleted {
            println!("    {}", path.display());
        }
    }
    if restore.written.is_empty() && restore.deleted.is_empty() {
        println!("  Working tree already matches.");
    }
}

fn format_timestamp(unix: i64) -> String {
    match Utc.timestamp_opt(unix, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => unix.to_string(),
    }
}

fn opt_u64(v: Option<u64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "?".to_string())
}

fn opt_str(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "?".to_string())
}
