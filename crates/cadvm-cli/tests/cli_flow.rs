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
