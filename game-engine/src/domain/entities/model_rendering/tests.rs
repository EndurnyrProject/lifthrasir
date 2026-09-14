use super::*;
use crate::domain::entities::{
    components::{GuildIdentity, NetworkEntity},
    registry::EntityRegistry,
    spawning::systems::spawn_network_entity_system,
    sprite_rendering::events::RequestSpriteSpawn,
};
use crate::infrastructure::job::JobSpriteRegistry;
use net_contract::events::UnitEntered;

#[derive(Resource, Default)]
struct SpriteRequests(usize);

#[test]
fn model_scene_is_a_single_owned_child_and_is_replaced_when_the_model_changes() {
    use bevy::{asset::AssetPlugin, gltf::Gltf, world_serialization::WorldAsset};
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Gltf>()
        .init_asset::<WorldAsset>()
        .add_systems(Update, systems::sync_model_scenes);
    let actor = app
        .world_mut()
        .spawn((
            Gr2Actor {
                model: "ro://models/3dmob/guildflag90_1.glb".into(),
            },
            Transform::from_xyz(10.0, -5.0, 20.0).with_scale(Vec3::splat(2.0)),
            Visibility::Hidden,
        ))
        .id();
    app.update();
    let root = app
        .world()
        .get::<systems::ModelSceneLink>(actor)
        .expect("model scene child")
        .0;
    assert_eq!(app.world().get::<ChildOf>(root).unwrap().parent(), actor);
    app.update();
    assert_eq!(
        app.world().get::<systems::ModelSceneLink>(actor).unwrap().0,
        root
    );
    app.world_mut().get_mut::<Gr2Actor>(actor).unwrap().model =
        "ro://models/3dmob/empelium90_0.glb".into();
    app.update();
    assert!(app.world().get_entity(root).is_err());
    let replacement = app.world().get::<systems::ModelSceneLink>(actor).unwrap().0;
    assert_ne!(replacement, root);
    assert_eq!(
        app.world().get::<Transform>(actor).unwrap().scale,
        Vec3::splat(2.0)
    );
    assert_eq!(
        *app.world().get::<Visibility>(actor).unwrap(),
        Visibility::Hidden
    );
    app.world_mut().entity_mut(actor).despawn();
    assert!(app.world().get_entity(replacement).is_err());
}

fn unit(gid: u32, job: u32, object_type: u32) -> UnitEntered {
    UnitEntered {
        gid,
        aid: gid,
        object_type,
        job,
        x: 10,
        y: 10,
        dir: 0,
        speed: 150,
        hp: 100,
        max_hp: 100,
        clevel: 1,
        body_state: 0,
        health_state: 0,
        effect_state: 0,
        virtue: 0,
        spirit_sphere_count: 0,
        head: 0,
        weapon: 0,
        shield: 0,
        accessory: 0,
        accessory2: 0,
        accessory3: 0,
        head_palette: 0,
        body_palette: 0,
        head_dir: 0,
        robe: 0,
        guild_id: 42,
        guild_name: "Guild".into(),
        emblem_id: 7,
        sex: 0,
        is_boss: false,
        name: "Actor".into(),
        display_size: 0,
        moving: false,
        dst_x: 0,
        dst_y: 0,
        move_start_time: 0,
    }
}

fn spawn_app() -> App {
    let mut data = lifthrasir_data::JobData::default();
    for (id, name) in [
        (722, "Guildflag90_1.gr2"),
        (1285, "Aguardian90_8.gr2"),
        (1287, "Sguardian90_9.gr2"),
        (1288, "Empelium90_0.gr2"),
        (1324, "TREASUREBOX_2.gr2"),
        (1002, "poring"),
        (46, "1_ETC_01"),
    ] {
        data.npc_sprites.insert(id, name.into());
    }
    let mut app = App::new();
    app.init_resource::<EntityRegistry>()
        .init_resource::<SpriteRequests>()
        .insert_resource(JobSpriteRegistry::from_job_data(data))
        .add_message::<UnitEntered>()
        .add_systems(Update, spawn_network_entity_system)
        .add_observer(
            |_: On<RequestSpriteSpawn>, mut requests: ResMut<SpriteRequests>| requests.0 += 1,
        );
    app
}

#[test]
fn reentry_refreshes_model_owner_and_facing_without_moving_the_entity() {
    use crate::domain::entities::character::components::visual::{CharacterDirection, Direction};
    let mut app = spawn_app();
    app.world_mut().write_message(unit(1, 722, 1));
    app.update();
    let actor = app
        .world()
        .resource::<EntityRegistry>()
        .get_entity(1)
        .unwrap();
    let position = app.world().get::<Transform>(actor).unwrap().translation;
    let mut updated = unit(1, 722, 1);
    updated.x = 99;
    updated.guild_id = 43;
    updated.emblem_id = 9;
    updated.dir = 6;
    updated.display_size = 1;
    app.world_mut().write_message(updated);
    app.update();
    assert_eq!(
        app.world().get::<GuildFlag>(actor).unwrap(),
        &GuildFlag {
            guild_id: 43,
            emblem_id: 9
        }
    );
    assert_eq!(
        app.world().get::<CharacterDirection>(actor).unwrap().facing,
        Direction::East
    );
    assert_eq!(
        app.world().get::<Transform>(actor).unwrap().translation,
        position
    );
    assert_eq!(
        app.world().get::<Transform>(actor).unwrap().scale,
        Vec3::splat(0.5)
    );
    app.world_mut().write_message(unit(1, 1285, 1));
    app.update();
    assert!(app.world().get::<GuildFlag>(actor).is_none());
    assert_eq!(
        app.world().get::<Gr2Actor>(actor).unwrap().model,
        "ro://models/3dmob/aguardian90_8.glb"
    );
    assert_eq!(app.world().resource::<SpriteRequests>().0, 0);
}

#[test]
fn ordinary_sprites_and_warp_portals_keep_their_existing_paths() {
    let mut app = spawn_app();
    for (gid, job, kind) in [(1, 1002, 5), (2, 46, 1), (3, 45, 1), (4, 0, 0)] {
        app.world_mut().write_message(unit(gid, job, kind));
    }
    app.update();
    assert_eq!(app.world().resource::<SpriteRequests>().0, 3);
    assert_eq!(
        app.world_mut()
            .query::<&Gr2Actor>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&crate::domain::entities::markers::WarpPortal>()
            .iter(app.world())
            .count(),
        1
    );
}

#[test]
fn gr2_npcs_mobs_and_companions_bypass_sprite_requests() {
    let mut app = spawn_app();
    for (gid, job, kind) in [
        (1, 722, 1),
        (2, 1285, 5),
        (3, 1287, 6),
        (4, 1288, 7),
        (5, 1324, 8),
    ] {
        app.world_mut().write_message(unit(gid, job, kind));
    }
    app.update();
    assert_eq!(app.world().resource::<SpriteRequests>().0, 0);
    assert_eq!(
        app.world_mut()
            .query::<&Gr2Actor>()
            .iter(app.world())
            .count(),
        5
    );
    let flag = app
        .world_mut()
        .query::<&GuildFlag>()
        .single(app.world())
        .unwrap();
    assert_eq!((flag.guild_id, flag.emblem_id), (42, 7));
    assert_eq!(
        app.world_mut()
            .query::<&GuildIdentity>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&NetworkEntity>()
            .iter(app.world())
            .count(),
        5
    );
}
