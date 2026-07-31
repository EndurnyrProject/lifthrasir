use game_engine::infrastructure::effect::AuthoredEffect;
use lifthrasir_data::{EffectData, EffectDescriptor, Visual};
use ron::extensions::Extensions;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::Path;
use zip::ZipArchive;

const ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets");
const EFFECTS_RON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../assets/data/ron/effects.ron"
);

#[test]
#[ignore = "requires the non-versioned assets/lifthrasir.pak retail archive"]
fn effect_catalog_assets_exist() {
    let mut archive = open_pak();
    let data = load_catalog();
    let mut authored_sources = BTreeMap::new();
    let mut missing = Vec::new();

    let mut checked = 0;

    for (section, descriptors) in [
        ("skills", &data.skills),
        ("special", &data.special),
        ("efsts", &data.efsts),
    ] {
        checked += check_descriptors(
            section,
            descriptors,
            &mut archive,
            &mut authored_sources,
            &mut missing,
        );
    }

    checked += check_authored_textures(&mut archive, &authored_sources, &mut missing);

    assert!(
        missing.is_empty(),
        "effect catalog asset references missing:\n{}",
        missing.join("\n")
    );

    // Guard against a vacuous pass: an empty-but-valid effects.ron (every map is
    // `#[serde(default)]`) or an emptied assets/data/effects folder would check
    // nothing at all and still report success.
    assert!(
        checked > 200,
        "expected the catalog to reference hundreds of assets, only checked {checked} - \
         the catalog or the authored effect folder is probably not being read"
    );
}

fn open_pak() -> ZipArchive<File> {
    let path = Path::new(ASSETS_DIR).join("lifthrasir.pak");
    let file = File::open(&path)
        .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()));
    ZipArchive::new(file)
        .unwrap_or_else(|error| panic!("failed to read {} as zip64: {error}", path.display()))
}

fn load_catalog() -> EffectData {
    let contents = fs::read_to_string(EFFECTS_RON)
        .unwrap_or_else(|error| panic!("failed to read {EFFECTS_RON}: {error}"));
    ron::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {EFFECTS_RON}: {error}"))
}

/// Returns the number of asset references checked.
fn check_descriptors(
    section: &str,
    descriptors: &BTreeMap<u32, EffectDescriptor>,
    archive: &mut ZipArchive<File>,
    authored_sources: &mut BTreeMap<String, Vec<String>>,
    missing: &mut Vec<String>,
) -> usize {
    let mut checked = 0;

    for (id, descriptor) in descriptors {
        let source = format!("{section}[{id}]");
        for visual in &descriptor.visuals {
            match visual {
                Visual::Str(name) if name.ends_with(".strfx.ron") => {
                    checked += 1;
                    let path = format!("ro://effects/{name}");
                    authored_sources
                        .entry(name.clone())
                        .or_default()
                        .push(source.clone());
                    if !Path::new(ASSETS_DIR)
                        .join("data")
                        .join("effects")
                        .join(name)
                        .is_file()
                    {
                        missing.push(format!("{source} Str references missing {path}"));
                    }
                }
                Visual::Str(name) => {
                    checked += 1;
                    check_pak_entry(
                        archive,
                        missing,
                        &source,
                        "Str",
                        &format!("ro://data/texture/effect/{name}"),
                    );
                }
                Visual::Sprite(stem) => {
                    for extension in ["spr", "act"] {
                        checked += 1;
                        check_pak_entry(
                            archive,
                            missing,
                            &source,
                            "Sprite",
                            &format!("ro://data/sprite/{stem}.{extension}"),
                        );
                    }
                }
                Visual::Shader(_) | Visual::Bespoke(_) | Visual::Efst(_) => {}
            }
        }

        if let Some(sound) = &descriptor.sound {
            checked += 1;
            check_pak_entry(
                archive,
                missing,
                &source,
                "sound",
                &format!("ro://data/wav/{sound}"),
            );
        }
    }

    checked
}

/// Returns the number of layer textures checked.
fn check_authored_textures(
    archive: &mut ZipArchive<File>,
    authored_sources: &BTreeMap<String, Vec<String>>,
    missing: &mut Vec<String>,
) -> usize {
    let mut checked = 0;
    let effects_dir = Path::new(ASSETS_DIR).join("data/effects");
    let mut paths = fs::read_dir(&effects_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", effects_dir.display()))
        .map(|entry| {
            entry
                .expect("failed to read authored effect directory entry")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".strfx.ron"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("authored effect name must be UTF-8");
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let effect: AuthoredEffect = ron::Options::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to parse {name}: {error}"));
        let source = authored_sources
            .get(name)
            .map(|sources| sources.join(", "))
            .unwrap_or_else(|| format!("authored effect {name}"));

        for (layer, layer_data) in effect.layers.iter().enumerate() {
            for texture in &layer_data.textures {
                checked += 1;
                check_authored_texture(
                    archive,
                    missing,
                    &format!("{source} layer {layer}"),
                    texture,
                );
            }
        }
    }

    checked
}

fn check_authored_texture(
    archive: &mut ZipArchive<File>,
    missing: &mut Vec<String>,
    source: &str,
    path: &str,
) {
    if path.starts_with("ro://") {
        check_pak_entry(archive, missing, source, "texture", path);
        return;
    }

    if !Path::new(ASSETS_DIR).join(path).is_file() {
        missing.push(format!("{source} texture references missing {path}"));
    }
}

fn check_pak_entry(
    archive: &mut ZipArchive<File>,
    missing: &mut Vec<String>,
    source: &str,
    kind: &str,
    path: &str,
) {
    let entry = path
        .strip_prefix("ro://")
        .expect("pak entries are resolved from ro:// paths")
        .replace('\\', "/")
        .to_lowercase();
    if archive.by_name(&entry).is_err() {
        missing.push(format!("{source} {kind} references missing {path}"));
    }
}
