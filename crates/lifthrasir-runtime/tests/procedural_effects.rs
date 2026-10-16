use bevy::prelude::*;
use lifthrasir_assets::{
    ContentAssetsPlugin,
    catalogs::shader_fx::{ShaderFxCatalog, ShaderFxEntry},
};
use lifthrasir_runtime::effects::procedural::{
    impact::FactorRamp,
    portal::{PortalMaterial, PortalVfx},
};
use lifthrasir_runtime::effects::{MapAmbientVfx, events::PlayProceduralVfx};
use lifthrasir_runtime::{EffectsPlaybackPlugin, RuntimeCorePlugin};
use std::collections::BTreeMap;

#[test]
fn generic_procedural_requests_update_without_gameplay_resources() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .add_plugins((
            ContentAssetsPlugin,
            RuntimeCorePlugin,
            EffectsPlaybackPlugin,
        ));
    app.insert_resource(ShaderFxCatalog::from_entries(BTreeMap::from([(
        "test".into(),
        ShaderFxEntry {
            kind: 1,
            primary: (1.0, 0.5, 0.2, 1.0),
            secondary: (1.0, 1.0, 1.0, 1.0),
            shape: (0.0, 0.0, 0.0, 0.0),
            duration: 1.0,
            scale: 2.0,
            light: None,
            garnish: None,
            texture: None,
            frames: None,
            travel: None,
        },
    )])));
    app.finish();
    app.cleanup();
    let portal = app
        .world_mut()
        .spawn((PortalVfx::default(), Transform::default()))
        .id();
    let smoke = app
        .world_mut()
        .spawn(MapAmbientVfx {
            key: "smoke".into(),
            emit_speed: 0.0,
            params: [0.0; 4],
        })
        .id();
    app.world_mut().write_message(PlayProceduralVfx {
        key: "test".into(),
        position: Vec3::ZERO,
        source: None,
        hits: 1,
        sound: None,
        color: Color::WHITE,
    });
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&FactorRamp>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world()
            .get::<MeshMaterial3d<PortalMaterial>>(portal)
            .is_some()
    );
    assert!(
        app.world()
            .get::<bevy_hanabi::ParticleEffect>(smoke)
            .is_some()
    );
}
