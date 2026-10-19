use lifthrasir_tooling::{BatchFailure, DiagnosticLevel, GrfVfs, collect_entries, convert_props};

#[test]
fn library_source_has_no_terminal_presentation_macros() {
    fn check(path: &std::path::Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path
                .file_name()
                .is_some_and(|name| name == "tests.rs" || name == "fixtures.rs")
            {
                continue;
            }
            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                let source = source.split("#[cfg(test)]\nmod tests {").next().unwrap();
                for macro_name in ["println!", "eprintln!", "print!", "dbg!"] {
                    assert!(
                        !source.contains(macro_name),
                        "{} contains {macro_name}",
                        path.display()
                    );
                }
            }
        }
    }
    check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}

#[test]
fn unreadable_pack_source_returns_a_warning_without_tightening_tolerance() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = collect_entries(&[temp.path().join("missing.grf")], None, &[]);
    assert_eq!(inputs.skipped, 1);
    assert!(inputs.entries.is_empty());
    assert_eq!(inputs.diagnostics.len(), 1);
    assert_eq!(inputs.diagnostics[0].level, DiagnosticLevel::Warning);
    assert!(inputs.diagnostics[0].message.contains("missing.grf"));
}

#[test]
fn failed_props_return_context_and_the_batch_summary_in_a_concrete_error() {
    let temp = tempfile::tempdir().unwrap();
    let source = GrfVfs::open(&[]).unwrap();
    let error = convert_props(&source, temp.path(), &["missing.rsm".into()], false).unwrap_err();
    let batch = error
        .downcast_ref::<BatchFailure>()
        .expect("partial report must remain available");
    assert_eq!(batch.failures.len(), 1);
    assert!(batch.failures[0].contains("missing.rsm"));
    assert!(batch.report.outputs.is_empty());
    assert!(batch.report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("0 converted, 1 failed, 0 skipped")
    }));
}
