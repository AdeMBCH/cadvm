//! `cadvm mcp` — a minimal Model Context Protocol server over stdio.
//!
//! Exposes cadvm as **tools** an AI agent (Claude Desktop/Code, Cursor, custom
//! MCP clients) can call natively: snapshot an AI iteration, diff it, verify it
//! against expectations, revert it. Speaks JSON-RPC 2.0, one message per line on
//! stdin/stdout — no network, no extra runtime, fitting the local-first model.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use cadvm_core::{checkout, diff, geom, meshdiff, revision, snapshot, verify};
use cadvm_core::{working_tree_status, Repository};

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the stdio MCP server until stdin closes.
pub fn run() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue, // ignore malformed lines
        };
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params");

        let outcome = dispatch(method, params);

        // Notifications (no id) get no response.
        let Some(id) = id else { continue };
        let message = match outcome {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(code_msg) => json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": code_msg.0, "message": code_msg.1},
            }),
        };
        writeln!(out, "{}", serde_json::to_string(&message)?)?;
        out.flush()?;
    }
    Ok(())
}

/// A JSON-RPC error (code, message).
type RpcError = (i64, String);

fn dispatch(method: &str, params: Option<&Value>) -> std::result::Result<Value, RpcError> {
    match method {
        "initialize" => {
            let pv = params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION)
                .to_string();
            Ok(json!({
                "protocolVersion": pv,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "cadvm", "version": env!("CARGO_PKG_VERSION")},
            }))
        }
        "tools/list" => Ok(json!({ "tools": tool_specs() })),
        "tools/call" => {
            let params = params.ok_or((-32602, "missing params".into()))?;
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or((-32602, "missing tool name".into()))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            // Tool errors are reported in the result with `isError`, per MCP.
            match call_tool(name, &args) {
                Ok(value) => Ok(tool_text(&value, false)),
                Err(e) => Ok(tool_text(&Value::String(e.to_string()), true)),
            }
        }
        "ping" => Ok(json!({})),
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

/// Wrap a payload as MCP tool-call content (a single text block of JSON).
fn tool_text(value: &Value, is_error: bool) -> Value {
    let text = if value.is_string() {
        value.as_str().unwrap_or_default().to_string()
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn tool_specs() -> Value {
    let rev = |d: &str| json!({"type": "string", "description": d});
    json!([
        {"name": "cadvm_status", "description": "Working-tree status vs HEAD (new/modified/deleted).",
         "inputSchema": {"type": "object", "properties": {"repo": rev("repo path (default: cwd)")}}},
        {"name": "cadvm_snapshot", "description": "Record a snapshot (commit) of the working tree. Use to pin an AI iteration.",
         "inputSchema": {"type": "object", "properties": {"message": rev("commit message"), "repo": rev("repo path")}, "required": ["message"]}},
        {"name": "cadvm_log", "description": "Commit history (newest first).",
         "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer"}, "repo": rev("repo path")}}},
        {"name": "cadvm_diff", "description": "Metadata diff between two revisions (default HEAD~1..HEAD).",
         "inputSchema": {"type": "object", "properties": {"rev_a": rev("old rev"), "rev_b": rev("new rev"), "repo": rev("repo path")}}},
        {"name": "cadvm_geom_diff", "description": "Geometric diff (added/removed/common) of modified files. STEP needs the cadvm-geom helper; STL/OBJ are pure Rust.",
         "inputSchema": {"type": "object", "properties": {"rev_a": rev("old rev"), "rev_b": rev("new rev"), "file": rev("restrict to one file"), "repo": rev("repo path")}, "required": ["rev_a", "rev_b"]}},
        {"name": "cadvm_verify", "description": "Assert geometric expectations on a diff; returns pass/fail. Expectations like 'added_volume>100'.",
         "inputSchema": {"type": "object", "properties": {"rev_a": rev("old rev"), "rev_b": rev("new rev"), "expects": {"type": "array", "items": {"type": "string"}}, "file": rev("file if several changed"), "repo": rev("repo path")}, "required": ["rev_a", "rev_b"]}},
        {"name": "cadvm_revert", "description": "Revert HEAD: create a commit restoring its parent. Use to undo a bad AI iteration.",
         "inputSchema": {"type": "object", "properties": {"force": {"type": "boolean"}, "repo": rev("repo path")}}},
        {"name": "cadvm_compare_files", "description": "Geometric diff of TWO CAD files on disk — no repository. Ideal for evals: compare a model's output to a reference.",
         "inputSchema": {"type": "object", "properties": {"file_a": rev("old/reference file path"), "file_b": rev("new/candidate file path")}, "required": ["file_a", "file_b"]}},
        {"name": "cadvm_verify_files", "description": "Assert geometric expectations on TWO files on disk (no repo); returns pass/fail. Expectations like 'added_volume>100'.",
         "inputSchema": {"type": "object", "properties": {"file_a": rev("reference file"), "file_b": rev("candidate file"), "expects": {"type": "array", "items": {"type": "string"}}}, "required": ["file_a", "file_b"]}},
    ])
}

