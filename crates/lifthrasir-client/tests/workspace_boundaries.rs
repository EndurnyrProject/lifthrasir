//! Migration guards traverse Cargo package identities, not display-name substrings.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    #[serde(default)]
    manifest_path: std::path::PathBuf,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize)]
struct DependencyKind {
    kind: Option<String>,
}

impl Metadata {
    fn package(&self, name: &str) -> &Package {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .unwrap_or_else(|| panic!("expected package {name} is missing"))
    }

    fn forbidden_dependencies(&self, package: &str, forbidden: &[&str]) -> Vec<String> {
        let root = self.package(package);
        let packages: HashMap<_, _> = self.packages.iter().map(|p| (&p.id, &p.name)).collect();
        let nodes: HashMap<_, _> = self.resolve.nodes.iter().map(|n| (&n.id, n)).collect();
        let mut pending = vec![&root.id];
        let mut visited = HashSet::new();
        let mut offenders = Vec::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let name = packages.get(id).expect("resolved package has no identity");
            if id != &root.id
                && forbidden.iter().any(|forbidden| {
                    name.as_str() == *forbidden
                        || (*forbidden == "bevy" && name.starts_with("bevy_"))
                })
            {
                offenders.push((*name).clone());
            }
            let node = nodes
                .get(id)
                .expect("resolved package has no dependency node");
            pending.extend(node.deps.iter().filter_map(|dependency| {
                dependency
                    .dep_kinds
                    .iter()
                    .any(|kind| kind.kind.as_deref() != Some("dev"))
                    .then_some(&dependency.pkg)
            }));
        }
        offenders.sort();
        offenders.dedup();
        offenders
    }
}

fn metadata() -> &'static Metadata {
    static METADATA: OnceLock<Metadata> = OnceLock::new();
    METADATA.get_or_init(|| {
        let output = Command::new(env!("CARGO"))
            .args([
                "metadata",
                "--locked",
                "--format-version",
                "1",
                "--filter-platform",
                "host-tuple",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cannot run cargo metadata to verify workspace boundaries");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("invalid cargo metadata")
    })
}

fn assert_package_present(package: &str) {
    metadata().package(package);
}

fn assert_no_dependency(package: &str, forbidden: &[&str]) {
    let offenders = metadata().forbidden_dependencies(package, forbidden);
    assert!(
        offenders.is_empty(),
        "{package} has forbidden normal/build dependencies: {offenders:?}"
    );
}

#[test]
fn current_workspace_boundaries() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for package in [
        "lifthrasir-data",
        "lifthrasir-pak",
        "lifthrasir-assets",
        "lifthrasir-runtime",
        "lifthrasir-tooling",
        "lifthrasir-ui",
        "lifthrasir-client",
        "lifthrasir-client-ui",
        "net-contract",
        "net-aesir",
        "ro-formats",
        "lifthrasir",
        "ro-to-lifthrasir-cli",
        "grf-utils",
    ] {
        assert_package_present(package);
        let directory = if matches!(package, "lifthrasir" | "grf-utils" | "ro-to-lifthrasir-cli") {
            "apps"
        } else {
            "crates"
        };
        assert_eq!(
            metadata()
                .package(package)
                .manifest_path
                .canonicalize()
                .unwrap(),
            workspace
                .join(directory)
                .join(package)
                .join("Cargo.toml")
                .canonicalize()
                .unwrap(),
            "unexpected package location: {package}"
        );
        assert!(
            !workspace.join(package).join("Cargo.toml").exists(),
            "old root manifest remains: {package}"
        );
    }
    for old in ["game-engine", "lifthrasir-ui-primitives"] {
        assert!(
            !workspace.join(old).join("Cargo.toml").exists(),
            "old migration manifest remains: {old}"
        );
    }
    for package in ["lifthrasir-data", "lifthrasir-pak", "lifthrasir-tooling"] {
        assert_no_dependency(
            package,
            &[
                "bevy",
                "game-engine",
                "lifthrasir-client",
                "lifthrasir-client-ui",
                "lifthrasir",
            ],
        );
    }
    assert_no_dependency(
        "lifthrasir-pak",
        &[
            "ro-formats",
            "lifthrasir-tooling",
            "ro-to-lifthrasir-cli",
            "grf-utils",
        ],
    );
    for package in ["lifthrasir-assets", "lifthrasir-runtime"] {
        assert_no_dependency(
            package,
            &[
                "game-engine",
                "lifthrasir-client",
                "lifthrasir-client-ui",
                "lifthrasir",
                "lifthrasir-ui",
                "net-contract",
                "net-aesir",
                "lifthrasir-tooling",
                "ro-to-lifthrasir-cli",
                "grf-utils",
            ],
        );
    }
    assert_no_dependency("lifthrasir-assets", &["lifthrasir-runtime"]);
    assert_no_dependency(
        "lifthrasir-ui",
        &[
            "lifthrasir-assets",
            "lifthrasir-runtime",
            "lifthrasir-tooling",
            "game-engine",
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir",
            "net-contract",
            "net-aesir",
        ],
    );
    for package in ["lifthrasir-client", "lifthrasir-client-ui"] {
        assert_no_dependency(
            package,
            &[
                "net-aesir",
                "bevy_quinnet",
                "prost",
                "prost-build",
                "protobuf",
            ],
        );
    }
    assert_no_dependency(
        "lifthrasir-client",
        &["lifthrasir-ui", "lifthrasir-client-ui"],
    );
}

