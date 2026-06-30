use crate::domain::entities::character::components::core::Grounded;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::RoAltitudeAsset;
use bevy::prelude::*;

/// Marks a fresh drop (`is_falling = true`) still animating its arc-and-bounce
/// onto the ground. Removed in favour of `Grounded` once the animation ends.
#[derive(Component, Default)]
pub struct FallingDrop {
    pub elapsed: f32,
}

const DROP_HEIGHT: f32 = 5.0;
const FALL_DURATION: f32 = 0.4;
const BOUNCE_HEIGHT: f32 = 0.6;
const FALL_PHASE_END: f32 = FALL_DURATION * 0.7;

/// Height above ground for a fresh drop at `elapsed` seconds into its
/// animation: a decelerating drop from `DROP_HEIGHT` followed by one small
/// bounce, then `None` once the animation has finished (`elapsed >= FALL_DURATION`).
fn falling_offset(elapsed: f32) -> Option<f32> {
    if elapsed >= FALL_DURATION {
        return None;
    }

    if elapsed < FALL_PHASE_END {
        let t = elapsed / FALL_PHASE_END;
        return Some(DROP_HEIGHT * (1.0 - t).powi(2));
    }

    let t = (elapsed - FALL_PHASE_END) / (FALL_DURATION - FALL_PHASE_END);
    Some(BOUNCE_HEIGHT * 4.0 * t * (1.0 - t))
}

/// Same altitude resource/asset access as `update_entity_altitude_system`
/// (`game-engine/src/domain/entities/movement/systems.rs`); falls back to the
/// entity's current height when no map altitude data is loaded.
fn ground_height_at(
    map_loader_query: &Query<&MapLoader>,
    altitude_assets: &Option<Res<Assets<RoAltitudeAsset>>>,
    world_pos: Vec3,
) -> Option<f32> {
    let altitude_assets = altitude_assets.as_ref()?;
    let map_loader = map_loader_query.single().ok()?;
    let altitude_handle = map_loader.altitude.as_ref()?;
    let altitude_asset = altitude_assets.get(altitude_handle)?;
    altitude_asset
        .altitude
        .get_terrain_height_at_position(world_pos)
}

pub fn animate_falling_drops(
    time: Res<Time>,
    map_loader_query: Query<&MapLoader>,
    altitude_assets: Option<Res<Assets<RoAltitudeAsset>>>,
    mut falling: Query<(Entity, &mut Transform, &mut FallingDrop)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut falling_drop) in &mut falling {
        falling_drop.elapsed += time.delta_secs();

        let ground = ground_height_at(&map_loader_query, &altitude_assets, transform.translation)
            .unwrap_or(transform.translation.y);

        match falling_offset(falling_drop.elapsed) {
            Some(offset) => transform.translation.y = ground + offset,
            None => {
                transform.translation.y = ground;
                commands
                    .entity(entity)
                    .remove::<FallingDrop>()
                    .insert(Grounded);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falling_offset_starts_elevated() {
        let offset = falling_offset(0.0).expect("animation running at elapsed 0");
        assert!(offset > 0.0);
        assert_eq!(offset, DROP_HEIGHT);
    }

    #[test]
    fn falling_offset_finishes_after_duration() {
        assert!(falling_offset(FALL_DURATION).is_none());
        assert!(falling_offset(FALL_DURATION + 1.0).is_none());
    }

    #[test]
    fn falling_offset_touches_ground_between_fall_and_bounce() {
        let offset = falling_offset(FALL_PHASE_END).expect("bounce phase still running");
        assert_eq!(offset, 0.0);
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app
    }

    #[test]
    fn finished_drop_swaps_falling_for_grounded() {
        let mut app = test_app();
        app.add_systems(Update, animate_falling_drops);

        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 9.0, 2.0),
                FallingDrop {
                    elapsed: FALL_DURATION,
                },
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<FallingDrop>(entity).is_none());
        assert!(world.get::<Grounded>(entity).is_some());
    }

    #[test]
    fn still_falling_drop_keeps_animating() {
        let mut app = test_app();
        app.add_systems(Update, animate_falling_drops);

        let entity = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 0.0, 0.0), FallingDrop::default()))
            .id();

        app.update();

        let world = app.world();
        assert!(world.get::<FallingDrop>(entity).is_some());
        assert!(world.get::<Grounded>(entity).is_none());
    }
}
