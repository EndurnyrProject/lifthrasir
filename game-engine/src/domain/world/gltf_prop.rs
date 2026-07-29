//! Runtime side of the converted prop library: what a spawned prop glb needs
//! that glTF itself cannot express.
//!
//! Two things: the RSM materials use the classic client's fixed-function
//! shading (pure lambert, no culling, no backface normal flip -- see
//! [`apply_ro_shading`]), which has no glTF equivalent and is therefore
//! re-applied here on the loaded asset, and the RSW-level animation intent
//! (`anim_type`/`anim_speed`, which lives in the map, not in the model) that
//! decides how the glb's single baked animation plays.
//!
//! [`wire_prop_scene`] is attached per prop with `.observe(...)` by the
//! spawner, not registered globally -- every other glb the app loads must stay
//! untouched.

use super::gltf_map::{CurrentMapNoShadeTint, LifPropRef, ROOT_FIX};
use crate::domain::entities::systems::AnimationType;
use bevy::animation::RepeatAnimation;
use bevy::gltf::{GltfAssetLabel, GltfExtras};
use bevy::math::Affine2;
use bevy::mesh::UvChannel;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use lifthrasir_data::lif::{EXTRAS_NO_SHADE, EXTRAS_UV_ANIMATION, LifUvAnimation, LifUvSample};

#[derive(Component)]
pub struct MapModel {
    pub filename: String,
    pub node_name: String,
}

/// Converts RSW's `anim_type` field to our enum. Most RO models should loop
/// by default for continuous animation.
pub(crate) fn rsw_anim_type_to_animation_type(anim_type: u32) -> AnimationType {
    match anim_type {
        0 => AnimationType::None,
        1 => AnimationType::Loop,
        2 => AnimationType::Once,
        _ => AnimationType::Loop,
    }
}

/// Attaches each converted prop scene to its map node once.
pub fn spawn_gltf_map_props(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    props: Query<(Entity, &LifPropRef), Without<MapModel>>,
) {
    for (entity, prop) in props.iter() {
        let path = &prop.0.model;

        commands.entity(entity).insert(MapModel {
            filename: path.clone(),
            node_name: String::new(),
        });

        if path.ends_with(".glb") {
            commands
                .spawn((
                    Transform::from_rotation(ROOT_FIX),
                    WorldAssetRoot(
                        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.clone())),
                    ),
                    PropAnim {
                        model: path.clone(),
                        anim_type: prop.0.anim_type,
                        anim_speed: prop.0.anim_speed,
                    },
                ))
                .observe(wire_prop_scene)
                .insert(ChildOf(entity));
        } else {
            error!("lif_prop ref '{path}' does not have a .glb extension");
        }
    }
}

/// On the `WorldAssetRoot` entity of a prop glb, carrying the RSW placement's
/// animation intent plus the asset path the glb was spawned from.
///
/// The path is the observer's contract with the spawner: the animation clip is
/// a *sibling sub-asset* of the scene handle
/// (`GltfAssetLabel::Animation(0)`), and a spawned scene keeps no back-pointer
/// to the glb it came from, so the spawner has to hand it over. It must be the
/// exact same `ro://` path the `WorldAssetRoot` handle was loaded with.
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component, Debug, Clone)]
pub struct PropAnim {
    pub model: String,
    pub anim_type: u32,
    pub anim_speed: f32,
}

#[derive(Component, Debug, Clone)]
pub(crate) struct PropUvAnimation {
    animation: LifUvAnimation,
    material: Handle<StandardMaterial>,
    player: Entity,
    node: AnimationNodeIndex,
    model: String,
}

/// The spawned scene instance is ready, but the glb's *assets* (materials,
/// textures) may still be loading -- scene spawn only needs the handles.
/// Wiring mutates the shared material assets, so running it against a
/// not-yet-loaded material silently does nothing and the model keeps glTF's
/// `double_sided: true` for the whole session -- which inverts lighting on
/// RO's CW-wound meshes (see [`apply_ro_shading`]). Which models lost that
/// race depended on load timing, so a different set broke on every login.
#[derive(Component)]
pub(crate) struct PropWiringPending;

/// Marks a freshly spawned prop scene for wiring once its assets exist.
pub fn wire_prop_scene(ready: On<WorldInstanceReady>, mut commands: Commands) {
    commands.entity(ready.entity).insert(PropWiringPending);
}

