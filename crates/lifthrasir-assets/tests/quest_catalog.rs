use bevy::asset::LoadState;
use bevy::prelude::*;
use lifthrasir_assets::catalogs::{QuestAsset, QuestDb};
use lifthrasir_assets::source::{
    AssetConfig, AssetsSection, register_ro_asset_source, setup_composite_source_from_config,
};
use lifthrasir_assets::{ContentAssetsPlugin, ContentCatalogPlugin};
use std::sync::{Arc, RwLock};

fn catalog_app(data_dir: &std::path::Path) -> App {
    let config = AssetConfig {
        assets: AssetsSection {
            data_folder: data_dir.to_string_lossy().into_owned(),
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
    app
}

fn update_until(app: &mut App, done: impl Fn(&World) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !done(app.world()) {
        assert!(
            std::time::Instant::now() < deadline,
            "quest catalog did not finish loading"
        );
        app.update();
        std::thread::yield_now();
    }
}

#[test]
fn quest_catalog_loads_text_from_ron() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("ron")).unwrap();
    std::fs::write(
        dir.path().join("ron/quest_data.ron"),
        r#"(quests:{5:(title:"Ransom",description:["Turtle","Shell"],summary:"Carapace")})"#,
    )
    .unwrap();
    let mut app = catalog_app(dir.path());

    update_until(&mut app, |world| world.contains_resource::<QuestDb>());

    let db = app.world().resource::<QuestDb>();
    let text = db.text(5).expect("quest 5");
    assert_eq!(text.title, "Ransom");
    assert_eq!(text.description, ["Turtle", "Shell"]);
    assert_eq!(text.summary, "Carapace");
}

#[test]
fn missing_quest_catalog_leaves_no_quest_db() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("ron")).unwrap();
    let mut app = catalog_app(dir.path());
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<QuestAsset>("ro://ron/quest_data.ron");

    update_until(&mut app, |world| {
        matches!(
            world.resource::<AssetServer>().load_state(&handle),
            LoadState::Failed(_)
        )
    });
    app.update();
    app.update();

    assert!(!app.world().contains_resource::<QuestDb>());
}
