use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn list_prints_known_targets() {
    let mut cmd = Command::cargo_bin("acari").expect("binary exists");

    cmd.arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cargo Registry"))
        .stdout(predicate::str::contains("NPM Cache"));
}

#[test]
fn headless_with_unknown_target_shows_message() {
    let mut cmd = Command::cargo_bin("acari").expect("binary exists");

    cmd.args(["--headless", "--target", "target-that-does-not-exist"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No scan targets matched your filters.",
        ));
}

#[test]
fn headless_bin_with_unknown_target_shows_message() {
    let mut cmd = Command::cargo_bin("headless_cleaner").expect("binary exists");

    cmd.args(["--target", "target-that-does-not-exist"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No scan targets matched your filters.",
        ));
}

#[test]
fn headless_scan_path_scans_custom_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let scan_root = temp.path().join("scan-root");
    fs::create_dir_all(&scan_root).expect("create root");
    fs::write(scan_root.join("a.txt"), b"abc").expect("write file");
    fs::write(scan_root.join("b.txt"), b"12345").expect("write file");

    let mut cmd = Command::cargo_bin("acari").expect("binary exists");
    cmd.args([
        "--headless",
        "--target",
        "target-that-does-not-exist",
        "--scan-path",
        scan_root.to_string_lossy().as_ref(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("[done] Custom Path 1"))
    .stdout(predicate::str::contains("8 bytes"))
    .stdout(predicate::str::contains(
        "Scan finished. Total reclaimable bytes: 8",
    ));
}

#[test]
fn headless_scan_path_with_clean_empties_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let scan_root = temp.path().join("clean-root");
    fs::create_dir_all(&scan_root).expect("create root");
    fs::write(scan_root.join("a.txt"), b"abc").expect("write file");
    fs::write(scan_root.join("b.txt"), b"12345").expect("write file");

    let mut cmd = Command::cargo_bin("acari").expect("binary exists");
    cmd.args([
        "--headless",
        "--clean",
        "--yes",
        "--target",
        "target-that-does-not-exist",
        "--scan-path",
        scan_root.to_string_lossy().as_ref(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Starting clean for 1 target(s)...",
    ))
    .stdout(predicate::str::contains("Cleaning finished. Targets: 1"));

    let remaining = fs::read_dir(&scan_root).expect("read root").count();
    assert_eq!(remaining, 0);
}

#[test]
fn headless_clean_without_yes_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let scan_root = temp.path().join("clean-root");
    fs::create_dir_all(&scan_root).expect("create root");
    fs::write(scan_root.join("a.txt"), b"abc").expect("write file");

    let mut cmd = Command::cargo_bin("acari").expect("binary exists");
    cmd.args([
        "--headless",
        "--clean",
        "--target",
        "target-that-does-not-exist",
        "--scan-path",
        scan_root.to_string_lossy().as_ref(),
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains(
        "Refusing destructive clean without --yes",
    ));
}

#[test]
fn headless_dry_run_does_not_remove_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let scan_root = temp.path().join("dry-run-root");
    fs::create_dir_all(&scan_root).expect("create root");
    fs::write(scan_root.join("a.txt"), b"abc").expect("write file");
    fs::write(scan_root.join("b.txt"), b"12345").expect("write file");

    let mut cmd = Command::cargo_bin("acari").expect("binary exists");
    cmd.args([
        "--headless",
        "--clean",
        "--dry-run",
        "--target",
        "target-that-does-not-exist",
        "--scan-path",
        scan_root.to_string_lossy().as_ref(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "Starting dry-run clean for 1 target(s)...",
    ))
    .stdout(predicate::str::contains(
        "Dry-run cleaning finished. Targets: 1",
    ));

    let remaining = fs::read_dir(&scan_root).expect("read root").count();
    assert_eq!(remaining, 2);
}
