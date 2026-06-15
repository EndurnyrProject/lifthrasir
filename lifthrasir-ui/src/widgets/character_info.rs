//! Character-info panel: name, job, base/job level, and HP/SP bars for the local
//! player. Reflects the `LocalPlayer`'s `CharacterStatus`/`CharacterData` into the
//! HUD elements by `CssID`, writing only when a value actually changed so we don't
//! churn `Changed<Paragraph>`/`Changed<ProgressBar>` every frame.

use bevy::prelude::*;
use bevy_extended_ui::styles::CssID;
use bevy_extended_ui::widgets::{Paragraph, ProgressBar};
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::core::CharacterData;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::components::EntityName;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::infrastructure::lua_scripts::job::registry::JobSpriteRegistry;

const NAME_ID: &str = "char-name";
const JOB_ID: &str = "char-job";
const LEVEL_ID: &str = "char-level";
const HP_TEXT_ID: &str = "hp-text";
const SP_TEXT_ID: &str = "sp-text";
const HP_BAR_ID: &str = "hp-bar";
const SP_BAR_ID: &str = "sp-bar";

pub struct CharacterInfoPlugin;

impl Plugin for CharacterInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_character_info.run_if(in_state(GameState::InGame)),
        );
    }
}

/// `current/max` as a 0..=100 percentage (extended_ui `ProgressBar` range).
fn percentage(current: u32, max: u32) -> f32 {
    if max == 0 {
        0.0
    } else {
        (current as f32 / max as f32) * 100.0
    }
}

fn set_text(texts: &mut Query<(&mut Paragraph, &CssID)>, id: &str, value: &str) {
    for (mut paragraph, css_id) in texts.iter_mut() {
        if css_id.0 == id && paragraph.text != value {
            paragraph.text = value.to_string();
        }
    }
}

fn set_bar(bars: &mut Query<(&mut ProgressBar, &CssID)>, id: &str, value: f32) {
    for (mut bar, css_id) in bars.iter_mut() {
        if css_id.0 == id && (bar.value - value).abs() > f32::EPSILON {
            bar.value = value;
        }
    }
}

fn update_character_info(
    player: Query<(&CharacterStatus, &CharacterData, Option<&EntityName>), With<LocalPlayer>>,
    job_registry: Option<Res<JobSpriteRegistry>>,
    mut texts: Query<(&mut Paragraph, &CssID)>,
    mut bars: Query<(&mut ProgressBar, &CssID)>,
) {
    let Ok((status, data, entity_name)) = player.single() else {
        return;
    };

    let name = entity_name
        .map(|n| n.name.clone())
        .unwrap_or_else(|| data.name.clone());
    let job_name = job_registry
        .as_deref()
        .and_then(|registry| registry.get_display_name(data.job_id as u32))
        .unwrap_or("Unknown");

    set_text(&mut texts, NAME_ID, &name);
    set_text(&mut texts, JOB_ID, job_name);
    set_text(
        &mut texts,
        LEVEL_ID,
        &format!(
            "Base Lv. {} / Job Lv. {}",
            status.base_level, status.job_level
        ),
    );
    set_text(
        &mut texts,
        HP_TEXT_ID,
        &format!("HP {} / {}", status.hp, status.max_hp),
    );
    set_text(
        &mut texts,
        SP_TEXT_ID,
        &format!("SP {} / {}", status.sp, status.max_sp),
    );
    set_bar(&mut bars, HP_BAR_ID, percentage(status.hp, status.max_hp));
    set_bar(&mut bars, SP_BAR_ID, percentage(status.sp, status.max_sp));
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::entities::character::components::core::CharacterStats;

    #[test]
    fn percentage_basic_ratios() {
        assert_eq!(percentage(50, 100), 50.0);
        assert_eq!(percentage(0, 0), 0.0);
        assert_eq!(percentage(100, 100), 100.0);
    }

    #[test]
    fn hp_bar_reflects_half_health() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let bar = app
            .world_mut()
            .spawn((ProgressBar::default(), CssID(HP_BAR_ID.to_string())))
            .id();

        app.world_mut().spawn((
            CharacterStatus {
                hp: 50,
                max_hp: 100,
                ..default()
            },
            CharacterData {
                name: "Hero".to_string(),
                job_id: 0,
                level: 1,
                experience: 0,
                stats: CharacterStats::default(),
                slot: 0,
            },
            LocalPlayer,
        ));

        app.add_systems(Update, update_character_info);
        app.update();

        let value = app.world().get::<ProgressBar>(bar).unwrap().value;
        assert_eq!(value, 50.0);
    }
}
