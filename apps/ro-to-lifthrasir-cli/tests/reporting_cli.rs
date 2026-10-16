use std::{fs, process::Command};

#[test]
fn corpus_host_renders_library_progress_on_stderr() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("convert.toml");
    fs::write(&config, "[assets]\ngrf = []\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ro-to-lifthrasir-cli"))
        .args(["model-corpus", "extract", "--loader"])
        .arg(&config)
        .arg("--out")
        .arg(temp.path().join("extracted"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("corpus: extracting 0 files from GRFs")
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("extracted 0 corpus files"));
}

#[test]
fn synthetic_batch_failure_renders_its_partial_report_and_exits_nonzero() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("convert.toml");
    fs::write(&config, "[assets]\ngrf = []\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ro-to-lifthrasir-cli"))
        .args(["convert-props", "--loader"])
        .arg(&config)
        .arg("--models-dir")
        .arg(temp.path().join("models"))
        .args(["--model", "missing.rsm"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("props: 0 converted, 1 failed, 0 skipped"),
        "{stdout}"
    );
    assert!(
        stderr.contains("missing.rsm") && stderr.contains("model not found in GRFs"),
        "{stderr}"
    );
    assert!(!temp.path().join("models/missing.glb").exists());
}
