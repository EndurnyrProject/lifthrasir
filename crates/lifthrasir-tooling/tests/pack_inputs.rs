use lifthrasir_tooling::{PackInputs, collect_entries};
use std::fs;

#[test]
fn loose_and_content_mounts_preserve_precedence_and_post_merge_exclusions() {
    let temp = tempfile::tempdir().unwrap();
    let loose = temp.path().join("data");
    let first = temp.path().join("content-first");
    let last = temp.path().join("content-last");
    for folder in [&loose, &first, &last] {
        fs::create_dir_all(folder.join("data")).unwrap();
    }
    fs::write(loose.join("same.txt"), b"loose").unwrap();
    fs::write(first.join("data/same.txt"), b"content").unwrap();
    fs::write(last.join("data/same.txt"), b"last").unwrap();
    fs::write(first.join("data/second.txt"), b"first").unwrap();
    fs::write(last.join("data/second.txt"), b"last").unwrap();
    fs::write(first.join("map.GND"), b"source").unwrap();
    let PackInputs {
        entries,
        skipped,
        excluded,
        ..
    } = collect_entries(&[], Some(&loose), &[first, last]);
    let entries: std::collections::HashMap<_, _> = entries.into_iter().collect();
    assert_eq!(entries["data/same.txt"], b"loose");
    assert_eq!(entries["data/second.txt"], b"first");
    assert_eq!(skipped, 0);
    assert_eq!(excluded, 1);
}

#[test]
fn missing_grfs_are_counted_and_missing_folders_keep_their_tolerant_behavior() {
    let temp = tempfile::tempdir().unwrap();
    let PackInputs {
        entries,
        skipped,
        excluded,
        ..
    } = collect_entries(
        &[temp.path().join("missing.grf")],
        Some(&temp.path().join("missing-data")),
        &[],
    );
    assert!(entries.is_empty());
    assert_eq!(skipped, 1);
    assert_eq!(excluded, 0);
}