fn sdk_forwarder_pattern() -> regex::Regex {
    regex::Regex::new(r"\bpub\s+use\s+[^;]*\blifthrasir_(?:assets|runtime|ui)\b[^;]*;").unwrap()
}

#[test]
fn forwarding_guard_catches_whole_crates_aliases_groups_and_inline_declarations() {
    let pattern = sdk_forwarder_pattern();
    for source in [
        "pub use lifthrasir_runtime;",
        "pub use lifthrasir_runtime as runtime;",
        "pub use ::lifthrasir_ui::{theme, widgets};",
        "mod x { pub use lifthrasir_assets::Gender; }",
        "#[doc(hidden)] pub use lifthrasir_runtime::*;",
        "pub use {crate::Owned, lifthrasir_ui::theme};",
    ] {
        assert!(pattern.is_match(source), "missed {source}");
    }
    for source in [
        "use lifthrasir_runtime::ActorRenderPlugin;",
        "pub use crate::Owned;",
        "pub use lifthrasir_runtime_tools::Owned;",
    ] {
        assert!(!pattern.is_match(source), "false positive {source}");
    }
}

#[test]
fn both_client_packages_have_no_public_sdk_forwarding_facade() {
    fn scan(path: &std::path::Path, pattern: &regex::Regex) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, pattern);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            assert!(
                !pattern.is_match(&source),
                "{} publicly forwards an SDK crate",
                path.display()
            );
        }
    }
    let pattern = sdk_forwarder_pattern();
    for package in ["lifthrasir-client", "lifthrasir-client-ui"] {
        scan(
            &metadata()
                .package(package)
                .manifest_path
                .parent()
                .unwrap()
                .join("src"),
            &pattern,
        );
    }
}

