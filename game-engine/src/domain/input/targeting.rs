use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_system, auto_init_resource};

use crate::core::state::GameState;
use crate::domain::skill::{CastTarget, SkillCastResolved};
use crate::domain::system_sets::InputSystems;

use super::cursor::{CursorType, handle_cursor_change_requests};
use super::events::CursorChangeRequest;
use super::resources::ForwardedMouseClick;
use super::systems::{handle_terrain_click, update_cursor_for_terrain};
use super::terrain_raycast::TerrainRaycastCache;

/// RO's press-then-click targeting state machine: `resolve_skill_cast` arms it
/// for entity/ground skills, and the click handlers below consume it.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_init_resource(plugin = crate::app::input_plugin::InputPlugin)]
pub enum TargetingMode {
    #[default]
    Idle,
    AwaitingEntity {
        skill_id: u32,
        level: u32,
    },
    AwaitingGround {
        skill_id: u32,
        level: u32,
    },
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        in_set = InputSystems::Click,
        run_if = in_state(GameState::InGame),
        before = handle_terrain_click
    )
)]
pub fn targeting_click(
    mut targeting: ResMut<TargetingMode>,
    mut mouse_click: ResMut<ForwardedMouseClick>,
    cache: Res<TerrainRaycastCache>,
    mut resolved: MessageWriter<SkillCastResolved>,
) {
    // Entity-target skills are resolved by the sprite picking observer
    // (`entities::picking::on_sprite_click`); here we handle only ground casts.
    // The click is consumed for any armed mode so an empty-ground click while a
    // skill is armed never leaks into player movement.
    if *targeting == TargetingMode::Idle {
        return;
    }

    if mouse_click.position.take().is_none() {
        return;
    }

    let TargetingMode::AwaitingGround { skill_id, level } = *targeting else {
        return;
    };

    let Some((x, y)) = cache.cell_coords else {
        return;
    };

    resolved.write(SkillCastResolved {
        skill_id,
        level,
        target: CastTarget::Ground(x, y),
    });
    *targeting = TargetingMode::Idle;
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame))
)]
pub fn cancel_targeting(mut targeting: ResMut<TargetingMode>, keys: Res<ButtonInput<KeyCode>>) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }

    *targeting = TargetingMode::Idle;
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(
        run_if = in_state(GameState::InGame),
        after = update_cursor_for_terrain,
        before = handle_cursor_change_requests
    )
)]
pub fn targeting_cursor(
    targeting: Res<TargetingMode>,
    mut cursor_messages: MessageWriter<CursorChangeRequest>,
) {
    if *targeting == TargetingMode::Idle {
        return;
    }

    cursor_messages.write(CursorChangeRequest::new(CursorType::Attack));
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = OnExit(GameState::InGame)
)]
pub fn disarm_on_exit(mut targeting: ResMut<TargetingMode>) {
    *targeting = TargetingMode::Idle;
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL_ID: u32 = 28;
    const LEVEL: u32 = 3;

    fn click_app() -> App {
        let mut app = App::new();
        app.init_resource::<TargetingMode>()
            .init_resource::<ForwardedMouseClick>()
            .init_resource::<TerrainRaycastCache>()
            .add_message::<SkillCastResolved>()
            .add_systems(Update, targeting_click);
        app
    }

    fn arm(app: &mut App, mode: TargetingMode) {
        *app.world_mut().resource_mut::<TargetingMode>() = mode;
    }

    fn click(app: &mut App) {
        app.world_mut()
            .resource_mut::<ForwardedMouseClick>()
            .position = Some(Vec2::ZERO);
    }

    fn mode(app: &App) -> TargetingMode {
        *app.world().resource::<TargetingMode>()
    }

    fn click_consumed(app: &App) -> bool {
        app.world()
            .resource::<ForwardedMouseClick>()
            .position
            .is_none()
    }

    fn resolved_msgs(app: &App) -> Vec<SkillCastResolved> {
        app.world()
            .resource::<Messages<SkillCastResolved>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn ground_click_resolves_to_cell_and_disarms() {
        let mut app = click_app();
        app.world_mut()
            .resource_mut::<TerrainRaycastCache>()
            .cell_coords = Some((120, 80));
        arm(
            &mut app,
            TargetingMode::AwaitingGround {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );
        click(&mut app);
        app.update();

        let msgs = resolved_msgs(&app);
        assert_eq!(msgs.len(), 1);
        let CastTarget::Ground(x, y) = msgs[0].target else {
            panic!("expected a ground target");
        };
        assert_eq!((x, y), (120, 80));
        assert_eq!(mode(&app), TargetingMode::Idle);
        assert!(click_consumed(&app));
    }

    #[test]
    fn entity_click_without_hover_stays_armed_but_consumes_click() {
        let mut app = click_app();
        arm(
            &mut app,
            TargetingMode::AwaitingEntity {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );
        click(&mut app);
        app.update();

        assert!(resolved_msgs(&app).is_empty());
        assert_eq!(
            mode(&app),
            TargetingMode::AwaitingEntity {
                skill_id: SKILL_ID,
                level: LEVEL,
            }
        );
        assert!(click_consumed(&app));
    }

    #[test]
    fn idle_click_passes_through_untouched() {
        let mut app = click_app();
        click(&mut app);
        app.update();

        assert!(resolved_msgs(&app).is_empty());
        assert!(!click_consumed(&app));
    }

    #[test]
    fn escape_cancels_targeting() {
        let mut app = App::new();
        app.init_resource::<TargetingMode>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, cancel_targeting);
        arm(
            &mut app,
            TargetingMode::AwaitingEntity {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn disarm_on_exit_resets_to_idle() {
        let mut app = App::new();
        app.init_resource::<TargetingMode>()
            .add_systems(Update, disarm_on_exit);
        arm(
            &mut app,
            TargetingMode::AwaitingGround {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );
        app.update();

        assert_eq!(mode(&app), TargetingMode::Idle);
    }

    #[test]
    fn cursor_switches_to_attack_when_armed() {
        let mut app = App::new();
        app.init_resource::<TargetingMode>()
            .add_message::<CursorChangeRequest>()
            .add_systems(Update, targeting_cursor);
        app.update();

        arm(
            &mut app,
            TargetingMode::AwaitingGround {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );
        app.update();

        let cursor = app
            .world()
            .resource::<Messages<CursorChangeRequest>>()
            .iter_current_update_messages()
            .last()
            .map(|m| m.cursor_type);
        assert_eq!(cursor, Some(CursorType::Attack));
    }

    #[test]
    fn armed_cursor_wins_over_terrain_cursor_every_frame() {
        use super::super::cursor::{CurrentCursorType, handle_cursor_change_requests};
        use super::super::systems::update_cursor_for_terrain;
        use crate::domain::entities::hover::CurrentlyHoveredEntity;

        let mut app = App::new();
        app.init_resource::<TargetingMode>()
            .init_resource::<CurrentCursorType>()
            .init_resource::<CurrentlyHoveredEntity>()
            .init_resource::<TerrainRaycastCache>()
            .add_message::<CursorChangeRequest>()
            .add_systems(
                Update,
                (
                    update_cursor_for_terrain,
                    targeting_cursor,
                    handle_cursor_change_requests,
                )
                    .chain(),
            );

        {
            let mut cache = app.world_mut().resource_mut::<TerrainRaycastCache>();
            cache.is_walkable = true;
            cache.cell_coords = Some((10, 10));
        }
        arm(
            &mut app,
            TargetingMode::AwaitingGround {
                skill_id: SKILL_ID,
                level: LEVEL,
            },
        );

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<CurrentCursorType>().get(),
            CursorType::Attack
        );
    }
}
