use std::{path::Path, process::Command};

#[test]
fn development_asset_link_reaches_workspace_assets() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::canonicalize(package.join("assets"))
            .expect("development assets link must resolve"),
        std::fs::canonicalize(package.join("../../assets")).unwrap()
    );
}

#[test]
fn version_build_tracks_git_from_nested_app_and_preserves_ci_override() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let probe = temp
        .path()
        .join(format!("version-probe{}", std::env::consts::EXE_SUFFIX));
    let compile = Command::new("rustc")
        .arg("--edition=2024")
        .arg(package.join("build.rs"))
        .arg("-o")
        .arg(&probe)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(probe)
        .current_dir(package)
        .env("LIFTHRASIR_VERSION", "  test-release  ")
        .env("CARGO_PKG_VERSION", "0.0.1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let directives = String::from_utf8(output.stdout).unwrap();
    assert!(directives.contains("cargo:rustc-env=LIFTHRASIR_VERSION=test-release\n"));
    let head = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-path", "HEAD"])
        .current_dir(package)
        .output()
        .unwrap();
    assert!(head.status.success());
    let head = String::from_utf8(head.stdout).unwrap();
    assert!(
        directives
            .lines()
            .any(|line| line == format!("cargo:rerun-if-changed={}", head.trim())),
        "nested build did not track actual Git HEAD: {directives}"
    );
    for path in directives
        .lines()
        .filter_map(|line| line.strip_prefix("cargo:rerun-if-changed="))
    {
        assert!(
            Path::new(path).is_absolute() && Path::new(path).exists(),
            "unresolved metadata path {path}"
        );
    }
}
