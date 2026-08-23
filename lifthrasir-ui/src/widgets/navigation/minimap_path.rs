use bevy::prelude::*;
use game_engine::domain::world::navigation::{ActiveRoute, densify};

use crate::{
    theme,
    widgets::minimap::{MinimapFrame, MinimapState, grid_to_frame_px},
};

const FRAME_SIZE: f32 = 180.0;
const SEGMENT_THICKNESS: f32 = 2.0;

#[derive(Component)]
pub struct NavigationSegment;

#[derive(Resource, Default)]
pub(super) struct NavigationSegmentKey(pub Option<(String, u64)>);

pub(super) fn reconcile_navigation_segments(
    active_route: Res<ActiveRoute>,
    frame: Query<Entity, With<MinimapFrame>>,
    state: Res<MinimapState>,
    existing: Query<Entity, With<NavigationSegment>>,
    mut key: ResMut<NavigationSegmentKey>,
    mut commands: Commands,
) {
    let Ok(frame) = frame.single() else {
        return;
    };
    let Some(route) = active_route.0.as_ref() else {
        despawn_navigation_segments(&existing, &mut commands);
        key.0 = None;
        return;
    };
    let Some(leg) = route.current_leg() else {
        despawn_navigation_segments(&existing, &mut commands);
        key.0 = None;
        return;
    };
    if leg.map != state.name {
        despawn_navigation_segments(&existing, &mut commands);
        key.0 = None;
        return;
    }

    let desired = (state.name.clone(), route.generation);
    if key.0.as_ref() == Some(&desired) {
        return;
    }

    despawn_navigation_segments(&existing, &mut commands);
    let cells = densify(route.cells_for_map(&state.name));
    for pair in cells.windows(2) {
        let (x0, y0) = grid_to_frame_px(
            pair[0].0,
            pair[0].1,
            state.width,
            state.height,
            FRAME_SIZE,
            FRAME_SIZE,
        );
        let (x1, y1) = grid_to_frame_px(
            pair[1].0,
            pair[1].1,
            state.width,
            state.height,
            FRAME_SIZE,
            FRAME_SIZE,
        );
        let (dx, dy) = (x1 - x0, y1 - y0);
        let length = dx.hypot(dy);
        let angle = dy.atan2(dx).to_degrees() + 90.0;
        commands.spawn((
            NavigationSegment,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((x0 + x1) * 0.5 - SEGMENT_THICKNESS * 0.5),
                top: Val::Px((y0 + y1) * 0.5 - length * 0.5),
                width: Val::Px(SEGMENT_THICKNESS),
                height: Val::Px(length),
                ..default()
            },
            UiTransform {
                rotation: Rot2::degrees(angle),
                ..default()
            },
            BackgroundColor(theme::MANA_BLUE),
            ZIndex(-1),
            Pickable::IGNORE,
            ChildOf(frame),
        ));
    }
    key.0 = Some(desired);
}

pub(super) fn reset_navigation_segments(
    existing: Query<Entity, With<NavigationSegment>>,
    mut key: ResMut<NavigationSegmentKey>,
    mut commands: Commands,
) {
    despawn_navigation_segments(&existing, &mut commands);
    key.0 = None;
}

