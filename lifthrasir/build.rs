use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=LIFTHRASIR_VERSION");
    println!("cargo:rerun-if-changed=../.git/HEAD");

    let version = ci_version()
        .or_else(git_describe)
        .unwrap_or_else(manifest_version);

    println!("cargo:rustc-env=LIFTHRASIR_VERSION={version}");
}

fn ci_version() -> Option<String> {
    non_empty(std::env::var("LIFTHRASIR_VERSION").ok()?)
}

fn git_describe() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    non_empty(String::from_utf8(output.stdout).ok()?)
}

fn manifest_version() -> String {
    std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION for build scripts")
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}
