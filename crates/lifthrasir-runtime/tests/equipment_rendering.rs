use bevy::prelude::*;
use lifthrasir_assets::{
    ContentAssetsPlugin, Gender,
    animation::{
        PendingAnimation, PendingAnimations,
        tags::{LAYER_BODY, LAYER_HEAD_TOP},
    },
    catalogs::{JobSpriteRegistry, WeaponDb},
};
use lifthrasir_runtime::actors::sprites::systems::events::EquipmentChangeEvent;
use lifthrasir_runtime::actors::{
    EntitySpriteData, EntitySpriteInfo, EquipmentSlot, PlayerAppearance,
};
use lifthrasir_runtime::{ActorRenderPlugin, RuntimeCorePlugin};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins((ContentAssetsPlugin, RuntimeCorePlugin, ActorRenderPlugin));
    app
}

fn actor(app: &mut App) -> Entity {
    let root = app
        .world_mut()
        .spawn((
            PlayerAppearance::default(),
            Gender::Male,
            EntitySpriteInfo {
                sprite_data: EntitySpriteData::Character {
                    job_id: 1,
                    gender: Gender::Male,
                    head: 0,
                    hair_color: 0,
                    riding: false,
                },
            },
        ))
        .id();
    app.world_mut().spawn(ChildOf(root));
    root
}

fn completed(root: Entity, layer_tag: moonshine_tag::Tag) -> PendingAnimation {
    PendingAnimation {
        sprite_handle: Handle::default(),
        action_handle: Handle::default(),
        palette_handle: None,
        layer_tag,
        callback_entity: Some(root),
    }
}

#[test]
fn removing_equipment_cancels_its_old_work_without_claiming_other_layers() {
    let mut app = app();
    let root = actor(&mut app);
    let other = app.world_mut().spawn_empty().id();
    let mut queue = app.world_mut().resource_mut::<PendingAnimations>();
    queue.request(
        Handle::default(),
        Handle::default(),
        None,
        LAYER_HEAD_TOP,
        Some(root),
    );
    queue.defer_completed(vec![
        (completed(root, LAYER_HEAD_TOP), Handle::default()),
        (completed(root, LAYER_BODY), Handle::default()),
        (completed(other, LAYER_HEAD_TOP), Handle::default()),
    ]);
    app.world_mut().write_message(EquipmentChangeEvent {
        character: root,
        slot: EquipmentSlot::HeadTop,
        view_id: None,
    });
    app.update();
    let mut queue = app.world_mut().resource_mut::<PendingAnimations>();
    assert!(!queue.has_pending());
    let remaining = queue.take_completed_where(|_| true);
    assert_eq!(remaining.len(), 2);
    assert!(
        remaining
            .iter()
            .all(|(request, _)| request.callback_entity != Some(root)
                || request.layer_tag != LAYER_HEAD_TOP)
    );
}

#[test]
fn completed_equipment_builds_and_unequips_without_client_components() {
    let mut app = app();
    let root = actor(&mut app);
    let animation = app
        .world_mut()
        .resource_mut::<Assets<lifthrasir_assets::animation::RoAnimationAsset>>()
        .add(lifthrasir_assets::animation::RoAnimationAsset {
            textures: Vec::new(),
            actions: Vec::new(),
            layer: LAYER_HEAD_TOP,
            sounds: Vec::new(),
        });
    app.world_mut()
        .resource_mut::<PendingAnimations>()
        .defer_completed(vec![(completed(root, LAYER_HEAD_TOP), animation.clone())]);
    app.update();
    let layers = |app: &mut App| {
        app.world_mut()
            .query::<(&ChildOf, &lifthrasir_runtime::actors::RenderLayer)>()
            .iter(app.world())
            .filter(|(parent, layer)| {
                parent.parent() == root && layer.equipment_slot == Some(EquipmentSlot::HeadTop)
            })
            .count()
    };
    assert_eq!(layers(&mut app), 1);
    assert_eq!(
        app.world()
            .get::<PlayerAppearance>(root)
            .unwrap()
            .equipment
            .get(&EquipmentSlot::HeadTop),
        Some(&animation)
    );
    app.world_mut().write_message(EquipmentChangeEvent {
        character: root,
        slot: EquipmentSlot::HeadTop,
        view_id: None,
    });
    app.update();
    assert_eq!(layers(&mut app), 0);
    assert!(
        !app.world()
            .get::<PlayerAppearance>(root)
            .unwrap()
            .equipment
            .contains_key(&EquipmentSlot::HeadTop)
    );
}

#[test]
fn weapon_requests_use_visual_metadata_without_character_stats() {
    let mut app = app();
    let root = actor(&mut app);
    app.insert_resource(JobSpriteRegistry::from_job_data(
        lifthrasir_data::JobData::default(),
    ));
    app.insert_resource(WeaponDb::from_weapon_data(lifthrasir_data::WeaponData {
        names: [(2, "_검".into())].into(),
        ..default()
    }));
    app.world_mut().write_message(EquipmentChangeEvent {
        character: root,
        slot: EquipmentSlot::Weapon,
        view_id: Some(2),
    });
    app.update();
    assert!(app.world().resource::<PendingAnimations>().has_pending());
}
