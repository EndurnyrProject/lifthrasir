use std::path::Path;

#[test]
fn client_source_has_no_migration_public_forwarders() {
    fn check(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for owner in ["lifthrasir_assets", "lifthrasir_runtime"] {
                    assert!(
                        !source.contains(&format!("pub use {owner}::")),
                        "migration forwarder remains in {}",
                        path.display()
                    );
                }
            }
        }
    }
    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}
