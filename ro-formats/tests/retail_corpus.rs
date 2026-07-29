//! Opt-in sweep of the real GRF archives.
//!
//! `format_layout.rs` pins the layout the parsers agree to read, but it is
//! written from the same understanding the parsers hold - a mistaken assumption
//! would be encoded in both and agree with itself. This sweep is the check that
//! catches that: it reads every model and map in the retail archives and
//! requires each one to parse and to consume exactly the bytes the format says
//! it should.
//!
//! It needs `assets/*.grf`, which are deliberately not in the repository, so it
//! is ignored by default. Run it after any change to a format reader:
//!
//! ```sh
//! cargo test -p ro-formats --test retail_corpus -- --ignored --nocapture
//! ```

use ro_formats::grf::GrfFile;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn archives() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("assets");

    ["data.grf", "en.grf"]
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect()
}

/// Parse every entry with the given extension, reporting a per-extension tally.
fn sweep(extension: &str, parse: impl Fn(&[u8]) -> Result<(), String>) {
    let archives = archives();
    assert!(
        !archives.is_empty(),
        "no GRF archives found under assets/; this test needs the retail data"
    );

    let mut parsed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in archives {
        let archive = GrfFile::from_path(path.clone()).expect("open archive");
        let mut reader = archive.entry_reader().expect("entry reader");

        let indexes: Vec<usize> = archive
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .filename
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{extension}"))
            })
            .map(|(index, _)| index)
            .collect();

        for index in indexes {
            let Some(bytes) = reader.get_entry(index) else {
                continue;
            };
            match parse(&bytes) {
                Ok(()) => parsed += 1,
                Err(error) => failures.push(format!(
                    "{} :: {}: {error}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    archive.entries[index].filename,
                )),
            }
        }
    }

    println!("{extension}: {parsed} parsed, {} failed", failures.len());
    assert!(
        failures.is_empty(),
        "{} {extension} file(s) failed; first 10:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
#[ignore = "needs the retail GRF archives under assets/"]
fn every_retail_rsm_parses_exactly() {
    sweep("rsm", |bytes| {
        ro_formats::Rsm::from_bytes(bytes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
}

#[test]
#[ignore = "needs the retail GRF archives under assets/"]
fn every_retail_rsw_parses_exactly() {
    sweep("rsw", |bytes| {
        ro_formats::RoWorld::from_bytes(bytes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
}

#[test]
#[ignore = "needs the retail GRF archives under assets/"]
fn every_retail_gnd_parses_exactly() {
    sweep("gnd", |bytes| {
        ro_formats::RoGround::from_bytes(bytes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
}

/// Print how many files sit behind each version gate.
///
/// A gate with no corpus behind it is a gate nothing has ever verified, so this
/// is worth glancing at whenever a reader changes.
#[test]
#[ignore = "needs the retail GRF archives under assets/"]
fn report_corpus_version_coverage() {
    let mut versions: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();

    for path in archives() {
        let archive = GrfFile::from_path(path).expect("open archive");
        let mut reader = archive.entry_reader().expect("entry reader");

        for index in 0..archive.entries.len() {
            let name = archive.entries[index].filename.to_ascii_lowercase();
            let extension = ["rsm", "rsw", "gnd"]
                .into_iter()
                .find(|e| name.ends_with(&format!(".{e}")));
            let Some(extension) = extension else { continue };
            let Some(bytes) = reader.get_entry(index) else {
                continue;
            };
            if bytes.len() < 6 {
                continue;
            }
            *versions
                .entry(extension)
                .or_default()
                .entry(format!("{}.{}", bytes[4], bytes[5]))
                .or_default() += 1;
        }
    }

    for (extension, counts) in versions {
        println!("{extension}: {counts:?}");
    }
}
