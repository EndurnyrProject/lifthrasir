//! Skill-targeting toast: a small `SkillName Lv.N` label that follows the cursor
//! while a skill is armed and waiting for the player to pick a target.
//!
//! RO shows the skill you're about to cast attached to the targeting cursor; this
//! mirrors that. It reads `TargetingMode` (armed by `resolve_skill_cast` for
//! entity/ground skills) and resolves the display name from `SkillCatalog`. The
//! level travels in the `TargetingMode` variant itself. Self/no-target skills cast
//! immediately and never arm targeting, so they never show a toast.
//!
//! Spawn/despawn is driven entirely by the update system (like the hotbar drag
//! ghost) rather than `show_hud`: it appears when targeting arms and vanishes when
//! it resolves, is cancelled, or the game exits.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_engine::core::state::GameState;
use game_engine::domain::input::targeting::TargetingMode;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;
use crate::worldspace::viewport_to_ui;

/// Offset of the toast's top-left from the cursor tip, so the label sits
/// below-right of the pointer without covering the cell being aimed at.
const CURSOR_OFFSET_X: f32 = 18.0;
const CURSOR_OFFSET_Y: f32 = 20.0;

/// Above the HUD, below the hotbar drag ghost (`GHOST_Z = 2000`).
const TOAST_Z: i32 = 1800;

/// The cursor-following targeting toast; stores its current label so the system
/// can tell when the armed skill changed and needs a fresh text child.
#[derive(Component)]
struct SkillTargetToast {
    label: String,
}

pub struct SkillTargetToastPlugin;

impl Plugin for SkillTargetToastPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_skill_target_toast.run_if(in_state(GameState::InGame)),
        );
    }
}

/// The armed skill's `Name Lv.N`, or `None` when idle or the skill has no catalog
/// entry (no fabricated placeholder — a nameless skill simply shows nothing).
fn toast_label(targeting: &TargetingMode, catalog: Option<&SkillCatalog>) -> Option<String> {
    let (skill_id, level) = match targeting {
        TargetingMode::Idle => return None,
        TargetingMode::AwaitingEntity { skill_id, level }
        | TargetingMode::AwaitingGround { skill_id, level } => (*skill_id, *level),
    };
    let name = catalog?.get(skill_id)?.display_name.clone();
    Some(format!("{name} Lv.{level}"))
}

/// Spawns, moves, relabels, or despawns the toast to match the armed state and
/// cursor each frame.
fn update_skill_target_toast(
    mut commands: Commands,
    targeting: Res<TargetingMode>,
    catalog: Option<Res<SkillCatalog>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    asset_server: Res<AssetServer>,
    mut toasts: Query<(Entity, &SkillTargetToast, &mut Node)>,
) {
    let existing = toasts.single_mut().ok();
    let label = toast_label(&targeting, catalog.as_deref());
    let cursor = windows.single().ok().and_then(Window::cursor_position);

    let (Some(label), Some(cursor)) = (label, cursor) else {
        if let Some((entity, _, _)) = existing {
            commands.entity(entity).despawn();
        }
        return;
    };

    let cursor = viewport_to_ui(cursor, &ui_scale);
    let left = Val::Px(cursor.x + CURSOR_OFFSET_X);
    let top = Val::Px(cursor.y + CURSOR_OFFSET_Y);

    match existing {
        Some((entity, toast, mut node)) => {
            node.left = left;
            node.top = top;
            if toast.label != label {
                commands.entity(entity).despawn();
                spawn_toast(&mut commands, &asset_server, label, left, top);
            }
        }
        None => spawn_toast(&mut commands, &asset_server, label, left, top),
    }
}

fn spawn_toast(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: String,
    left: Val,
    top: Val,
) {
    let font = asset_server.load(theme::FONT_BODY);
    let container = commands
        .spawn((
            SkillTargetToast {
                label: label.clone(),
            },
            Node {
                position_type: PositionType::Absolute,
                left,
                top,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS_2),
            BorderColor::all(theme::EMERALD_DEEP),
            GlobalZIndex(TOAST_Z),
            Pickable::IGNORE,
            DespawnOnExit(GameState::InGame),
        ))
        .id();
    commands.spawn((
        theme::label(label, font, 12.0, theme::TEXT),
        ChildOf(container),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> SkillCatalog {
        use lifthrasir_data::{SkillData, SkillMeta};
        let mut data = SkillData::default();
        data.skills.insert(
            17,
            SkillMeta {
                name: "MG_FIREBOLT".to_string(),
                display_name: "Fire Bolt".to_string(),
                description: vec![],
                max_level: 10,
                sp_cost: vec![12],
                attack_range: vec![9],
            },
        );
        SkillCatalog::from_skill_data(data)
    }

    #[test]
    fn idle_has_no_label() {
        assert_eq!(toast_label(&TargetingMode::Idle, Some(&catalog())), None);
    }

    #[test]
    fn awaiting_entity_shows_name_and_level() {
        let label = toast_label(
            &TargetingMode::AwaitingEntity {
                skill_id: 17,
                level: 5,
            },
            Some(&catalog()),
        );
        assert_eq!(label.as_deref(), Some("Fire Bolt Lv.5"));
    }

    #[test]
    fn awaiting_ground_shows_name_and_level() {
        let label = toast_label(
            &TargetingMode::AwaitingGround {
                skill_id: 17,
                level: 3,
            },
            Some(&catalog()),
        );
        assert_eq!(label.as_deref(), Some("Fire Bolt Lv.3"));
    }

    #[test]
    fn unknown_skill_has_no_label() {
        let label = toast_label(
            &TargetingMode::AwaitingEntity {
                skill_id: 9999,
                level: 1,
            },
            Some(&catalog()),
        );
        assert_eq!(label, None);
    }

    #[test]
    fn missing_catalog_has_no_label() {
        let label = toast_label(
            &TargetingMode::AwaitingEntity {
                skill_id: 17,
                level: 5,
            },
            None,
        );
        assert_eq!(label, None);
    }
}
