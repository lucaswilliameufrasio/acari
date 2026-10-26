use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn cmd() -> Command {
    let mut c = Command::cargo_bin("acari").expect("binary exists");
    c.env("LANG", "C");
    c
}

#[cfg(target_os = "macos")]
fn safe_scan_tempdir() -> tempfile::TempDir {
    tempfile::tempdir_in("/tmp").expect("tempdir")
}

#[cfg(not(target_os = "macos"))]
fn safe_scan_tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
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
    let temp = safe_scan_tempdir();
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
            "Scan finished. Candidate estimate",
        ));
}

#[test]
fn headless_scan_path_with_clean_empties_directory() {
    let temp = safe_scan_tempdir();
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
    let temp = safe_scan_tempdir();
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
    let temp = safe_scan_tempdir();
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
fn list_marks_config_custom_targets_as_custom() {
    let temp = tempfile::tempdir().expect("tempdir");

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["target", "add", "My Custom Target", "/tmp/my-custom"])
        .assert()
        .success();

    // Config-defined custom targets must be labelled as custom in --list
    // (previously they were mislabelled as built-in because the check relied
    // on a description string that only scan-path targets set).
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("My Custom Target (custom)"));
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
    let temp = safe_scan_tempdir();
    let root = temp.path().canonicalize().unwrap().display().to_string();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", &root])
        .assert()
        .success()
        .stdout(predicate::str::contains("added."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains(&root));
}

#[test]
fn project_setup_is_opt_in_and_persists_validated_roots() {
    let temp = safe_scan_tempdir();
    let projects = temp.path().join("projects");
    fs::create_dir(&projects).unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args(["project", "setup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not added automatically"));
    assert!(!temp.path().join("acari/config.toml").exists());
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args(["project", "setup"])
        .arg(&projects)
        .arg(projects.join("."))
        .assert()
        .success();
    let config: acari::config::target_config::TargetConfig =
        toml::from_str(&fs::read_to_string(temp.path().join("acari/config.toml")).unwrap())
            .unwrap();
    assert_eq!(config.project_scan.roots.len(), 1);
    assert!(projects.is_dir());
    let before = fs::read(temp.path().join("acari/config.toml")).unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args(["project", "setup"])
        .arg(&projects)
        .arg(temp.path().join("missing"))
        .assert()
        .failure();
    assert_eq!(
        fs::read(temp.path().join("acari/config.toml")).unwrap(),
        before
    );
}

