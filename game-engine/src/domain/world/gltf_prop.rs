//! Runtime side of the converted prop library: what a spawned prop glb needs
//! that glTF itself cannot express.
//!
//! Two things: the RSM materials are pure lambert (`reflectance = 0.0`), which
//! has no glTF equivalent and is therefore re-applied here on the loaded
//! asset, and the RSW-level animation intent (`anim_type`/`anim_speed`, which
//! lives in the map, not in the model) that decides how the glb's single baked
//! animation plays.
//!
//! [`wire_prop_scene`] is attached per prop with `.observe(...)` by the
//! spawner, not registered globally -- every other glb the app loads must stay
//! untouched.

use crate::domain::entities::systems::AnimationType;
use crate::presentation::rendering::models::rsw_anim_type_to_animation_type;
use bevy::animation::RepeatAnimation;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

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

/// Wires a freshly spawned prop scene: pure-lambert materials always, plus the
/// baked animation when the RSW asked for one.
///
/// A prop whose RSW says "animate" but whose glb has no animation (the RSM had
/// no keyframes) simply has no `AnimationPlayer` descendant -- the common case,
/// and not worth a warning.
pub fn wire_prop_scene(ready: On<WorldInstanceReady>, mut commands: Commands) {
    commands.run_system_cached_with(wire_prop_instance, ready.entity);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn wire_prop_instance(
    root: In<Entity>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    children: Query<&Children>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    props: Query<&PropAnim>,
    mut players: Query<&mut AnimationPlayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for descendant in children.iter_descendants(*root) {
        let Ok(handle) = mesh_materials.get(descendant) else {
            continue;
        };
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        material.reflectance = 0.0;
    }

    let Ok(prop) = props.get(*root) else {
        return;
    };
    let Some(repeat) = prop_repeat_mode(prop.anim_type) else {
        return;
    };
    let Some(player_entity) = children
        .iter_descendants(*root)
        .find(|entity| players.contains(*entity))
    else {
        return;
    };
    let Ok(mut player) = players.get_mut(player_entity) else {
        return;
    };

    let clip = asset_server.load(GltfAssetLabel::Animation(0).from_asset(prop.model.clone()));
    let (graph, node) = AnimationGraph::from_clip(clip);
    player
        .play(node)
        .set_repeat(repeat)
        .set_speed(prop.anim_speed);
    commands
        .entity(player_entity)
        .insert(AnimationGraphHandle(graphs.add(graph)));
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

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<StandardMaterial>()
            .init_asset::<AnimationClip>()
            .init_asset::<AnimationGraph>();
        app
    }

    fn spawn_prop(app: &mut App, anim_type: u32, with_player: bool) -> (Entity, Entity, Entity) {
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                reflectance: 0.5,
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
    fn maps_rsw_anim_types_to_repeat_modes() {
        assert_eq!(prop_repeat_mode(0), None);
        assert_eq!(prop_repeat_mode(1), Some(RepeatAnimation::Forever));
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
}
