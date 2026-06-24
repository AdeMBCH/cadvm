//! End-to-end CLI test driving the documented priority flow through the binary.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

const CUBE_HOLE5: &str = include_str!("../../../tests/fixtures/cube_hole5.step");
const CUBE_HOLE8: &str = include_str!("../../../tests/fixtures/cube_hole8.step");
const TWO_HOLES: &str = include_str!("../../../tests/fixtures/two_holes.stp");

fn cadvm(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cadvm").unwrap();
    cmd.current_dir(dir);
    cmd
}

#[test]
fn full_priority_flow() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let piece = dir.join("piece.step");

    cadvm(dir).arg("init").assert().success();

    fs::write(&piece, CUBE_HOLE5).unwrap();
    cadvm(dir)
        .args(["snapshot", "-m", "Cube avec trou 5"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 tracked file"));

    fs::write(&piece, CUBE_HOLE8).unwrap();
    cadvm(dir)
        .args(["snapshot", "-m", "Passage trou 5 vers 8"])
        .assert()
        .success();

    cadvm(dir)
        .arg("log")
        .assert()
        .success()
        .stdout(predicate::str::contains("Passage trou 5 vers 8"))
        .stdout(predicate::str::contains("Cube avec trou 5"));

    cadvm(dir)
        .args(["diff", "HEAD~1", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entities: 5 -> 6"));

    // Revert brings the working file back to the 5mm hole.
    cadvm(dir).args(["revert", "HEAD"]).assert().success();
    assert_eq!(fs::read_to_string(&piece).unwrap(), CUBE_HOLE5);

    cadvm(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Clean working tree."));

    cadvm(dir)
        .args(["branch", "second-hole"])
        .assert()
        .success();
    cadvm(dir)
        .args(["switch", "second-hole"])
        .assert()
        .success();

    fs::write(&piece, TWO_HOLES).unwrap();
    cadvm(dir)
        .args(["snapshot", "-m", "Ajout deuxieme trou 5"])
        .assert()
        .success();

    cadvm(dir)
        .arg("branch")
        .assert()
        .success()
        .stdout(predicate::str::contains("* second-hole"));

    cadvm(dir).args(["switch", "main"]).assert().success();
    cadvm(dir)
        .args(["switch", "second-hole"])
        .assert()
        .success();

    cadvm(dir).arg("gc").assert().success();
}

#[test]
fn snapshot_outside_repo_fails() {
    let tmp = tempfile::tempdir().unwrap();
    cadvm(tmp.path())
        .args(["snapshot", "-m", "no repo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a cadvm repository"));
}

#[test]
fn verify_files_gates_with_exit_code() {
    // Repo-less geometric gate on two files (the eval/CI use case). STL → pure
    // Rust, so no Open CASCADE needed.
    let root = env!("CARGO_MANIFEST_DIR");
    let a = format!("{root}/../../tests/fixtures/block_v1.stl");
    let b = format!("{root}/../../tests/fixtures/block_v2.stl");

    // Passing assertion → exit 0.
    Command::cargo_bin("cadvm")
        .unwrap()
        .args(["verify", "--files", &a, &b, "--expect", "added_tris>0"])
        .assert()
        .success();

    // Failing assertion → non-zero exit (the gate rejects).
    Command::cargo_bin("cadvm")
        .unwrap()
        .args(["verify", "--files", &a, &b, "--expect", "removed_tris<1"])
        .assert()
        .failure();
}

#[test]
fn mcp_handshake_and_tools_list() {
    let tmp = tempfile::tempdir().unwrap();
    // initialize + tools/list do not touch a repository.
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        "\n",
    );
    cadvm(tmp.path())
        .arg("mcp")
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"serverInfo\""))
        .stdout(predicate::str::contains("cadvm_geom_diff"))
        .stdout(predicate::str::contains("cadvm_verify"));
}