// ---- tool implementations --------------------------------------------------

fn call_tool(name: &str, args: &Value) -> Result<Value> {
    match name {
        "cadvm_status" => tool_status(args),
        "cadvm_snapshot" => tool_snapshot(args),
        "cadvm_log" => tool_log(args),
        "cadvm_diff" => tool_diff(args),
        "cadvm_geom_diff" => tool_geom_diff(args),
        "cadvm_verify" => tool_verify(args),
        "cadvm_revert" => tool_revert(args),
        "cadvm_compare_files" => tool_compare_files(args),
        "cadvm_verify_files" => tool_verify_files(args),
        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

fn tool_compare_files(args: &Value) -> Result<Value> {
    let a = arg_str(args, "file_a").context("`file_a` is required")?;
    let b = arg_str(args, "file_b").context("`file_b` is required")?;
    crate::geom_diff_value_for_paths(std::path::Path::new(a), std::path::Path::new(b))
}

fn tool_verify_files(args: &Value) -> Result<Value> {
    let a = arg_str(args, "file_a").context("`file_a` is required")?;
    let b = arg_str(args, "file_b").context("`file_b` is required")?;
    let mut checks = Vec::new();
    if let Some(arr) = args.get("expects").and_then(Value::as_array) {
        for e in arr {
            if let Some(s) = e.as_str() {
                checks.push(verify::parse_check(s).map_err(|m| anyhow::anyhow!(m))?);
            }
        }
    }
    let metrics = crate::metrics_for_paths(std::path::Path::new(a), std::path::Path::new(b))?;
    let report = verify::evaluate(metrics, &checks);
    Ok(json!({ "file_a": a, "file_b": b, "report": report }))
}

fn open(args: &Value) -> Result<Repository> {
    let dir = match args.get("repo").and_then(Value::as_str) {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().context("cannot determine current directory")?,
    };
    Ok(Repository::discover(&dir)?)
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn paths_json(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|p| p.display().to_string()).collect()
}

fn tool_status(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let st = working_tree_status(&repo)?;
    Ok(json!({
        "branch": st.branch,
        "new": paths_json(&st.new),
        "modified": paths_json(&st.modified),
        "deleted": paths_json(&st.deleted),
        "clean": st.is_clean(),
    }))
}

fn tool_snapshot(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let message = arg_str(args, "message").context("`message` is required")?;
    let out = snapshot::snapshot(&repo, message, Utc::now().timestamp())?;
    Ok(json!({
        "commit": out.commit_id.short(),
        "file_count": out.file_count,
        "branch": out.branch,
    }))
}

fn tool_log(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX);
    let head = match repo.head_commit_id()? {
        Some(h) => h,
        None => return Ok(json!({ "commits": [] })),
    };
    let history = revision::commit_history(&repo, &head)?;
    let commits: Vec<Value> = history
        .iter()
        .take(limit as usize)
        .map(|c| {
            let files = repo
                .read_manifest(&c.manifest)
                .map(|m| m.file_count())
                .unwrap_or(0);
            json!({
                "id": c.id.short(),
                "message": c.message,
                "author": c.author.as_ref().map(|a| a.display()),
                "timestamp": c.timestamp_unix,
                "files": files,
            })
        })
        .collect();
    Ok(json!({ "commits": commits }))
}

fn resolve_pair(args: &Value, default: bool) -> (String, String) {
    let a = arg_str(args, "rev_a").map(str::to_string);
    let b = arg_str(args, "rev_b").map(str::to_string);
    match (a, b) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, "HEAD".into()),
        _ if default => ("HEAD~1".into(), "HEAD".into()),
        (None, Some(b)) => ("HEAD~1".into(), b),
        (None, None) => ("HEAD~1".into(), "HEAD".into()),
    }
}