#[test]
fn project_scan_without_roots_explains_setup() {
    let temp = tempfile::tempdir().unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args(["project", "scan", "--headless"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not scanned"))
        .stderr(predicate::str::contains("acari project setup"));
}

#[cfg(target_os = "linux")]
#[test]
fn breakdown_distinguishes_unconfigured_roots_and_honors_custom_patterns() {
    let temp = tempfile::tempdir().unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .env("HOME", temp.path())
        .env("PATH", "/nonexistent")
        .args(["df", "--breakdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Projects: not scanned"))
        .stdout(predicate::str::contains("not guaranteed savings"));
    let project = temp.path().join("projects");
    fs::create_dir_all(project.join(".custom")).unwrap();
    fs::write(project.join(".custom/data"), vec![1; 8192]).unwrap();
    let mut cfg = acari::config::target_config::TargetConfig::default();
    cfg.project_scan.roots.push(project.display().to_string());
    cfg.project_scan.patterns.push(".custom".into());
    fs::create_dir_all(temp.path().join("acari")).unwrap();
    fs::write(
        temp.path().join("acari/config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .env("HOME", temp.path())
        .env("PATH", "/nonexistent")
        .args(["df", "--breakdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Candidates (projects, allocated blocks): 8.00 KB",
        ));
    cfg.scan.exclude_patterns.push(".custom".into());
    fs::write(
        temp.path().join("acari/config.toml"),
        toml::to_string(&cfg).unwrap(),
    )
    .unwrap();
    cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .env("HOME", temp.path())
        .env("PATH", "/nonexistent")
        .args(["df", "--breakdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Candidates (projects, allocated blocks): 0 B",
        ));
    assert!(project.join(".custom/data").is_file());
}

#[test]
fn json_exposes_overlap_and_estimate_metadata() {
    let temp = safe_scan_tempdir();
    let root = temp.path().join("cache");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::write(root.join("nested/data"), "abc").unwrap();
    let result = cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args([
            "--headless",
            "--json",
            "--target",
            "not-a-target",
            "--scan-path",
        ])
        .arg(&root)
        .arg("--scan-path")
        .arg(root.join("nested"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(json["total_bytes"], 3);
    assert_eq!(json["estimate_only"], true);
    assert_eq!(json["scan_errors"], 0);
    let targets = json["targets"].as_array().unwrap();
    assert_eq!(
        targets
            .iter()
            .map(|target| target["counted_bytes"].as_u64().unwrap())
            .sum::<u64>(),
        3
    );
    assert!(
        targets
            .iter()
            .all(|target| target["dangerous"].is_boolean())
    );
}

#[test]
fn json_distinguishes_failed_scan_from_measured_zero() {
    let temp = safe_scan_tempdir();
    let missing = temp.path().join("missing-scan-target");
    let output = cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args([
            "--headless",
            "--json",
            "--target",
            "not-a-target",
            "--scan-path",
        ])
        .arg(missing)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["scan_errors"], 1);
    assert_eq!(json["targets"][0]["scan_errors"], 1);
    assert_eq!(json["total_bytes"], 0);
}

#[test]
fn project_remove_root_removes_from_config() {
    let temp = safe_scan_tempdir();
    let root = temp.path().canonicalize().unwrap().display().to_string();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", &root]).assert().success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "remove-root", &root])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed."));

    let config_path = temp.path().join("acari").join("config.toml");
    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(!content.contains(&root));
}

#[test]
fn project_list_roots_shows_roots() {
    let temp = safe_scan_tempdir();
    let root = temp.path().canonicalize().unwrap().display().to_string();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "add-root", &root]).assert().success();

    let mut c = cmd();
    c.env("ACARI_CONFIG_HOME", temp.path());
    c.args(["project", "list-roots"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project roots:"))
        .stdout(predicate::str::contains(&root));
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
    c.args(["project", "add-pattern", ".mycustom"])
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
    assert!(!content.contains(".mycustom"), ".mycustom should be gone");
}

#[test]
fn headless_json_scan_path_emits_json() {
    let temp = safe_scan_tempdir();
    let root = temp.path().join("cache");
    fs::create_dir_all(&root).expect("create dir");
    fs::write(root.join("a.bin"), vec![0u8; 8]).expect("write file");

    cmd()
        .args([
            "--headless",
            "--json",
            "--target",
            "not-a-target",
            "--scan-path",
        ])
        .arg(root.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"targets\""))
        .stdout(predicate::str::contains("\"total_bytes\""));
}

#[test]
fn project_scan_json_emits_json() {
    let temp = tempfile::tempdir().expect("tempdir");
    let proj = temp.path().join("proj");
    fs::create_dir_all(proj.join("node_modules")).expect("create dir");
    fs::write(proj.join("node_modules").join("p.js"), b"x").expect("write file");

    let output = cmd()
        .args(["project", "scan", "--headless", "--json"])
        .arg(proj.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"targets\""))
        .stdout(predicate::str::contains("\"total_bytes\""))
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["discovery_errors"], 0);
    assert_eq!(json["total_bytes"], 1);
}

#[test]
fn project_json_missing_root_reports_discovery_error_not_clean_zero() {
    let temp = tempfile::tempdir().unwrap();
    let output = cmd()
        .env("ACARI_CONFIG_HOME", temp.path())
        .args(["project", "scan", "--headless", "--json"])
        .arg(temp.path().join("missing"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["discovery_errors"], 1);
    assert_eq!(json["scan_errors"], 1);
    assert_eq!(json["targets"].as_array().unwrap().len(), 0);
}

#[test]
fn history_shows_entries_and_clear_removes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log = temp.path().join("history.log");
    fs::write(
        &log,
        "2026-01-01 00:00:00 | Clean completed | targets=1 reclaimed=100 errors=0\n",
    )
    .expect("write history");

    cmd()
        .env("ACARI_DATA_HOME", temp.path())
        .arg("history")
        .assert()
        .success()
        .stdout(predicate::str::contains("Clean completed"));

    cmd()
        .env("ACARI_DATA_HOME", temp.path())
        .args(["history", "--clear"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cleared"));

    assert!(!log.exists(), "history log should be removed after --clear");
}

#[test]
fn df_prints_overview() {
    cmd()
        .arg("df")
        .assert()
        .success()
        .stdout(predicate::str::contains("Total:"))
        .stdout(predicate::str::contains("Free:"));
}