fn despawn_navigation_segments(
    existing: &Query<Entity, With<NavigationSegment>>,
    commands: &mut Commands,
) {
    for entity in existing {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::world::navigation::Route;
    use net_contract::dto::{RouteDestination, RouteLeg};

    fn segment_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ActiveRoute>()
            .init_resource::<NavigationSegmentKey>()
            .add_systems(Update, reconcile_navigation_segments);
        app
    }

    fn segment_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<NavigationSegment>>()
            .iter(app.world())
            .count()
    }

    fn route(map: &str, cells: Vec<(u16, u16)>) -> Route {
        Route {
            generation: 1,
            legs: vec![RouteLeg {
                map: map.into(),
                cells,
                exit_portal: None,
                next_map: None,
            }],
            current: 0,
            destination: RouteDestination {
                map: map.into(),
                x: 0,
                y: 0,
            },
            hide_window: false,
        }
    }

    fn minimap_state(name: &str) -> MinimapState {
        MinimapState {
            name: name.into(),
            width: 100,
            height: 100,
            handle: None,
        }
    }

    fn segment_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<NavigationSegment>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn route_spawns_one_segment_per_densified_pair() {
        let mut app = segment_app();
        let frame = app.world_mut().spawn(MinimapFrame).id();
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (4, 1)]));

        app.update();

        assert_eq!(segment_count(&mut app), 3);
        let segments = segment_entities(&mut app);
        let children = app.world().get::<Children>(frame).unwrap();
        assert!(segments.iter().all(|entity| children.contains(entity)));
    }

    #[test]
    fn horizontal_segment_uses_vertical_bar_rotation_offset() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(50, 50), (51, 50)]));

        app.update();

        let (node, transform, z_index) = app
            .world_mut()
            .query::<(&Node, &UiTransform, &ZIndex)>()
            .single(app.world())
            .unwrap();
        assert!((transform.rotation.as_degrees() - 90.0).abs() < f32::EPSILON);
        assert!(matches!(node.left, Val::Px(left) if (left - 89.9).abs() < 0.001));
        assert!(matches!(node.top, Val::Px(top) if (top - 89.1).abs() < 0.001));
        assert_eq!(*z_index, ZIndex(-1));
    }

    #[test]
    fn wrong_map_clears_an_already_rendered_path() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));
        app.update();
        assert_eq!(segment_count(&mut app), 1);

        app.world_mut().resource_mut::<MinimapState>().name = "geffen".into();
        app.update();

        assert_eq!(segment_count(&mut app), 0);
        assert!(app.world().resource::<NavigationSegmentKey>().0.is_none());
    }

    #[test]
    fn missing_minimap_frame_is_a_no_op() {
        let mut app = segment_app();
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));

        app.update();

        assert_eq!(segment_count(&mut app), 0);
        app.world_mut().spawn(MinimapFrame);
        app.update();
        assert_eq!(segment_count(&mut app), 1);
    }

    #[test]
    fn clearing_route_despawns_all_segments() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));
        app.update();
        assert_eq!(segment_count(&mut app), 1);

        app.world_mut().resource_mut::<ActiveRoute>().0 = None;
        app.update();

        assert_eq!(segment_count(&mut app), 0);
        assert!(app.world().resource::<NavigationSegmentKey>().0.is_none());
    }

    #[test]
    fn unchanged_route_keeps_existing_segments() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));
        app.update();
        let original = segment_entities(&mut app);

        app.update();

        assert_eq!(segment_entities(&mut app), original);
    }

    #[test]
    fn changed_leg_rebuilds_segments() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        let mut route = route("prontera", vec![(1, 1), (2, 1)]);
        route.legs.push(RouteLeg {
            map: "prontera".into(),
            cells: vec![(3, 1), (4, 1)],
            exit_portal: None,
            next_map: None,
        });
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route.clone());
        app.update();
        let original = segment_entities(&mut app);

        route.current = 1;
        route.generation = 2;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(route);
        app.update();

        assert_eq!(segment_count(&mut app), 1);
        assert_ne!(segment_entities(&mut app), original);
    }

    #[test]
    fn changed_destination_rebuilds_segments_on_the_same_map() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));
        app.update();
        let original = segment_entities(&mut app);

        let mut replacement = route("prontera", vec![(3, 1), (4, 1)]);
        replacement.generation = 2;
        replacement.destination.x = 4;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(replacement);
        app.update();

        assert_ne!(segment_entities(&mut app), original);
    }

    #[test]
    fn reroute_rebuilds_segments_for_the_same_destination_and_leg() {
        let mut app = segment_app();
        app.world_mut().spawn(MinimapFrame);
        app.insert_resource(minimap_state("prontera"));
        app.world_mut().resource_mut::<ActiveRoute>().0 =
            Some(route("prontera", vec![(1, 1), (2, 1)]));
        app.update();
        let original = segment_entities(&mut app);

        let mut replacement = route("prontera", vec![(3, 1), (4, 1)]);
        replacement.generation = 2;
        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(replacement);
        app.update();

        assert_ne!(segment_entities(&mut app), original);
    }
}
