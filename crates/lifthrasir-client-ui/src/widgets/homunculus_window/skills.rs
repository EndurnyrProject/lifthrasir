use bevy::{prelude::*, ui_widgets::Activate};
use bevy_feathers::controls::FeathersButton;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::domain::{
    homunculus::{
        HomunculusCastIntent, HomunculusIntent, HomunculusSkillSpec, HomunculusState,
        homunculus_skill_spec,
    },
    hotbar::HotbarSlot,
};
use lifthrasir_ui::{theme::ro_dark, widgets::chrome::chrome_text};
use net_contract::dto::HomunculusAction;

use super::{HomunculusTab, OwnerUi};
use crate::widgets::hotbar::HotbarDrag;

#[derive(Component, Default, Clone, Copy)]
pub(super) struct SkillRow(u32);
#[derive(Component, Default, Clone, Copy)]
pub(super) struct SkillName(u32);
#[derive(Component, Default, Clone, Copy)]
struct SkillButton {
    id: u32,
    action: SkillAction,
}
#[derive(Default, Clone, Copy)]
enum SkillAction {
    #[default]
    Cast,
    Rank,
    Learn,
}

pub(super) fn page() -> impl Scene {
    bsn! {
        HomunculusTab::Skills
        Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(4) }
        Children [ @row(8001) -- @row(8002) -- @row(8003) -- @row(8004) -- @row(8005) -- @row(8006) -- @row(8007) -- @row(8008) -- @row(8009) -- @row(8010) -- @row(8011) -- @row(8012) -- @row(8013) -- @row(8014) -- @row(8015) -- @row(8016) ]
    }
}

fn row(id: u32) -> impl Scene {
    bsn! {
        SkillRow(id)
        Node { display: Display::None, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(3) }
        Children [
            @button(id, SkillAction::Cast, "Cast") -- @button(id, SkillAction::Rank, "Rank") -- @button(id, SkillAction::Learn, "Learn")
            -- SkillName(id) @chrome_text(String::new(), 12.0, ro_dark::TITLE)
        ]
    }
}

fn button(id: u32, action: SkillAction, label: &'static str) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { @chrome_text(label.into(), 12.0, ro_dark::BUTTON_LABEL) } }
        SkillButton { id, action }
        on(activate)
        on(drag_skill)
    }
}

fn activate(
    event: On<Activate>,
    buttons: Query<&SkillButton>,
    mut ui: ResMut<OwnerUi>,
    state: Res<HomunculusState>,
    mut casts: MessageWriter<HomunculusCastIntent>,
    mut intents: MessageWriter<HomunculusIntent>,
) {
    let Ok(&SkillButton { id, action }) = buttons.get(event.entity) else {
        return;
    };
    let Some(s) = state.snapshot() else {
        return;
    };
    let Some(skill) = s.skills.iter().find(|skill| skill.skill_id == id) else {
        return;
    };
    match action {
        SkillAction::Rank if skill.level > 0 => {
            let selected = selected_rank(&ui, id, skill.level);
            ui.skill_ranks.insert(
                id,
                if selected == 1 {
                    skill.level
                } else {
                    selected - 1
                },
            );
        }
        SkillAction::Cast if skill.level > 0 => {
            casts.write(HomunculusCastIntent {
                skill_id: id,
                level: Some(selected_rank(&ui, id, skill.level)),
            });
        }
        SkillAction::Learn if state.can_mutate() && skill.learnable && s.skill_points > 0 => {
            intents.write(HomunculusIntent::Act {
                durable_id: s.durable_id,
                action: HomunculusAction::LearnSkill { skill_id: id },
            });
        }
        _ => {}
    }
}

fn selected_rank(ui: &OwnerUi, id: u32, learned: u32) -> u32 {
    ui.skill_ranks
        .get(&id)
        .copied()
        .unwrap_or(learned)
        .min(learned)
}

fn drag_skill(
    event: On<PointerDragStart>,
    buttons: Query<&SkillButton>,
    state: Res<HomunculusState>,
    mut drag: ResMut<HotbarDrag>,
) {
    let Ok(&SkillButton {
        id,
        action: SkillAction::Cast,
    }) = buttons.get(event.entity)
    else {
        return;
    };
    if matches!(
        homunculus_skill_spec(id),
        Some(HomunculusSkillSpec::SelfTarget | HomunculusSkillSpec::Enemy)
    ) && state.snapshot().is_some_and(|s| {
        s.skills
            .iter()
            .any(|skill| skill.skill_id == id && skill.level > 0)
    }) {
        drag.payload = Some(HotbarSlot::HomunculusSkill(id));
    }
}

pub(super) fn update(
    ui: Res<OwnerUi>,
    state: Res<HomunculusState>,
    time: Res<Time<Real>>,
    catalog: Option<Res<SkillCatalog>>,
    mut rows: Query<(&SkillRow, &mut Node)>,
    mut names: Query<(&SkillName, &mut Text)>,
) {
    if !ui.open || ui.tab != HomunculusTab::Skills {
        return;
    }
    let snapshot = state.snapshot();
    let info = |id| snapshot.and_then(|s| s.skills.iter().find(|skill| skill.skill_id == id));
    for (row, mut node) in &mut rows {
        node.display = if info(row.0).is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (name, mut text) in &mut names {
        let id = name.0;
        let Some(skill) = info(id) else {
            continue;
        };
        let name = catalog
            .as_ref()
            .and_then(|db| db.get(id))
            .map_or_else(|| format!("Skill #{id}"), |s| s.display_name.clone());
        let rank = selected_rank(&ui, id, skill.level);
        let cooldown = snapshot
            .and_then(|s| s.cooldowns.iter().find(|c| c.skill_id == id))
            .map_or(0, |c| state.remaining(c.remaining_ms, time.elapsed()));
        let value = if homunculus_skill_spec(id) == Some(HomunculusSkillSpec::Passive) {
            format!("{name} {}/{} (Passive)", skill.level, skill.max_level)
        } else if cooldown > 0 {
            format!(
                "{name} Lv.{rank}/{} · {}s",
                skill.level,
                cooldown.div_ceil(1000)
            )
        } else {
            format!("{name} Lv.{rank}/{}", skill.level)
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}
