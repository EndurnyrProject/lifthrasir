use bevy::prelude::*;
use lifthrasir_runtime::RuntimeCorePlugin;
use lifthrasir_runtime::audio::events::{PlayBgmEvent, PlaySkillSfx};
use lifthrasir_runtime::effects::events::PlayProceduralVfx;
use lifthrasir_runtime::rendering::{SharedSpriteQuad, SpriteRenderingSystems};

#[derive(Resource, Default)]
struct Phases(Vec<&'static str>);

#[test]
fn world_coordinate_calibration_survives_the_move() {
    use lifthrasir_runtime::rendering::coordinates::{
        Direction, spawn_coords_to_world_position, world_position_to_spawn_coords,
    };
    let point = spawn_coords_to_world_position(2, 3);
    assert_eq!(point, Vec3::new(10.0, 0.0, -15.0));
    assert_eq!(world_position_to_spawn_coords(point), (2, 3));
    assert_eq!(Direction::from_movement_vector(10.0, 0.0), Direction::East);
}

#[test]
fn silent_core_updates_and_orders_phases_without_client_state() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Phases>()
        .add_plugins(RuntimeCorePlugin)
        .add_systems(
            Update,
            (
                (|mut phases: ResMut<Phases>| phases.0.push("playback"))
                    .in_set(SpriteRenderingSystems::AnimationPlayback),
                (|mut phases: ResMut<Phases>| phases.0.push("spawn"))
                    .in_set(SpriteRenderingSystems::HierarchySpawn),
                (|mut phases: ResMut<Phases>| phases.0.push("populate"))
                    .in_set(SpriteRenderingSystems::AssetPopulation),
            ),
        );
    app.update();
    assert_eq!(
        app.world().resource::<Phases>().0,
        ["spawn", "populate", "playback"]
    );
    assert!(app.world().contains_resource::<SharedSpriteQuad>());
    assert!(app.world().contains_resource::<Messages<PlayBgmEvent>>());
    assert!(app.world().contains_resource::<Messages<PlaySkillSfx>>());
    assert!(
        app.world()
            .contains_resource::<Messages<PlayProceduralVfx>>()
    );
    app.update();
}
