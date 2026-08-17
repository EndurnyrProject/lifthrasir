//! World-space light pillars for server-driven viewpoint markers.
//!
//! One entity per marker `id` is the single source of truth: it carries the
//! pillar mesh as a child, plus [`Grounded`] (auto terrain height) and
//! [`MapScoped`] (auto teardown on warp/teleport/death/logout). The timed
//! marker additionally carries [`ViewpointExpiry`] for its 15s auto-remove.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use net_contract::events::ViewpointChanged;

use crate::core::GameState;
use crate::domain::entities::character::components::core::Grounded;
use crate::domain::world::map_scoped::MapScoped;
use crate::utils::coordinates::spawn_coords_to_world_position;

/// Pillar height in world units. The cuboid is center-origin, so the child is
/// offset by half this so its base rests on the grounded parent origin.
const PILLAR_HEIGHT: f32 = 40.0;
/// Pillar cross-section width/depth in world units.
const PILLAR_WIDTH: f32 = 2.5;

/// A server-driven marker slot at grid cell `(x, y)`.
#[derive(Component, Debug)]
pub struct ViewpointMarker {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub color: Color,
}

/// Auto-remove timer for the timed marker; absent on persistent markers.
#[derive(Component, Debug)]
pub struct ViewpointExpiry(pub Timer);

/// Resolved payload for a marker slot after folding one frame's event batch.
struct ShowData {
    x: u16,
    y: u16,
    color: Color,
    ttl: Option<Duration>,
}

/// Consume [`ViewpointChanged`] and reconcile the per-`id` marker entities.
///
/// Events are folded in order so each `id` resolves to a single final state
/// (last event wins), then every previously-existing marker in a resolved slot
/// is despawned before its replacement is spawned. A slot therefore stays at
/// exactly one entity even when one frame's batch holds several messages for
/// the same `id`.
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn apply_viewpoint_changes(
    mut events: MessageReader<ViewpointChanged>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    markers: Query<(Entity, &ViewpointMarker)>,
) {
    let mut resolved: HashMap<u32, Option<ShowData>> = HashMap::new();
    for event in events.read() {
        match event {
            ViewpointChanged::Remove { id } => {
                resolved.insert(*id, None);
            }
            ViewpointChanged::Show {
                id,
                x,
                y,
                color,
                ttl,
            } => {
                resolved.insert(
                    *id,
                    Some(ShowData {
                        x: *x,
                        y: *y,
                        color: *color,
                        ttl: *ttl,
                    }),
                );
            }
        }
    }

    for (entity, marker) in markers.iter() {
        if resolved.contains_key(&marker.id) {
            commands.entity(entity).despawn();
        }
    }

    for (id, show) in resolved {
        let Some(show) = show else { continue };
        let marker = ViewpointMarker {
            id,
            x: show.x,
            y: show.y,
            color: show.color,
        };
        spawn_marker(&mut commands, &mut meshes, &mut materials, marker, show.ttl);
    }
}

