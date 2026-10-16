use bevy::prelude::*;
use lifthrasir_assets::catalogs::*;
use lifthrasir_assets::source::{
    AssetConfig, AssetsSection, register_ro_asset_source, setup_composite_source_from_config,
};
use lifthrasir_assets::{ContentAssetsPlugin, ContentCatalogPlugin};
use std::sync::{Arc, RwLock};

#[test]
fn content_catalogs_are_opt_in() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .add_plugins(ContentAssetsPlugin);
    app.update();
    assert!(!app.world().contains_resource::<Assets<JobDataAsset>>());
    assert!(!app.world().contains_resource::<JobSpriteRegistry>());
}

#[test]
fn synthetic_catalogs_load_without_client_state_or_networking() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("ron")).unwrap();
    for (name, contents) in [
        (
            "job_data",
            r#"(npc_sprites:{46:"npc"},display_names:{0:"Novice"})"#,
        ),
        ("item_data", "(items:{})"),
        ("accessory_data", r#"(names:{1:"_고글"})"#),
        (
            "weapon_data",
            r#"(names:{2:"_검"},hit_sounds:{},bow_types:[])"#,
        ),
        ("skill_data", "(skills:{})"),
        (
            "status_icons",
            r#"(icons:{10:(image:"BLESSING.TGA",name:"Blessing")})"#,
        ),
        ("effects", "(skills:{},special:{})"),
    ] {
        std::fs::write(dir.path().join(format!("ron/{name}.ron")), contents).unwrap();
    }
    let config = AssetConfig {
        assets: AssetsSection {
            data_folder: dir.path().to_string_lossy().into_owned(),
            archive: vec![],
        },
    };
    let source = setup_composite_source_from_config(&config).unwrap();
    let mut app = App::new();
    register_ro_asset_source(&mut app, Arc::new(RwLock::new(source)));
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, ContentCatalogPlugin));
    app.finish();
    app.cleanup();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.update();
        let world = app.world();
        if world.contains_resource::<JobSpriteRegistry>()
            && world.contains_resource::<ItemDb>()
            && world.contains_resource::<AccessoryDb>()
            && world.contains_resource::<WeaponDb>()
            && world.contains_resource::<SkillCatalog>()
            && world.contains_resource::<StatusIconCatalog>()
            && world.contains_resource::<EffectCatalog>()
            && world.contains_resource::<ShaderFxCatalog>()
        {
            assert_eq!(
                world.resource::<JobSpriteRegistry>().get_display_name(0),
                Some("Novice")
            );
            assert!(world.resource::<ItemDb>().is_empty());
            assert_eq!(world.resource::<AccessoryDb>().accname(1), Some("_고글"));
            assert_eq!(world.resource::<WeaponDb>().suffix(2), Some("_검"));
            assert!(world.resource::<SkillCatalog>().get(1).is_none());
            assert_eq!(
                world.resource::<StatusIconCatalog>().name(10),
                Some("Blessing")
            );
            assert!(world.resource::<EffectCatalog>().skill(1).is_none());
            assert!(world.resource::<ShaderFxCatalog>().get("missing").is_none());
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "content catalogs did not finish loading"
        );
        std::thread::yield_now();
    }
}
