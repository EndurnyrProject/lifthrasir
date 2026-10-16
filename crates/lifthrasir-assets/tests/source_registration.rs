use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;
use lifthrasir_assets::source::{
    ArchiveConfig, AssetConfig, AssetSource, AssetsSection, register_ro_asset_source,
    setup_composite_source_from_config,
};
use std::sync::{Arc, RwLock};

#[derive(Asset, TypePath)]
struct Bytes(Vec<u8>);

#[derive(Default, TypePath)]
struct BytesLoader;

impl AssetLoader for BytesLoader {
    type Asset = Bytes;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> Result<Bytes, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Bytes(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["bin"]
    }
}

#[test]
fn relative_config_paths_keep_cwd_resolution() {
    const CHILD: &str = "LIFTHRASIR_WORKSPACE_REUSE_SOURCE_CWD_CHILD";
    if std::env::var_os(CHILD).is_some() {
        for (name, expected) in [
            ("direct.pak", b"root".as_slice()),
            ("fallback.pak", b"fallback"),
        ] {
            let config = AssetConfig {
                assets: AssetsSection {
                    data_folder: "./absent".into(),
                    archive: vec![ArchiveConfig {
                        path: name.into(),
                        priority: 0,
                    }],
                },
            };
            let source = setup_composite_source_from_config(&config).unwrap();
            assert_eq!(source.load("data/cwd.bin").unwrap(), expected);
        }
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    for (path, bytes) in [
        ("direct.pak", b"root".as_slice()),
        ("assets/direct.pak", b"wrong-root"),
        ("assets/fallback.pak", b"fallback"),
    ] {
        lifthrasir_pak::write_pak_parallel(
            &vec![("data/cwd.bin".into(), bytes.to_vec())],
            &dir.path().join(path),
            1,
            None,
            1,
            |_| {},
        )
        .unwrap();
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "relative_config_paths_keep_cwd_resolution"])
        .env(CHILD, "1")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn archive_priority_is_explicit_and_metadata_reads_stay_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    let mut archives = Vec::new();
    for (priority, bytes) in [(5, b"later".as_slice()), (0, b"first")] {
        let path = dir.path().join(format!("{priority}.pak"));
        lifthrasir_pak::write_pak_parallel(
            &vec![("data/priority.bin".into(), bytes.to_vec())],
            &path,
            1,
            None,
            1,
            |_| {},
        )
        .unwrap();
        archives.push(ArchiveConfig {
            path: path.to_string_lossy().into_owned(),
            priority,
        });
    }
    let config = AssetConfig {
        assets: AssetsSection {
            data_folder: dir.path().join("absent").to_string_lossy().into_owned(),
            archive: archives,
        },
    };
    let source = setup_composite_source_from_config(&config).unwrap();
    assert_eq!(source.load("data/priority.bin").unwrap(), b"first");
    let reader = lifthrasir_assets::source::hierarchical_reader::HierarchicalAssetReader::new(
        Arc::new(RwLock::new(source)),
    );
    let path = std::path::Path::new("data/priority.bin");
    use bevy::asset::io::{AssetReader, AssetReaderError};
    let result = futures_lite::future::block_on(reader.read_meta(path));
    assert!(matches!(result, Err(AssetReaderError::NotFound(missing)) if missing == path));
}

#[test]
fn source_registered_before_asset_plugin_reads_pak_and_loose_override() {
    let dir = tempfile::tempdir().unwrap();
    let pak_path = dir.path().join("fixture.pak");
    lifthrasir_pak::write_pak_parallel(
        &vec![
            ("data/override.bin".into(), b"pak".to_vec()),
            ("data/pak.bin".into(), b"pak-only".to_vec()),
        ],
        &pak_path,
        1,
        None,
        1,
        |_| {},
    )
    .unwrap();
    let loose = dir.path().join("loose");
    std::fs::create_dir_all(loose.join("data")).unwrap();
    std::fs::write(loose.join("data/override.bin"), b"loose").unwrap();
    let config = AssetConfig {
        assets: AssetsSection {
            data_folder: loose.to_string_lossy().into_owned(),
            archive: vec![ArchiveConfig {
                path: pak_path.to_string_lossy().into_owned(),
                priority: 0,
            }],
        },
    };
    let source = setup_composite_source_from_config(&config).unwrap();
    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Bytes>()
        .init_asset_loader::<BytesLoader>();
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>();
    let overridden: Handle<Bytes> = server.load("ro://data/override.bin");
    let archived: Handle<Bytes> = server.load("ro://data/pak.bin");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.update();
        let assets = app.world().resource::<Assets<Bytes>>();
        if let (Some(loose), Some(pak)) = (assets.get(&overridden), assets.get(&archived)) {
            assert_eq!(loose.0, b"loose");
            assert_eq!(pak.0, b"pak-only");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "ro:// assets did not load"
        );
        std::thread::yield_now();
    }
}