fn tool_diff(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let (a, b) = resolve_pair(args, true);
    let a_id = revision::resolve(&repo, &a)?;
    let b_id = revision::resolve(&repo, &b)?;
    let d = diff::diff_manifests(
        &repo.manifest_of_commit(&a_id)?,
        &repo.manifest_of_commit(&b_id)?,
    );
    Ok(json!({ "rev_a": a_id.short(), "rev_b": b_id.short(), "diff": d }))
}

fn tool_geom_diff(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let (a, b) = resolve_pair(args, true);
    let a_id = revision::resolve(&repo, &a)?;
    let b_id = revision::resolve(&repo, &b)?;
    let manifest_a = repo.manifest_of_commit(&a_id)?;
    let manifest_b = repo.manifest_of_commit(&b_id)?;

    let only = arg_str(args, "file").map(PathBuf::from);
    let targets: Vec<PathBuf> = match &only {
        Some(f) => vec![f.clone()],
        None => diff::diff_manifests(&manifest_a, &manifest_b)
            .modified
            .into_iter()
            .map(|f| f.path)
            .collect(),
    };

    let tmp = repo.tmp_dir();
    let mut files = Vec::new();
    for (i, path) in targets.iter().enumerate() {
        match (manifest_a.files.get(path), manifest_b.files.get(path)) {
            (Some(ea), Some(eb)) if eb.format.is_mesh() => {
                let ca = repo.store().read_file_content(&ea.blob_ref)?;
                let cb = repo.store().read_file_content(&eb.blob_ref)?;
                let m = meshdiff::diff(&ca, &cb, eb.format);
                files.push(json!({"path": path, "kind": "mesh", "diff": m}));
            }
            (Some(ea), Some(eb)) => {
                let fa = crate::extract_version(&repo, &tmp, ea, &format!("mcp-a{i}"))?;
                let fb = crate::extract_version(&repo, &tmp, eb, &format!("mcp-b{i}"))?;
                let g = geom::diff_files(&fa, &fb);
                let _ = std::fs::remove_file(&fa);
                let _ = std::fs::remove_file(&fb);
                files.push(json!({"path": path, "kind": "brep", "diff": g?}));
            }
            _ => files.push(json!({"path": path, "kind": "one-sided"})),
        }
    }
    Ok(json!({ "rev_a": a_id.short(), "rev_b": b_id.short(), "files": files }))
}

fn tool_verify(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let (a, b) = resolve_pair(args, true);
    let a_id = revision::resolve(&repo, &a)?;
    let b_id = revision::resolve(&repo, &b)?;
    let manifest_a = repo.manifest_of_commit(&a_id)?;
    let manifest_b = repo.manifest_of_commit(&b_id)?;

    let modified: Vec<PathBuf> = diff::diff_manifests(&manifest_a, &manifest_b)
        .modified
        .into_iter()
        .map(|f| f.path)
        .collect();
    let file = match arg_str(args, "file") {
        Some(f) => PathBuf::from(f),
        None => match modified.as_slice() {
            [one] => one.clone(),
            [] => anyhow::bail!("no modified files between {a} and {b}"),
            _ => anyhow::bail!("several files changed; pass `file`"),
        },
    };
    let ea = manifest_a.files.get(&file).context("file not in rev_a")?;
    let eb = manifest_b.files.get(&file).context("file not in rev_b")?;

    let mut checks = Vec::new();
    if let Some(arr) = args.get("expects").and_then(Value::as_array) {
        for e in arr {
            if let Some(s) = e.as_str() {
                checks.push(verify::parse_check(s).map_err(|m| anyhow::anyhow!(m))?);
            }
        }
    }
    let metrics = crate::metrics_for_file(&repo, ea, eb)?;
    let report = verify::evaluate(metrics, &checks);
    Ok(json!({ "file": file, "rev_a": a_id.short(), "rev_b": b_id.short(), "report": report }))
}

fn tool_revert(args: &Value) -> Result<Value> {
    let repo = open(args)?;
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    let out = checkout::revert(&repo, "HEAD", force, Utc::now().timestamp())?;
    Ok(json!({
        "new_commit": out.new_commit_id.short(),
        "reverted_commit": out.reverted_commit_id.short(),
        "branch": out.branch,
    }))
}