#[test]
fn isolated_runtime_consumer_does_not_activate_gr2_import() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = metadata()
        .package("lifthrasir-runtime")
        .manifest_path
        .parent()
        .unwrap();
    let path = serde_json::to_string(&runtime.to_string_lossy()).unwrap();
    std::fs::create_dir(temp.path().join("src")).unwrap();
    std::fs::write(temp.path().join("src/lib.rs"), "").unwrap();
    std::fs::write(temp.path().join("Cargo.toml"), format!(
        "[package]\nname = \"runtime-isolation-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\nresolver = \"2\"\n[dependencies]\nlifthrasir-runtime = {{ path = {path} }}\n"
    )).unwrap();
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| path.join("Cargo.lock").is_file())
        .unwrap();
    std::fs::copy(workspace.join("Cargo.lock"), temp.path().join("Cargo.lock")).unwrap();
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--filter-platform",
            "host-tuple",
        ])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated consumer metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        graph["workspace_members"].as_array().unwrap().len(),
        1,
        "feature probe must be a separate one-package workspace"
    );
    assert!(
        graph["packages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|package| !matches!(
                package["name"].as_str(),
                Some("lifthrasir-client" | "lifthrasir-tooling")
            )),
        "feature probe unexpectedly includes client/offline tooling"
    );
    let format_id = &graph["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "ro-formats")
        .unwrap()["id"];
    let node = graph["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| &node["id"] == format_id)
        .unwrap();
    assert!(
        !node["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|feature| feature == "gr2"),
        "runtime-only consumer activated RO GR2 import: {node}"
    );
}

#[test]
fn editor_is_an_app_without_client_or_network_dependencies() {
    let editor = metadata().package("lifthrasir-editor");
    assert!(
        editor
            .manifest_path
            .ends_with("apps/lifthrasir-editor/Cargo.toml"),
        "lifthrasir-editor lives at {}",
        editor.manifest_path.display()
    );
    assert_no_dependency(
        "lifthrasir-editor",
        &[
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir",
            "lifthrasir-ui",
            "net-contract",
            "net-aesir",
            "bevy_quinnet",
        ],
    );
}

#[test]
fn tooling_is_bevy_free_and_does_not_own_proto_generation() {
    assert_package_present("lifthrasir-tooling");
    assert_no_dependency(
        "lifthrasir-tooling",
        &[
            "bevy",
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir-assets",
            "lifthrasir-runtime",
            "lifthrasir-ui",
            "net-contract",
            "net-aesir",
            "protox",
            "prost-build",
            "clap",
            "indicatif",
        ],
    );
}

#[test]
fn client_package_replaces_engine_without_a_compatibility_package() {
    assert_package_present("lifthrasir-client");
    assert!(
        metadata()
            .packages
            .iter()
            .all(|package| package.name != "game-engine"),
        "legacy game-engine compatibility package must not exist"
    );
}

#[test]
fn shared_ui_is_unique_and_has_no_client_or_content_policy_dependencies() {
    assert_package_present("lifthrasir-client-ui");
    assert_eq!(
        metadata()
            .packages
            .iter()
            .filter(|package| package.name == "lifthrasir-ui")
            .count(),
        1
    );
    assert_no_dependency(
        "lifthrasir-ui",
        &[
            "game-engine",
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir-runtime",
            "lifthrasir-assets",
            "net-contract",
            "net-aesir",
            "lifthrasir",
        ],
    );
}

#[test]
fn ignores_dev_only_edges_but_keeps_mixed_build_edges() {
    let graph: Metadata = serde_json::from_value(serde_json::json!({
        "packages": [
            {"id": "root@1", "name": "root"},
            {"id": "dev@1", "name": "dev-helper"},
            {"id": "forbidden@1", "name": "forbidden"}
        ],
        "resolve": {"nodes": [
            {"id": "root@1", "deps": [{"pkg": "dev@1", "dep_kinds": [{"kind": "dev"}]}]},
            {"id": "dev@1", "deps": [{"pkg": "forbidden@1", "dep_kinds": [{"kind": null}]}]},
            {"id": "forbidden@1", "deps": []}
        ]}
    }))
    .unwrap();
    assert!(
        graph
            .forbidden_dependencies("root", &["forbidden"])
            .is_empty()
    );

    let mut graph = graph;
    graph.resolve.nodes[0].deps[0]
        .dep_kinds
        .push(DependencyKind {
            kind: Some("build".into()),
        });
    assert_eq!(
        graph.forbidden_dependencies("root", &["forbidden"]),
        ["forbidden"]
    );
}

#[test]
fn bevy_free_boundary_rejects_bevy_subcrates() {
    let graph: Metadata = serde_json::from_value(serde_json::json!({
        "packages": [
            {"id": "root@1", "name": "root"},
            {"id": "math@1", "name": "bevy_math"}
        ],
        "resolve": {"nodes": [
            {"id": "root@1", "deps": [{"pkg": "math@1", "dep_kinds": [{"kind": null}]}]},
            {"id": "math@1", "deps": []}
        ]}
    }))
    .unwrap();
    assert_eq!(
        graph.forbidden_dependencies("root", &["bevy"]),
        ["bevy_math"]
    );
}

#[test]
#[should_panic(expected = "expected package missing is missing")]
fn missing_expected_package_is_not_silently_skipped() {
    let graph = Metadata {
        packages: Vec::new(),
        resolve: Resolve { nodes: Vec::new() },
    };
    graph.forbidden_dependencies("missing", &["bevy"]);
}

#[test]
fn detects_direct_and_transitive_normal_and_build_dependencies_by_identity() {
    let graph: Metadata = serde_json::from_value(serde_json::json!({
        "packages": [
            {"id": "root@1", "name": "root"},
            {"id": "alias@1", "name": "bridge"},
            {"id": "forbidden@1", "name": "forbidden"}
        ],
        "resolve": {"nodes": [
            {"id": "root@1", "deps": [{"pkg": "alias@1", "dep_kinds": [{"kind": null}]}]},
            {"id": "alias@1", "deps": [{"pkg": "forbidden@1", "dep_kinds": [{"kind": "build"}]}]},
            {"id": "forbidden@1", "deps": []}
        ]}
    }))
    .unwrap();
    assert_eq!(
        graph.forbidden_dependencies("root", &["bridge"]),
        ["bridge"]
    );
    assert_eq!(
        graph.forbidden_dependencies("root", &["forbidden"]),
        ["forbidden"]
    );
}
