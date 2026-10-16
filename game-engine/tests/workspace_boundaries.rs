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
    for package in [
        "lifthrasir-client",
        "lifthrasir-data",
        "lifthrasir-pak",
        "lifthrasir-assets",
        "lifthrasir-runtime",
        "lifthrasir",
        "net-contract",
        "net-aesir",
    ] {
        assert_package_present(package);
    }
    assert_no_dependency("lifthrasir-client", &["bevy_quinnet", "prost", "net-aesir"]);
    assert_no_dependency(
        "lifthrasir-runtime",
        &[
            "game-engine",
            "lifthrasir",
            "lifthrasir-client",
            "net-contract",
            "net-aesir",
            "lifthrasir-ui",
            "lifthrasir-client-ui",
            "lifthrasir-tooling",
            "ro-to-lifthrasir-cli",
            "grf-utils",
        ],
    );
    assert_no_dependency(
        "lifthrasir-assets",
        &[
            "game-engine",
            "lifthrasir",
            "lifthrasir-client",
            "net-contract",
            "net-aesir",
            "lifthrasir-runtime",
            "lifthrasir-ui",
            "lifthrasir-client-ui",
            "lifthrasir-tooling",
        ],
    );
    assert_no_dependency(
        "lifthrasir-pak",
        &[
            "bevy",
            "ro-formats",
            "game-engine",
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir-ui",
            "lifthrasir",
            "net-contract",
            "net-aesir",
            "ro-to-lifthrasir-cli",
            "grf-utils",
            "lifthrasir-tooling",
        ],
    );
    assert_no_dependency(
        "lifthrasir-data",
        &[
            "bevy",
            "game-engine",
            "lifthrasir-client",
            "lifthrasir-client-ui",
            "lifthrasir-ui",
            "lifthrasir",
            "net-contract",
            "net-aesir",
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
            .all(|package| package.name != "game-engine")
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
