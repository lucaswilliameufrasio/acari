use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn cmd() -> Command {
    let mut c = Command::cargo_bin("acari").expect("binary exists");
    c.env("LANG", "C");
    c
}

#[test]
fn list_prints_known_targets() {
    cmd()
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Cargo Registry"))
        .stdout(predicate::str::contains("NPM Cache"));
}

#[test]
fn headless_with_unknown_target_shows_message() {
    cmd()
        .args(["--headless", "--target", "target-that-does-not-exist"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No scan targets matched your filters.",
        ));
}

#[test]
fn headless_bin_with_unknown_target_shows_message() {
    let mut c = Command::cargo_bin("headless_cleaner").expect("binary exists");
    c.env("LANG", "C");
    c.args(["--target", "target-that-does-not-exist"])
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

    cmd()
        .args([
            "--headless",
            "--target",
            "target-that-does-not-exist",
            "--scan-path",
            scan_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[done] Custom Path 1"))
        .stdout(predicate::str::contains("8 B"))
        .stdout(predicate::str::contains(
            "Scan finished. Total reclaimable bytes",
        ));
}

#[test]
fn headless_scan_path_with_clean_empties_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let scan_root = temp.path().join("clean-root");
    fs::create_dir_all(&scan_root).expect("create root");
    fs::write(scan_root.join("a.txt"), b"abc").expect("write file");
    fs::write(scan_root.join("b.txt"), b"12345").expect("write file");

    cmd()
        .args([
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

    cmd()
        .args([
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

    cmd()
        .args([
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

#[test]
fn target_add_persists_to_config() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(temp.path().join("acari")).expect("create acari dir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args([
        "target",
        "add",
        "My Drive",
        "/mnt/drive",
        "-d",
        "External disk",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Target 'My Drive' added."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains("My Drive"));
    assert!(content.contains("/mnt/drive"));
    assert!(content.contains("External disk"));
}

#[test]
fn target_add_duplicate_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "add", "My Drive", "/mnt/drive"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "add", "my drive", "/mnt/other"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn target_remove_removes_from_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "add", "My Drive", "/mnt/drive"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "remove", "my drive"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Target 'my drive' removed."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(!content.contains("My Drive"));
}

#[test]
fn target_remove_missing_shows_not_found() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "remove", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn target_list_shows_custom_targets() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "add", "My Drive", "/mnt/drive"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My Drive"))
        .stdout(predicate::str::contains("/mnt/drive"))
        .stdout(predicate::str::contains("Config last modified"));
}

#[test]
fn target_list_empty_shows_hint() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("acari target add"));
}

// --- project CLI tests ---

#[test]
fn project_add_pattern_persists_to_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", ".terraform"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pattern '.terraform' added."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains(".terraform"));
}

#[test]
fn project_add_pattern_builtin_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", "node_modules"])
        .assert()
        .success()
        .stdout(predicate::str::contains("built-in"));
}

#[test]
fn project_remove_pattern_removes_from_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", ".terraform"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "remove-pattern", ".terraform"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pattern '.terraform' removed."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(!content.contains(".terraform"));
}

#[test]
fn project_list_patterns_shows_builtin_and_custom() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", ".terraform"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "list-patterns"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Built-in patterns:"))
        .stdout(predicate::str::contains("node_modules"))
        .stdout(predicate::str::contains(".terraform"));
}

#[test]
fn project_add_root_persists_to_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", "~/projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Root '~/projects' added."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains("~/projects"));
}

#[test]
fn project_remove_root_removes_from_config() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", "~/projects"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "remove-root", "~/projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Root '~/projects' removed."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(!content.contains("~/projects"));
}

#[test]
fn project_list_roots_shows_roots() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", "~/projects"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "list-roots"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project roots:"))
        .stdout(predicate::str::contains("~/projects"));
}

#[test]
fn project_scan_headless_finds_junk() {
    let temp = tempfile::tempdir().expect("tempdir");
    let junk = temp.path().join("node_modules");
    fs::create_dir_all(junk.join("dep")).expect("create node_modules");
    fs::write(junk.join("dep").join("lib.js"), b"x").expect("write dep");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args([
        "project",
        "scan",
        temp.path().to_string_lossy().as_ref(),
        "--headless",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("node_modules"))
    .stdout(predicate::str::contains("B"));
}

#[test]
fn project_clear_patterns_removes_all() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", ".terraform"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-pattern", ".serverless"])
        .assert()
        .success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "clear-patterns"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2 custom pattern"));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(!content.contains(".terraform"), ".terraform should be gone");
    assert!(
        !content.contains(".serverless"),
        ".serverless should be gone"
    );
}
