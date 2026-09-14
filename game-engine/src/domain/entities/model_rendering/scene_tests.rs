use super::*;
use crate::domain::{
    entities::character::states::AnimationState,
    guild::{
        GuildPlugin, GuildSystems,
        emblems::{EmblemKey, GuildEmblemImages},
    },
};
use bevy::gltf::{Gltf, GltfMaterialName};
use lifthrasir_data::gr2::{EMBLEM_MATERIAL, IDLE};
use net_contract::{
    commands::GuildEmblemFetchRequested,
    events::{GuildIngress, GuildIngressPayload, ZoneDisconnected},
    state::ZoneSessionGeneration,
};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Gltf>>()
        .init_resource::<Assets<AnimationClip>>()
        .init_resource::<Assets<AnimationGraph>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_message::<GuildIngress>()
        .add_message::<ZoneDisconnected>()
        .insert_resource(ZoneSessionGeneration(1))
        .add_plugins(GuildPlugin)
        .add_systems(Update, systems::wire_scenes.before(GuildSystems::UiSync))
        .add_systems(Update, systems::sync_emblems.in_set(GuildSystems::UiSync));
    app
}

fn gltf(app: &mut App, idle: bool) -> Handle<Gltf> {
    let mut clip = AnimationClip::default();
    clip.set_duration(1.0);
    let clip = app
        .world_mut()
        .resource_mut::<Assets<AnimationClip>>()
        .add(clip);
    app.world_mut().resource_mut::<Assets<Gltf>>().add(Gltf {
        scenes: vec![],
        named_scenes: default(),
        meshes: vec![],
        named_meshes: default(),
        materials: vec![],
        named_materials: default(),
        nodes: vec![],
        named_nodes: default(),
        skins: vec![],
        named_skins: default(),
        default_scene: None,
        animations: vec![clip.clone()],
        named_animations: if idle {
            [(IDLE.into(), clip)].into()
        } else {
            default()
        },
        source: None,
    })
}

fn scene(
    app: &mut App,
    gltf: Handle<Gltf>,
    material: Handle<StandardMaterial>,
    guild: u32,
) -> (Entity, Entity, Entity, Entity) {
    let actor = app
        .world_mut()
        .spawn((
            AnimationState::Idle,
            GuildFlag {
                guild_id: guild,
                emblem_id: 7,
            },
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((
            systems::ModelScene {
                model: "ro://models/3dmob/test.glb".into(),
                gltf,
            },
            systems::SceneReady,
            ChildOf(actor),
        ))
        .id();
    let node = app.world_mut().spawn(ChildOf(root)).id();
    let player = app
        .world_mut()
        .spawn((AnimationPlayer::default(), ChildOf(node)))
        .id();
    let mesh = app
        .world_mut()
        .spawn((
            Mesh3d(Handle::default()),
            MeshMaterial3d(material),
            GltfMaterialName(EMBLEM_MATERIAL.into()),
            ChildOf(node),
        ))
        .id();
    (actor, root, player, mesh)
}

fn texture(app: &App, mesh: Entity) -> Option<Handle<Image>> {
    let handle = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(mesh)
        .unwrap();
    app.world()
        .resource::<Assets<StandardMaterial>>()
        .get(&handle.0)
        .unwrap()
        .base_color_texture
        .clone()
}

#[test]
fn delayed_materials_wire_once_and_emblems_are_isolated_by_owner() {
    let mut app = app();
    let gltf = gltf(&mut app, true);
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let source = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color_texture: Some(image.clone()),
            ..default()
        });
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .remove(&source)
        .unwrap();
    let (first, root, player, first_mesh) = scene(&mut app, gltf.clone(), source.clone(), 42);
    let (_, second_root, _, second_mesh) = scene(&mut app, gltf, source.clone(), 43);
    app.update();
    assert!(app.world().get::<systems::SceneReady>(root).is_some());
    assert!(
        app.world()
            .get::<animation::ActorPlayback>(player)
            .is_none()
    );
    app.world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .insert(source.id(), material)
        .unwrap();
    app.update();
    assert!(app.world().get::<systems::SceneReady>(root).is_none());
    assert!(
        app.world()
            .get::<systems::SceneReady>(second_root)
            .is_none()
    );
    assert!(
        app.world()
            .get::<animation::ActorPlayback>(player)
            .is_some()
    );
    let first_handle = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(first_mesh)
        .unwrap()
        .0
        .clone();
    let second_handle = app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(second_mesh)
        .unwrap()
        .0
        .clone();
    assert_ne!(first_handle, second_handle);
    assert_ne!(first_handle, source);
    for _ in 0..2 {
        let request = app
            .world()
            .resource::<Messages<GuildEmblemFetchRequested>>()
            .iter_current_update_messages()
            .last()
            .unwrap();
        let (guild_id, emblem_id) = (request.guild_id, request.emblem_id);
        let data = crate::domain::guild::emblems::test_emblem_bmp(24, 24);
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(1),
            payload: GuildIngressPayload::EmblemData {
                guild_id,
                emblem_id,
                data,
            },
        });
        app.update();
    }
    let cache = app.world().resource::<GuildEmblemImages>();
    assert_eq!(
        texture(&app, first_mesh),
        cache.cached(EmblemKey::new(42, 7).unwrap())
    );
    assert_eq!(
        texture(&app, second_mesh),
        cache.cached(EmblemKey::new(43, 7).unwrap())
    );
    assert_ne!(texture(&app, first_mesh), texture(&app, second_mesh));
    app.world_mut()
        .get_mut::<GuildFlag>(first)
        .unwrap()
        .guild_id = 44;
    app.update();
    assert_eq!(texture(&app, first_mesh), Some(image.clone()));
    app.world_mut().write_message(ZoneDisconnected {
        reason: "test reset".into(),
    });
    app.update();
    assert_eq!(texture(&app, second_mesh), Some(image.clone()));
    assert_eq!(
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&source)
            .unwrap()
            .base_color_texture,
        Some(image)
    );
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(first_mesh)
            .unwrap()
            .0,
        first_handle
    );
}

#[test]
#[should_panic(expected = "has no idle animation")]
fn missing_required_idle_animation_fails_loudly() {
    let mut app = app();
    let gltf = gltf(&mut app, false);
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    scene(&mut app, gltf, material, 0);
    app.update();
}

#[test]
fn all_facings_are_world_relative() {
    use crate::{domain::world::gltf_map::ROOT_FIX, utils::coordinates::Direction};
    let directions = [
        Vec3::NEG_Z,
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::NEG_X,
        Vec3::new(-1.0, 0.0, 1.0),
        Vec3::Z,
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::X,
        Vec3::new(1.0, 0.0, -1.0),
    ];
    for (i, expected) in directions.into_iter().enumerate() {
        let rotation = systems::facing_rotation(Direction::from_u8(i as u8)) * ROOT_FIX.inverse();
        assert!((rotation * Vec3::NEG_Z).abs_diff_eq(expected.normalize(), 1e-5));
    }
}