/// Spawn one marker pillar: a `Grounded`/`MapScoped` root plus a child mesh.
fn spawn_marker(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    marker: ViewpointMarker,
    ttl: Option<Duration>,
) {
    let position = spawn_coords_to_world_position(marker.x, marker.y);
    let material = materials.add(StandardMaterial {
        base_color: marker.color.with_alpha(0.35),
        emissive: LinearRgba::from(marker.color),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let mesh = meshes.add(Mesh::from(Cuboid::new(
        PILLAR_WIDTH,
        PILLAR_HEIGHT,
        PILLAR_WIDTH,
    )));

    let mut entity = commands.spawn((
        marker,
        Transform::from_translation(position),
        Visibility::default(),
        Grounded,
        MapScoped,
    ));
    if let Some(duration) = ttl {
        entity.insert(ViewpointExpiry(Timer::new(duration, TimerMode::Once)));
    }
    // World up is -Y: offset the center-origin cuboid negative-Y so its base
    // rests on the grounded parent origin and the pillar rises upward.
    entity.with_child((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, -PILLAR_HEIGHT / 2.0, 0.0),
    ));
}

/// Tick [`ViewpointExpiry`] timers and despawn markers whose time is up.
#[auto_add_system(
    plugin = crate::domain::world::WorldDomainPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn expire_viewpoint_markers(
    mut commands: Commands,
    time: Res<Time>,
    mut markers: Query<(Entity, &mut ViewpointExpiry)>,
) {
    for (entity, mut expiry) in &mut markers {
        if expiry.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::registry::EntityRegistry;
    use crate::domain::world::map_scoped::despawn_map_scoped;
    use bevy::state::app::StatesPlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<Time>()
            .add_message::<ViewpointChanged>()
            .add_systems(
                Update,
                (apply_viewpoint_changes, expire_viewpoint_markers).chain(),
            );
        app
    }

    fn show(id: u32, ttl: Option<Duration>) -> ViewpointChanged {
        ViewpointChanged::Show {
            id,
            x: 10,
            y: 20,
            color: Color::srgb_u8(255, 128, 0),
            ttl,
        }
    }

    fn marker_ids(app: &mut App) -> Vec<u32> {
        let mut ids: Vec<u32> = app
            .world_mut()
            .query::<&ViewpointMarker>()
            .iter(app.world())
            .map(|marker| marker.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    fn markers(app: &mut App) -> Vec<(u32, u16, u16, Color)> {
        app.world_mut()
            .query::<&ViewpointMarker>()
            .iter(app.world())
            .map(|marker| (marker.id, marker.x, marker.y, marker.color))
            .collect()
    }

    #[test]
    fn show_spawns_one_marker_with_the_pillar_mesh() {
        let mut app = test_app();
        app.world_mut().write_message(show(1, None));
        app.update();

        assert_eq!(marker_ids(&mut app), vec![1]);

        // The pillar mesh is a child offset to -Y (world up is -Y) so its base
        // rests on the grounded parent origin.
        let mut children = app.world_mut().query_filtered::<&Transform, With<Mesh3d>>();
        let transforms: Vec<_> = children.iter(app.world()).collect();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].translation.y, -PILLAR_HEIGHT / 2.0);
    }

    #[test]
    fn second_show_for_the_same_id_replaces_instead_of_duplicating() {
        let mut app = test_app();
        app.world_mut().write_message(show(1, None));
        app.update();
        app.world_mut().write_message(show(1, None));
        app.update();

        assert_eq!(marker_ids(&mut app), vec![1]);
    }

    #[test]
    fn remove_despawns_the_matching_marker() {
        let mut app = test_app();
        app.world_mut().write_message(show(1, None));
        app.update();
        app.world_mut()
            .write_message(ViewpointChanged::Remove { id: 1 });
        app.update();

        assert!(marker_ids(&mut app).is_empty());
    }

    #[test]
    fn timed_marker_expires_after_its_ttl_while_persistent_survives() {
        let mut app = test_app();
        app.world_mut()
            .write_message(show(1, Some(Duration::from_secs(15))));
        app.world_mut().write_message(show(2, None));
        app.update();

        assert_eq!(marker_ids(&mut app), vec![1, 2]);
        let mut expiring = app
            .world_mut()
            .query_filtered::<Entity, With<ViewpointExpiry>>();
        assert_eq!(expiring.iter(app.world()).count(), 1);

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(16));
        app.update();

        assert_eq!(marker_ids(&mut app), vec![2]);
    }

    #[test]
    fn exiting_ingame_despawns_all_markers() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_resource::<Time>()
            .add_message::<ViewpointChanged>()
            .add_plugins(StatesPlugin)
            .init_state::<GameState>()
            .init_resource::<EntityRegistry>()
            .add_systems(
                Update,
                (apply_viewpoint_changes, expire_viewpoint_markers).chain(),
            )
            .add_systems(OnExit(GameState::InGame), despawn_map_scoped);

        app.world_mut().write_message(show(1, None));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        assert_eq!(marker_ids(&mut app), vec![1]);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Loading);
        app.update();

        assert!(marker_ids(&mut app).is_empty());
    }

    #[test]
    fn two_shows_same_id_in_one_update_spawn_one_marker_with_last_data() {
        let mut app = test_app();
        app.world_mut().write_message(ViewpointChanged::Show {
            id: 1,
            x: 10,
            y: 20,
            color: Color::srgb_u8(255, 128, 0),
            ttl: None,
        });
        app.world_mut().write_message(ViewpointChanged::Show {
            id: 1,
            x: 30,
            y: 40,
            color: Color::srgb_u8(0, 0, 255),
            ttl: None,
        });
        app.update();

        assert_eq!(
            markers(&mut app),
            vec![(1, 30, 40, Color::srgb_u8(0, 0, 255))]
        );
    }

    #[test]
    fn show_then_remove_same_id_in_one_update_leaves_no_marker() {
        let mut app = test_app();
        app.world_mut().write_message(show(1, None));
        app.world_mut()
            .write_message(ViewpointChanged::Remove { id: 1 });
        app.update();

        assert!(marker_ids(&mut app).is_empty());
    }

    #[test]
    fn remove_then_show_same_id_in_one_update_spawns_one_marker() {
        let mut app = test_app();
        app.world_mut()
            .write_message(ViewpointChanged::Remove { id: 1 });
        app.world_mut().write_message(show(1, None));
        app.update();

        assert_eq!(marker_ids(&mut app), vec![1]);
    }
}