/// Wires pending prop scenes once every material asset their meshes reference
/// actually exists: pure-lambert materials always, plus the baked animation
/// when the RSW asked for one.
///
/// The gate is deliberately the *literal* precondition of the wiring -- the
/// `StandardMaterial` assets being present -- not the asset server's load
/// state. Load-state bookkeeping is not guaranteed to coincide with
/// `Assets<StandardMaterial>` insertion, and a failed texture dependency must
/// not stop the materials themselves from being treated. A spawned scene
/// implies its glb loaded, so its labeled materials are inserted within a
/// frame and this always terminates.
///
/// A prop whose RSW says "animate" but whose glb has no animation (the RSM had
/// no keyframes) simply has no `AnimationPlayer` descendant -- the common case,
/// and not worth a warning.
pub(crate) fn wire_pending_prop_scenes(
    mut commands: Commands,
    pending: Query<Entity, With<PropWiringPending>>,
    children: Query<&Children>,
    handles: Query<&MeshMaterial3d<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
) {
    for entity in &pending {
        let ready = children
            .iter_descendants(entity)
            .filter_map(|descendant| handles.get(descendant).ok())
            .all(|handle| materials.contains(&handle.0));
        if !ready {
            continue;
        }
        commands.entity(entity).remove::<PropWiringPending>();
        commands.run_system_cached_with(wire_prop_instance, entity);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_prop_instance(
    root: In<Entity>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tint: Res<CurrentMapNoShadeTint>,
    children: Query<&Children>,
    primitives: Query<(
        Option<&GltfExtras>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    props: Query<&PropAnim>,
    mut players: Query<&mut AnimationPlayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let Ok(prop) = props.get(*root) else {
        error!(entity = ?*root, "prop scene root has no PropAnim metadata");
        return;
    };
    let repeat = prop_repeat_mode(prop.anim_type);
    let player_entity = repeat.and_then(|_| {
        children
            .iter_descendants(*root)
            .find(|entity| players.contains(*entity))
    });
    let animation_target = player_entity.and_then(|entity| {
        let Ok(mut player) = players.get_mut(entity) else {
            error!(entity = ?entity, model = %prop.model, "prop animation player disappeared during scene wiring");
            return None;
        };
        let clip = asset_server.load(GltfAssetLabel::Animation(0).from_asset(prop.model.clone()));
        let (graph, node) = AnimationGraph::from_clip(clip);
        player
            .play(node)
            .set_repeat(repeat.expect("player is only selected for animated props"))
            .set_speed(prop.anim_speed);
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(graphs.add(graph)));
        Some((entity, node))
    });

    for descendant in children.iter_descendants(*root) {
        let Ok((extras, material_handle)) = primitives.get(descendant) else {
            continue;
        };
        let metadata =
            extras.and_then(|extras| parse_primitive_extras(descendant, &prop.model, extras));
        let no_shade = metadata.as_ref().is_some_and(|metadata| metadata.no_shade);
        let uv = metadata.and_then(|metadata| metadata.uv);
        let active_uv = repeat.is_some() && uv.is_some();

        if active_uv && animation_target.is_none() {
            error!(entity = ?descendant, model = %prop.model, "animated UV primitive has no descendant AnimationPlayer and graph node; retaining baked UV0");
        }
        let active_uv = active_uv
            .then_some(uv)
            .flatten()
            .filter(|_| animation_target.is_some());
        let needs_clone = no_shade || active_uv.is_some();

        let Some(material_handle) = material_handle else {
            if needs_clone {
                error!(entity = ?descendant, model = %prop.model, "prop primitive metadata has no target StandardMaterial; retaining baked appearance");
            }
            continue;
        };

        if !needs_clone {
            let Some(mut material) = materials.get_mut(&material_handle.0) else {
                error!(entity = ?descendant, model = %prop.model, "prop material asset missing at wiring time; model keeps baked glTF shading");
                continue;
            };
            apply_ro_shading(&mut material);
            continue;
        }

        let Some(mut material) = materials.get(&material_handle.0).cloned() else {
            error!(entity = ?descendant, model = %prop.model, "prop primitive target material asset is unavailable; retaining baked appearance");
            continue;
        };
        apply_ro_shading(&mut material);
        if no_shade {
            material.unlit = true;
            material.base_color = tinted(material.base_color, tint.0);
        }
        if let Some(animation) = active_uv.as_ref() {
            material.base_color_channel = UvChannel::Uv1;
            material.uv_transform =
                affine(animation.sample(0, false).expect("validated UV metadata"));
        }

        let material = materials.add(material);
        commands
            .entity(descendant)
            .insert(MeshMaterial3d(material.clone()));
        if let (Some(animation), Some((player, node))) = (active_uv, animation_target) {
            commands.entity(descendant).insert(PropUvAnimation {
                animation,
                material,
                player,
                node,
                model: prop.model.clone(),
            });
        }
    }
}

/// The classic client's fixed-function model shading: pure lambert
/// (`reflectance = 0.0`), no backface culling, and a surface lit identically
/// from both sides.
///
/// `double_sided: false` is deliberate and load-bearing, and must stay split
/// from `cull_mode: None`. glTF's `doubleSided` sets both, but Bevy's
/// `double_sided` flag additionally negates the normal on back faces -- and
/// mirrored RSW placements (negative-determinant transforms, e.g. scale
/// `[-1, 1, 1]`) flip triangle winding, so on those instances *every* visible
/// face rasterizes as a back face and the flip inverts their lighting. The
/// inverse-transpose normal is already correct under a mirror; with the flip
/// disabled, mirrored and unmirrored instances shade identically, matching
/// the original client (whose per-vertex lighting never depended on the
/// viewing side).
fn apply_ro_shading(material: &mut StandardMaterial) {
    material.reflectance = 0.0;
    material.double_sided = false;
    material.cull_mode = None;
}

#[derive(Default)]
struct PrimitiveMetadata {
    uv: Option<LifUvAnimation>,
    no_shade: bool,
}

fn parse_primitive_extras(
    entity: Entity,
    model: &str,
    extras: &GltfExtras,
) -> Option<PrimitiveMetadata> {
    let value: serde_json::Value = match serde_json::from_str(&extras.value) {
        Ok(value) => value,
        Err(error) => {
            error!(?entity, %model, %error, "malformed prop primitive extras; retaining baked appearance");
            return None;
        }
    };
    let Some(object) = value.as_object() else {
        error!(?entity, %model, "prop primitive extras are not a JSON object; retaining baked appearance");
        return None;
    };

    let uv = object.get(EXTRAS_UV_ANIMATION).and_then(|value| {
        let animation: LifUvAnimation = match serde_json::from_value(value.clone()) {
            Ok(animation) => animation,
            Err(error) => {
                error!(?entity, %model, %error, "malformed prop UV animation metadata; retaining baked UV0");
                return None;
            }
        };
        if let Err(error) = animation.validate() {
            error!(?entity, %model, %error, "invalid prop UV animation metadata; retaining baked UV0");
            return None;
        }
        Some(animation)
    });

    let no_shade = match object.get(EXTRAS_NO_SHADE) {
        None => false,
        Some(value) => match value.as_bool() {
            Some(value) => value,
            None => {
                error!(?entity, %model, "malformed prop no-shade metadata; retaining baked shading");
                false
            }
        },
    };
    Some(PrimitiveMetadata { uv, no_shade })
}

fn tinted(color: Color, tint: [f32; 3]) -> Color {
    let color = color.to_linear();
    Color::linear_rgba(
        color.red * tint[0],
        color.green * tint[1],
        color.blue * tint[2],
        color.alpha,
    )
}

fn affine(sample: LifUvSample) -> Affine2 {
    let [a, b, tx, c, d, ty, _, _, _] = sample.matrix3();
    Affine2::from_cols(Vec2::new(a, c), Vec2::new(b, d), Vec2::new(tx, ty))
}

pub(crate) fn play_prop_uv_animation(
    animations: Query<&PropUvAnimation>,
    players: Query<&AnimationPlayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for animation in &animations {
        let Ok(player) = players.get(animation.player) else {
            error!(entity = ?animation.player, model = %animation.model, "prop UV animation lost its AnimationPlayer");
            continue;
        };
        let Some(active) = player.animation(animation.node) else {
            error!(entity = ?animation.player, model = %animation.model, "prop UV animation lost its graph node");
            continue;
        };
        let Some(mut material) = materials.get_mut(&animation.material) else {
            error!(entity = ?animation.player, model = %animation.model, "prop UV animation lost its cloned material");
            continue;
        };
        let time_ms = seconds_to_millis(active.seek_time());
        let repeat = match active.repeat_mode() {
            RepeatAnimation::Forever => true,
            RepeatAnimation::Count(_) => !active.is_finished(),
            RepeatAnimation::Never => false,
        };
        match animation.animation.sample(time_ms, repeat) {
            Ok(sample) => material.uv_transform = affine(sample),
            Err(error) => {
                error!(entity = ?animation.player, model = %animation.model, %error, "prop UV animation became invalid")
            }
        }
    }
}

fn seconds_to_millis(seconds: f32) -> u32 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    ((seconds as f64 * 1000.0).min(u32::MAX as f64)) as u32
}

/// How the glb's baked animation should repeat, or `None` when the RSW says
/// this prop does not animate at all.
fn prop_repeat_mode(anim_type: u32) -> Option<RepeatAnimation> {
    match rsw_anim_type_to_animation_type(anim_type) {
        AnimationType::None => None,
        AnimationType::Loop => Some(RepeatAnimation::Forever),
        AnimationType::Once => Some(RepeatAnimation::Never),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetPlugin, LoadContext};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::gltf::extensions::{
        ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
    };
    use bevy::gltf::{GltfMaterial, GltfPlugin};
    use bevy::image::ImagePlugin;
    use bevy::mesh::{Mesh, MeshPlugin, VertexAttributeValues};
    use bevy::world_serialization::{WorldAsset, WorldSerializationPlugin};
    use lifthrasir_data::lif::LifProp;
    use std::time::{Duration, Instant};

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            WorldSerializationPlugin,
        ))
        .init_asset::<StandardMaterial>()
        .init_asset::<AnimationClip>()
        .init_asset::<AnimationGraph>()
        .init_resource::<CurrentMapNoShadeTint>();
        app
    }

    fn spawn_lif_prop(app: &mut App, model: &str) -> Entity {
        app.world_mut()
            .spawn(LifPropRef(LifProp {
                model: model.to_string(),
                anim_type: 1,
                anim_speed: 2.0,
            }))
            .id()
    }

    #[derive(Clone, Default)]
    struct StandardMaterialHandler;

    impl GltfExtensionHandler for StandardMaterialHandler {
        fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
            Box::new(self.clone())
        }

        fn on_material(
            &mut self,
            load_context: &mut LoadContext<'_>,
            _gltf_material: &gltf::Material,
            _material: Handle<GltfMaterial>,
            source: &GltfMaterial,
            material_label: &str,
        ) {
            load_context.add_labeled_asset(
                format!("{material_label}/std"),
                StandardMaterial {
                    base_color: source.base_color,
                    base_color_channel: source.base_color_channel.clone(),
                    base_color_texture: source.base_color_texture.clone(),
                    unlit: source.unlit,
                    ..default()
                },
            );
        }

        fn on_spawn_mesh_and_material(
            &mut self,
            load_context: &mut LoadContext<'_>,
            _primitive: &gltf::Primitive,
            _mesh: &gltf::Mesh,
            _material: &gltf::Material,
            entity: &mut EntityWorldMut,
            material_label: &str,
        ) {
            entity.insert(MeshMaterial3d(
                load_context.get_label_handle::<StandardMaterial>(format!("{material_label}/std")),
            ));
        }
    }

    fn loader_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: FIXTURES.to_string(),
                ..default()
            },
            ImagePlugin::default(),
            MeshPlugin,
            WorldSerializationPlugin,
            GltfPlugin::default(),
        ));
        app.world_mut()
            .resource_mut::<GltfExtensionHandlers>()
            .0
            .write_blocking()
            .push(Box::new(StandardMaterialHandler));
        app.init_asset::<StandardMaterial>()
            .register_type::<MeshMaterial3d<StandardMaterial>>()
            .init_asset::<AnimationClip>()
            .init_asset::<AnimationGraph>()
            .init_resource::<CurrentMapNoShadeTint>();
        app.finish();
        app.cleanup();
        app
    }

    fn spawn_prop(app: &mut App, anim_type: u32, with_player: bool) -> (Entity, Entity, Entity) {
        // Mimic what Bevy's glTF loader produces for a `doubleSided` material.
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                reflectance: 0.5,
                double_sided: true,
                cull_mode: None,
                ..default()
            });

        let mesh = app.world_mut().spawn(MeshMaterial3d(material)).id();
        let player = app.world_mut().spawn(AnimationPlayer::default()).id();
        let root = app
            .world_mut()
            .spawn(PropAnim {
                model: "ro://models/prop.glb".to_string(),
                anim_type,
                anim_speed: 2.5,
            })
            .id();

        app.world_mut().entity_mut(mesh).insert(ChildOf(root));
        if with_player {
            app.world_mut().entity_mut(player).insert(ChildOf(root));
        }
        (root, mesh, player)
    }

    fn reflectance_of(app: &App, mesh: Entity) -> f32 {
        let handle = app.world().get::<MeshMaterial3d<StandardMaterial>>(mesh);
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&handle.unwrap().0)
            .unwrap()
            .reflectance
    }

    #[test]
    fn rsm_ref_is_logged_and_skipped() {
        let mut app = test_app();
        let entity = spawn_lif_prop(&mut app, "ro://data/model/prontera/tree01.rsm");

        app.world_mut()
            .run_system_once(spawn_gltf_map_props)
            .unwrap();

        let world = app.world();
        assert!(world.get::<MapModel>(entity).is_some());
        assert!(world.get::<Children>(entity).is_none());
    }

    #[test]
    fn glb_ref_spawns_a_child_gltf_scene() {
        let mut app = test_app();
        let entity = spawn_lif_prop(&mut app, "ro://models/prontera/tree01.glb");

        app.world_mut()
            .run_system_once(spawn_gltf_map_props)
            .unwrap();

        let world = app.world();
        assert_eq!(
            world.get::<MapModel>(entity).unwrap().filename,
            "ro://models/prontera/tree01.glb"
        );
        let children = world.get::<Children>(entity).expect("one child spawned");
        assert_eq!(children.len(), 1);
        let child = children[0];
        assert_eq!(world.get::<Transform>(child).unwrap().rotation, ROOT_FIX);
        assert!(world.get::<WorldAssetRoot>(child).is_some());
        let anim = world.get::<PropAnim>(child).expect("PropAnim on the child");
        assert_eq!(anim.model, "ro://models/prontera/tree01.glb");
        assert_eq!(anim.anim_type, 1);
        assert_eq!(anim.anim_speed, 2.0);
    }

    #[test]
    fn an_entity_is_only_dispatched_once() {
        let mut app = test_app();
        let entity = spawn_lif_prop(&mut app, "ro://models/prontera/tree01.glb");

        app.world_mut()
            .run_system_once(spawn_gltf_map_props)
            .unwrap();
        app.world_mut()
            .run_system_once(spawn_gltf_map_props)
            .unwrap();

        assert_eq!(app.world().get::<Children>(entity).unwrap().len(), 1);
    }

    #[test]
    fn an_unrecognized_extension_is_logged_and_skipped() {
        let mut app = test_app();
        let entity = spawn_lif_prop(&mut app, "ro://models/prontera/tree01.rsm2");

        app.world_mut()
            .run_system_once(spawn_gltf_map_props)
            .unwrap();

        let world = app.world();
        assert!(world.get::<MapModel>(entity).is_some());
        assert!(world.get::<Children>(entity).is_none());
    }

    #[test]
    fn real_glb_imports_primitive_extras_uv1_and_drives_the_cloned_material() {
        let mut app = loader_app();
        app.world_mut().resource_mut::<CurrentMapNoShadeTint>().0 = [0.5, 1.0, 1.0];
        let scene: Handle<WorldAsset> = app
            .world()
            .resource::<AssetServer>()
            .load(GltfAssetLabel::Scene(0).from_asset("uv_prop.glb"));
        let scene_state = scene.clone();
        let root = app
            .world_mut()
            .spawn((
                WorldAssetRoot(scene),
                PropAnim {
                    model: "uv_prop.glb".to_string(),
                    anim_type: 1,
                    anim_speed: 1.0,
                },
            ))
            .id();

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            app.update();
            let state = app
                .world()
                .resource::<AssetServer>()
                .load_state(&scene_state);
            let world = app.world_mut();
            let extras = world.query::<&GltfExtras>().iter(world).count();
            let players = world.query::<&AnimationPlayer>().iter(world).count();
            let meshes = world.query::<&Mesh3d>().iter(world).count();
            if state.is_loaded() && extras == 1 && players == 1 && meshes == 1 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "uv_prop.glb never spawned: {state:?}, extras={extras}, players={players}, meshes={meshes}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        let source_handle = {
            let world = app.world_mut();
            let mut query = world.query::<&MeshMaterial3d<StandardMaterial>>();
            query.single(world).unwrap().0.clone()
        };
        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();
        app.world_mut().flush();

        let (animation, extras, mesh_handle, material_handle) = {
            let world = app.world_mut();
            let mut query = world.query::<(
                &PropUvAnimation,
                &GltfExtras,
                &Mesh3d,
                &MeshMaterial3d<StandardMaterial>,
            )>();
            let mut found = query.iter(world);
            let (animation, extras, mesh, material) = found.next().expect("animated primitive");
            assert!(found.next().is_none());
            (
                animation.clone(),
                extras.value.clone(),
                mesh.0.clone(),
                material.0.clone(),
            )
        };
        assert!(extras.contains(EXTRAS_UV_ANIMATION));
        let meshes = app.world().resource::<Assets<Mesh>>();
        let uv1 = meshes
            .get(&mesh_handle)
            .and_then(|mesh| mesh.attribute(Mesh::ATTRIBUTE_UV_1))
            .expect("TEXCOORD_1 imported");
        assert!(matches!(uv1, VertexAttributeValues::Float32x2(values) if values.len() == 3));

        {
            let materials = app.world().resource::<Assets<StandardMaterial>>();
            let source = materials.get(&source_handle).expect("source material");
            let clone = materials.get(&material_handle).expect("cloned material");
            assert_ne!(source_handle, material_handle);
            assert_eq!(source.base_color_channel, UvChannel::Uv0);
            assert_eq!(clone.base_color_channel, UvChannel::Uv1);
            assert_eq!(clone.uv_transform, Affine2::IDENTITY);
        }

        app.world_mut()
            .get_mut::<AnimationPlayer>(animation.player)
            .unwrap()
            .animation_mut(animation.node)
            .unwrap()
            .set_seek_time(0.5);
        app.world_mut()
            .run_system_cached(play_prop_uv_animation)
            .unwrap();
        let transform = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&material_handle)
            .unwrap()
            .uv_transform;
        assert_eq!(transform.translation, Vec2::new(0.5, 0.0));
    }

    /// The race this guards: scene instances become ready before the glb's
    /// material assets exist, so eager wiring silently skipped the material
    /// mutation and the model kept glTF's `double_sided: true` all session.
    /// Mirrors production: the observer sets the marker at instance-ready and
    /// the pending system defers until the material assets are present.
    #[test]
    fn pending_wiring_defers_until_material_assets_exist_then_applies_ro_shading() {
        let mut app = loader_app();
        let scene: Handle<WorldAsset> = app
            .world()
            .resource::<AssetServer>()
            .load(GltfAssetLabel::Scene(0).from_asset("uv_prop.glb"));
        let root = app
            .world_mut()
            .spawn((
                WorldAssetRoot(scene),
                PropAnim {
                    model: "uv_prop.glb".to_string(),
                    anim_type: 0,
                    anim_speed: 1.0,
                },
            ))
            .id();
        app.world_mut().entity_mut(root).observe(wire_prop_scene);

        let deadline = Instant::now() + Duration::from_secs(30);
        let mut marker_seen = false;
        loop {
            app.update();
            marker_seen |= app.world().get::<PropWiringPending>(root).is_some();
            app.world_mut()
                .run_system_once(wire_pending_prop_scenes)
                .unwrap();
            app.world_mut().flush();
            if marker_seen && app.world().get::<PropWiringPending>(root).is_none() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "prop wiring never ran (marker_seen={marker_seen})"
            );
            std::thread::sleep(Duration::from_millis(2));
        }

        let handle = {
            let world = app.world_mut();
            let mut query = world.query::<&MeshMaterial3d<StandardMaterial>>();
            query.single(world).unwrap().0.clone()
        };
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let material = materials.get(&handle).expect("material asset loaded");
        assert_eq!(material.reflectance, 0.0);
        assert!(!material.double_sided);
        assert_eq!(material.cull_mode, None);
    }

    #[test]
    fn maps_rsw_anim_types_to_repeat_modes() {
        assert_eq!(prop_repeat_mode(0), None);
        assert_eq!(prop_repeat_mode(1), Some(RepeatAnimation::Forever));
        assert_eq!(prop_repeat_mode(2), Some(RepeatAnimation::Never));
        assert_eq!(prop_repeat_mode(99), Some(RepeatAnimation::Forever));
    }

    #[test]
    fn zeroes_reflectance_on_the_shared_material_asset() {
        let mut app = test_app();
        let (root, mesh, _) = spawn_prop(&mut app, 0, false);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        assert_eq!(reflectance_of(&app, mesh), 0.0);
    }

    /// Mirrored RSW placements flip triangle winding; Bevy's `double_sided`
    /// backface normal flip would invert their lighting, so RO shading must
    /// keep culling off while clearing `double_sided`.
    #[test]
    fn ro_shading_disables_double_sided_flip_but_keeps_culling_off() {
        let mut app = test_app();
        let (root, mesh, _) = spawn_prop(&mut app, 0, false);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        let handle = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(mesh)
            .unwrap()
            .0
            .clone();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let material = materials.get(&handle).unwrap();
        assert!(!material.double_sided);
        assert_eq!(material.cull_mode, None);
    }

    #[test]
    fn plays_the_baked_animation_when_the_rsw_asks_for_one() {
        let mut app = test_app();
        let (root, _, player) = spawn_prop(&mut app, 1, true);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        assert!(app.world().get::<AnimationGraphHandle>(player).is_some());
        let animation = app
            .world()
            .get::<AnimationPlayer>(player)
            .unwrap()
            .playing_animations()
            .next()
            .expect("the baked animation should be playing")
            .1;
        assert_eq!(animation.repeat_mode(), RepeatAnimation::Forever);
        assert_eq!(animation.speed(), 2.5);
    }

    #[test]
    fn leaves_the_player_alone_when_the_rsw_says_static() {
        let mut app = test_app();
        let (root, mesh, player) = spawn_prop(&mut app, 0, true);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        assert!(app.world().get::<AnimationGraphHandle>(player).is_none());
        assert_eq!(
            app.world()
                .get::<AnimationPlayer>(player)
                .unwrap()
                .playing_animations()
                .count(),
            0
        );
        assert_eq!(reflectance_of(&app, mesh), 0.0);
    }

    #[test]
    fn tolerates_a_glb_without_an_animation_player() {
        let mut app = test_app();
        let (root, mesh, _) = spawn_prop(&mut app, 1, false);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        assert_eq!(reflectance_of(&app, mesh), 0.0);
    }

    fn uv_animation() -> LifUvAnimation {
        use lifthrasir_data::lif::{LifScalarKey, LifUvChannel, LifUvProperty};

        LifUvAnimation {
            duration_ms: 1_000,
            channels: vec![LifUvChannel {
                property: LifUvProperty::TranslateU,
                keys: vec![
                    LifScalarKey {
                        time_ms: 0,
                        value: 0.25,
                    },
                    LifScalarKey {
                        time_ms: 1_000,
                        value: 1.25,
                    },
                ],
            }],
        }
    }

    fn target_extras(no_shade: bool) -> GltfExtras {
        GltfExtras {
            value: serde_json::json!({
                EXTRAS_UV_ANIMATION: uv_animation(),
                EXTRAS_NO_SHADE: no_shade,
            })
            .to_string(),
        }
    }

    fn spawn_target(
        app: &mut App,
        source: Handle<StandardMaterial>,
        anim_type: u32,
        no_shade: bool,
    ) -> (Entity, Entity) {
        let primitive = app
            .world_mut()
            .spawn((MeshMaterial3d(source), target_extras(no_shade)))
            .id();
        let player = app.world_mut().spawn(AnimationPlayer::default()).id();
        let root = app
            .world_mut()
            .spawn(PropAnim {
                model: "ro://models/uv_prop.glb".to_string(),
                anim_type,
                anim_speed: 1.0,
            })
            .id();
        app.world_mut().entity_mut(primitive).insert(ChildOf(root));
        app.world_mut().entity_mut(player).insert(ChildOf(root));
        (root, primitive)
    }

    #[test]
    fn combined_uv_and_no_shade_clone_once_without_mutating_source() {
        let mut app = test_app();
        app.insert_resource(CurrentMapNoShadeTint([0.5, 0.25, 1.0]));
        let source = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::linear_rgba(0.8, 0.4, 0.2, 0.75),
                reflectance: 0.5,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
        let source_value = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&source)
            .unwrap()
            .clone();
        let (root, primitive) = spawn_target(&mut app, source.clone(), 1, true);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        let rebound = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(primitive)
            .unwrap();
        assert_ne!(rebound.0, source);
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(materials.len(), 2, "UV + no-shade must share one clone");
        assert_eq!(
            materials.get(&source).unwrap().reflectance,
            source_value.reflectance
        );
        assert_eq!(
            materials.get(&source).unwrap().base_color,
            source_value.base_color
        );
        let clone = materials.get(&rebound.0).unwrap();
        assert_eq!(clone.reflectance, 0.0);
        assert!(!clone.double_sided);
        assert_eq!(clone.cull_mode, None);
        assert!(clone.unlit);
        assert_eq!(clone.base_color_channel, UvChannel::Uv1);
        assert_eq!(
            clone.uv_transform,
            affine(uv_animation().sample(0, false).unwrap())
        );
        assert_eq!(clone.base_color, Color::linear_rgba(0.4, 0.1, 0.2, 0.75));
        assert!(app.world().get::<PropUvAnimation>(primitive).is_some());
    }

    #[test]
    fn static_uv_metadata_keeps_baked_uv0_and_does_not_clone() {
        let mut app = test_app();
        let source = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let (root, primitive) = spawn_target(&mut app, source.clone(), 0, false);

        app.world_mut()
            .run_system_cached_with(wire_prop_instance, root)
            .unwrap();

        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(primitive)
                .unwrap()
                .0,
            source
        );
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(materials.len(), 1);
        let material = materials.get(&source).unwrap();
        assert_eq!(material.base_color_channel, UvChannel::Uv0);
        assert_eq!(material.uv_transform, Affine2::IDENTITY);
        assert!(app.world().get::<PropUvAnimation>(primitive).is_none());
    }

    #[test]
    fn two_instances_get_independent_materials() {
        let mut app = test_app();
        let source = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let (first_root, first) = spawn_target(&mut app, source.clone(), 1, false);
        let (second_root, second) = spawn_target(&mut app, source, 1, false);
        app.world_mut()
            .run_system_cached_with(wire_prop_instance, first_root)
            .unwrap();
        app.world_mut()
            .run_system_cached_with(wire_prop_instance, second_root)
            .unwrap();

        let first = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(first)
            .unwrap();
        let second = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(second)
            .unwrap();
        assert_ne!(first.0, second.0);
    }

    fn playback_app(repeat: RepeatAnimation, seek_time: f32) -> (App, Handle<StandardMaterial>) {
        let mut app = test_app();
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let (_, node) = AnimationGraph::from_clip(Handle::default());
        let mut player = AnimationPlayer::default();
        player
            .play(node)
            .set_repeat(repeat)
            .set_seek_time(seek_time);
        let player = app.world_mut().spawn(player).id();
        app.world_mut().spawn(PropUvAnimation {
            animation: uv_animation(),
            material: material.clone(),
            player,
            node,
            model: "ro://models/uv_prop.glb".to_string(),
        });
        (app, material)
    }

    #[test]
    fn playback_uses_loop_phase_and_once_terminal_hold() {
        let (mut looping, loop_material) = playback_app(RepeatAnimation::Forever, 1.0);
        looping
            .world_mut()
            .run_system_cached(play_prop_uv_animation)
            .unwrap();
        assert_eq!(
            looping
                .world()
                .resource::<Assets<StandardMaterial>>()
                .get(&loop_material)
                .unwrap()
                .uv_transform,
            affine(uv_animation().sample(0, false).unwrap())
        );

        let (mut once, once_material) = playback_app(RepeatAnimation::Never, 1.0);
        once.world_mut()
            .run_system_cached(play_prop_uv_animation)
            .unwrap();
        assert_eq!(
            once.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&once_material)
                .unwrap()
                .uv_transform,
            affine(uv_animation().sample(1_000, false).unwrap())
        );
    }

    #[test]
    fn paused_and_zero_speed_players_hold_their_authoritative_seek_time() {
        let (mut app, material) = playback_app(RepeatAnimation::Forever, 0.4);
        let state = {
            let world = app.world_mut();
            let mut query = world.query::<&PropUvAnimation>();
            query.single(world).unwrap().clone()
        };
        app.world_mut()
            .get_mut::<AnimationPlayer>(state.player)
            .unwrap()
            .animation_mut(state.node)
            .unwrap()
            .pause()
            .set_speed(0.0);

        app.world_mut()
            .run_system_cached(play_prop_uv_animation)
            .unwrap();
        let expected = affine(uv_animation().sample(400, true).unwrap());
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap()
                .uv_transform,
            expected
        );
        app.world_mut()
            .run_system_cached(play_prop_uv_animation)
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material)
                .unwrap()
                .uv_transform,
            expected
        );
    }
}
