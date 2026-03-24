use std::fs;
use std::time::Duration;

use acari::application::cleaner::{CleanMode, start_background_clean};
use acari::application::scanner::start_background_scan;
use acari::domain::{AppEvent, CleanTarget};

fn test_target(name: &'static str, path: &std::path::Path) -> CleanTarget {
    let leaked_path: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
    CleanTarget {
        name,
        path: leaked_path,
        description: "test",
    }
}

#[tokio::test]
async fn scanner_emits_target_and_finished_events() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let root = temp.path().join("scan-root");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("a.txt"), b"abc").expect("write file");

    let target = test_target("Scan Flow", &root);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_scan(tx, vec![target]);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_completed = false;
    let mut saw_finished = false;

    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            match event {
                AppEvent::TargetCompleted { target_name, .. } if target_name == "Scan Flow" => {
                    saw_completed = true;
                }
                AppEvent::ScanFinished => {
                    saw_finished = true;
                    break;
                }
                _ => {}
            }
        }
    }

    let _ = handle.await;

    assert!(saw_completed, "expected TargetCompleted event");
    assert!(saw_finished, "expected ScanFinished event");
}

#[tokio::test]
async fn cleaner_emits_cleaning_finished_and_removes_entries() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let root = temp.path().join("clean-root");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("cache.bin"), vec![0_u8; 64]).expect("write file");

    let target = test_target("Clean Flow", &root);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_clean(tx, vec![(target, 64)], CleanMode::Execute);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_target_cleaned = false;
    let mut saw_finished = false;

    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            match event {
                AppEvent::TargetCleaned { target_name, .. } if target_name == "Clean Flow" => {
                    saw_target_cleaned = true;
                }
                AppEvent::CleaningFinished { .. } => {
                    saw_finished = true;
                    break;
                }
                _ => {}
            }
        }
    }

    let _ = handle.await;

    let remaining = fs::read_dir(&root).expect("read root").count();
    assert_eq!(remaining, 0, "expected cache directory to be emptied");
    assert!(saw_target_cleaned, "expected TargetCleaned event");
    assert!(saw_finished, "expected CleaningFinished event");
}

#[tokio::test]
async fn cleaner_dry_run_keeps_entries() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let root = temp.path().join("dry-run-root");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("cache.bin"), vec![0_u8; 32]).expect("write file");

    let target = test_target("Dry Run Flow", &root);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    let handle = start_background_clean(tx, vec![(target, 32)], CleanMode::DryRun);

    let mut saw_finished = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
            && matches!(event, AppEvent::CleaningFinished { .. })
        {
            saw_finished = true;
            break;
        }
    }

    let _ = handle.await;

    let remaining = fs::read_dir(&root).expect("read root").count();
    assert_eq!(remaining, 1, "dry-run must keep files");
    assert!(saw_finished);
}
