//! Edits an entire server-owned tactics configuration, including untouched rules.
use bevy::{prelude::*, text::EditableText, ui_widgets::Activate};
use bevy_feathers::controls::{FeathersButton, FeathersTextInput};
use lifthrasir_client::domain::homunculus::{
    HomunculusIntent, HomunculusSkillSpec, HomunculusState, homunculus_skill_spec,
};
use lifthrasir_ui::{theme::ro_dark, widgets::chrome::chrome_text};
use net_contract::dto::{
    HomunculusAction, HomunculusAiConfig, HomunculusAiSkillConfig, HomunculusAiSkillMode as Mode,
    HomunculusAiStance as Stance, HomunculusHpRange,
};

use super::{HomunculusTab, OwnerUi};

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    #[default]
    Stance,
    Leash,
    Join,
    Retaliate,
    AvoidBoss,
    AutoFeed,
    FeedAt,
    Allow,
    Deny,
    SpReserve,
    Mode,
    Priority,
    SelfHp,
    OwnerHp,
    TargetHp,
}
#[derive(Component, Default, Clone)]
pub(super) struct TacticsStatus;
#[derive(Component, Default, Clone, Copy)]
pub(super) enum Group {
    #[default]
    Advanced,
    Skill,
    TargetHp,
}

#[derive(Default)]
pub(super) struct Editor {
    companion: u64,
    base: Option<HomunculusAiConfig>,
    draft: Option<HomunculusAiConfig>,
    submitted: Option<HomunculusAiConfig>,
    sync: bool,
    advanced: bool,
    row: usize,
    error: Option<&'static str>,
}

impl Editor {
    fn reset(&mut self, id: u64, config: &HomunculusAiConfig) {
        self.companion = id;
        self.base = Some(config.clone());
        self.draft = Some(config.clone());
        self.submitted = None;
        self.row = 0;
        self.sync = true;
        self.error = None;
    }
    fn stale(&self, config: &HomunculusAiConfig) -> bool {
        self.base.as_ref() != Some(config)
    }
}

pub(super) fn page() -> impl Scene {
    bsn! {
        HomunculusTab::Tactics
        Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(3) }
        Children [
            @field(Field::Stance, "Stance (passive / assist / aggressive)")
            -- @field(Field::Leash, "Leash (2–14)")
            -- @field(Field::Join, "Join your target (yes/no)")
            -- @field(Field::Retaliate, "Retaliate (yes/no)")
            -- @field(Field::AvoidBoss, "Avoid bosses (yes/no)")
            -- @field(Field::AutoFeed, "Auto-feed (yes/no)")
            -- @field(Field::FeedAt, "Feed at hunger (1–75)")
            -- @advanced()
            -- Node { flex_direction: FlexDirection::Row, column_gap: px(4) }
               Children [ @action("Advanced", Control::Advanced) -- @action("Reset draft", Control::Reset) -- @action("Apply", Control::Apply) ]
            -- TacticsStatus @chrome_text(String::new(), 12.0, ro_dark::TITLE)
        ]
    }
}

fn advanced() -> impl Scene {
    bsn! {
        Group::Advanced Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(3) }
        Children [
            @field(Field::Allow, "Allowed mob IDs (comma-separated)")
            -- @field(Field::Deny, "Denied mob IDs (comma-separated)")
            -- @field(Field::SpReserve, "Auto-cast SP reserve (0–100%)")
            -- @action("Next skill rule", Control::Next)
            -- Group::Skill Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(3) }
               Children [
                   @field(Field::Mode, "Skill mode (manual / auto)")
                   -- @field(Field::Priority, "Priority (1–100)")
                   -- @field(Field::SelfHp, "Companion HP at or below (%)")
                   -- @field(Field::OwnerHp, "Owner HP at or below (%)")
                   -- Group::TargetHp Node { display: Display::None, flex_direction: FlexDirection::Column }
                      Children [ @field(Field::TargetHp, "Target HP range (min-max)") ]
               ]
        ]
    }
}

fn field(field: Field, label: &'static str) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, column_gap: px(4) }
        Children [ @chrome_text(label.into(), 12.0, ro_dark::TITLE)
            -- @FeathersTextInput { @visible_width: {Some(140.0)} } field ]
    }
}

#[derive(Component, Default, Clone, Copy)]
enum Control {
    #[default]
    Apply,
    Reset,
    Advanced,
    Next,
}

fn action(label: &'static str, control: Control) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { @chrome_text(label.into(), 12.0, ro_dark::BUTTON_LABEL) } }
        control on(activate)
    }
}

fn read_field(fields: &Query<(&Field, &EditableText)>, field: Field) -> String {
    fields
        .iter()
        .find(|(kind, _)| **kind == field)
        .map_or_else(String::new, |(_, value)| value.value().to_string())
}

fn read_draft(
    editor: &Editor,
    fields: &Query<(&Field, &EditableText)>,
) -> Option<HomunculusAiConfig> {
    let mut result = editor.draft.clone()?;
    let field = |kind| read_field(fields, kind);
    result.stance = match field(Field::Stance).trim().to_ascii_lowercase().as_str() {
        "passive" => Stance::Passive,
        "assist" => Stance::Assist,
        "aggressive" => Stance::Aggressive,
        _ => return None,
    };
    result.leash_distance = number(&field(Field::Leash), 2..=14)?;
    result.join_owner_target = boolean(&field(Field::Join))?;
    result.retaliate = boolean(&field(Field::Retaliate))?;
    result.avoid_bosses = boolean(&field(Field::AvoidBoss))?;
    result.auto_feed = boolean(&field(Field::AutoFeed))?;
    result.auto_feed_threshold = number(&field(Field::FeedAt), 1..=75)?;
    result.auto_cast_sp_reserve_percent = number(&field(Field::SpReserve), 0..=100)?;
    result.allowed_mob_class_ids = ids(&field(Field::Allow))?;
    result.denied_mob_class_ids = ids(&field(Field::Deny))?;
    if let Some(skill) = result.skills.get_mut(editor.row)
        && editable_skill(skill)
    {
        skill.mode = match field(Field::Mode).trim().to_ascii_lowercase().as_str() {
            "manual" => Mode::Manual,
            "auto" => Mode::Auto,
            _ => return None,
        };
        skill.priority = number(&field(Field::Priority), 1..=100)?;
        if skill.mode == Mode::Manual {
            skill.self_hp_threshold = None;
            skill.owner_hp_threshold = None;
            skill.target_hp_range = None;
        } else {
            skill.self_hp_threshold = optional_hp(&field(Field::SelfHp))?;
            skill.owner_hp_threshold = optional_hp(&field(Field::OwnerHp))?;
            if homunculus_skill_spec(skill.skill_id) == Some(HomunculusSkillSpec::Enemy) {
                let range = field(Field::TargetHp);
                skill.target_hp_range = if range.trim().is_empty() {
                    None
                } else {
                    let (min, max) = range.split_once('-')?;
                    let min_percent = number(min, 0..=100)?;
                    let max_percent = number(max, min_percent..=100)?;
                    Some(HomunculusHpRange {
                        min_percent,
                        max_percent,
                    })
                };
            }
        }
    }
    Some(result)
}

fn number(s: &str, range: std::ops::RangeInclusive<u32>) -> Option<u32> {
    s.trim().parse::<u32>().ok().filter(|n| range.contains(n))
}
fn boolean(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}
fn optional_hp(s: &str) -> Option<Option<u32>> {
    if s.trim().is_empty() {
        Some(None)
    } else {
        number(s, 1..=100).map(Some)
    }
}
fn ids(s: &str) -> Option<Vec<u32>> {
    if s.trim().is_empty() {
        return Some(Vec::new());
    }
    let mut ids = s
        .split(',')
        .map(|id| number(id, 1..=u32::MAX))
        .collect::<Option<Vec<_>>>()?;
    ids.sort_unstable();
    ids.windows(2).all(|pair| pair[0] != pair[1]).then_some(ids)
}
fn editable_skill(skill: &HomunculusAiSkillConfig) -> bool {
    matches!(
        homunculus_skill_spec(skill.skill_id),
        Some(HomunculusSkillSpec::Enemy | HomunculusSkillSpec::SelfTarget)
    )
}
fn known(config: &HomunculusAiConfig) -> bool {
    matches!(
        config.stance,
        Stance::Passive | Stance::Assist | Stance::Aggressive
    ) && config.skills.iter().all(|s| {
        homunculus_skill_spec(s.skill_id).is_some() && matches!(s.mode, Mode::Manual | Mode::Auto)
    })
}

fn activate(
    event: On<Activate>,
    buttons: Query<&Control>,
    fields: Query<(&Field, &EditableText)>,
    state: Res<HomunculusState>,
    mut ui: ResMut<OwnerUi>,
    mut out: MessageWriter<HomunculusIntent>,
) {
    let Ok(&button) = buttons.get(event.entity) else {
        return;
    };
    let Some((id, config)) = state
        .snapshot()
        .and_then(|s| Some((s.durable_id, s.ai_config.as_ref()?)))
    else {
        return;
    };
    if !ui.open || ui.tab != HomunculusTab::Tactics {
        return;
    }
    let editor = &mut ui.tactics;
    match button {
        Control::Advanced => {
            editor.advanced = !editor.advanced;
            return;
        }
        Control::Reset => {
            editor.reset(id, config);
            return;
        }
        _ => {}
    }
    if editor.companion != id || !known(config) || editor.stale(config) || editor.sync {
        return;
    }
    let Some(draft) = read_draft(editor, &fields) else {
        editor.error = Some("Check the field values. Leave unused HP thresholds blank.");
        return;
    };
    editor.error = None;
    if matches!(button, Control::Next) {
        editor.row = (editor.row + 1) % draft.skills.len().max(1);
        editor.draft = Some(draft);
        editor.sync = true;
    } else if state.can_mutate() && draft != *config {
        editor.submitted = Some(draft.clone());
        editor.draft = Some(draft.clone());
        out.write(HomunculusIntent::Act {
            durable_id: id,
            action: HomunculusAction::ReplaceAi { config: draft },
        });
    }
}

pub(super) fn update(
    mut ui: ResMut<OwnerUi>,
    state: Res<HomunculusState>,
    mut fields: Query<(&Field, &mut EditableText)>,
    mut status: Query<&mut Text, With<TacticsStatus>>,
    mut groups: Query<(&Group, &mut Node)>,
) {
    if !ui.open {
        ui.tactics = Editor::default();
        return;
    }
    if ui.tab != HomunculusTab::Tactics {
        return;
    }
    let editor = &mut ui.tactics;
    let mut detail = "Tactics are unavailable without a companion.".to_owned();
    if let Some((id, config)) = state
        .snapshot()
        .and_then(|s| Some((s.durable_id, s.ai_config.as_ref()?)))
    {
        if editor.companion != id
            || editor.base.is_none()
            || editor.submitted.as_ref() == Some(config)
        {
            editor.reset(id, config);
        }
        let current = editor
            .draft
            .as_ref()
            .and_then(|draft| draft.skills.get(editor.row));
        for (&group, mut node) in &mut groups {
            let shown = match group {
                Group::Advanced => editor.advanced,
                Group::Skill => current.is_some_and(editable_skill),
                Group::TargetHp => current.is_some_and(|s| {
                    homunculus_skill_spec(s.skill_id) == Some(HomunculusSkillSpec::Enemy)
                }),
            };
            node.display = if shown { Display::Flex } else { Display::None };
        }
        if editor.sync
            && let Some(draft) = &editor.draft
        {
            for (&field, mut input) in &mut fields {
                input
                    .editor_mut()
                    .set_text(&field_text(draft, current, field));
            }
            editor.sync = false;
        }
        detail = if !known(config) {
            "This configuration contains rules the client cannot edit.".into()
        } else if editor.stale(config) {
            "Tactics changed on the server. Reset the draft to reload.".into()
        } else if let Some(error) = editor.error {
            error.into()
        } else if state.pending_mutation().is_some() {
            "Waiting for the server…".into()
        } else if editor.submitted.is_some()
            && let Some(super::HomunculusFeedbackKind::Rejected(error)) = state.last_feedback()
        {
            super::error_text(*error).into()
        } else if let Some(skill) = current {
            format!(
                "Skill #{} · rule {}/{}",
                skill.skill_id,
                editor.row + 1,
                config.skills.len()
            )
        } else {
            "No learned skills have tactics rules.".into()
        };
    } else {
        *editor = Editor::default();
        for (_, mut node) in &mut groups {
            node.display = Display::None;
        }
    }
    if let Ok(mut text) = status.single_mut()
        && text.0 != detail
    {
        text.0 = detail;
    }
}

fn field_text(
    cfg: &HomunculusAiConfig,
    skill: Option<&HomunculusAiSkillConfig>,
    field: Field,
) -> String {
    let boolean = |yes| {
        if yes {
            "yes".to_string()
        } else {
            "no".to_string()
        }
    };
    let ids = |values: &[u32]| {
        values
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    match field {
        Field::Stance => format!("{:?}", cfg.stance).to_lowercase(),
        Field::Leash => cfg.leash_distance.to_string(),
        Field::Join => boolean(cfg.join_owner_target),
        Field::Retaliate => boolean(cfg.retaliate),
        Field::AvoidBoss => boolean(cfg.avoid_bosses),
        Field::AutoFeed => boolean(cfg.auto_feed),
        Field::FeedAt => cfg.auto_feed_threshold.to_string(),
        Field::Allow => ids(&cfg.allowed_mob_class_ids),
        Field::Deny => ids(&cfg.denied_mob_class_ids),
        Field::SpReserve => cfg.auto_cast_sp_reserve_percent.to_string(),
        Field::Mode => skill.map_or_else(String::new, |s| format!("{:?}", s.mode).to_lowercase()),
        Field::Priority => skill.map_or_else(String::new, |s| s.priority.to_string()),
        Field::SelfHp => skill
            .and_then(|s| s.self_hp_threshold)
            .map_or_else(String::new, |n| n.to_string()),
        Field::OwnerHp => skill
            .and_then(|s| s.owner_hp_threshold)
            .map_or_else(String::new, |n| n.to_string()),
        Field::TargetHp => skill
            .and_then(|s| s.target_hp_range)
            .map_or_else(String::new, |r| {
                format!("{}-{}", r.min_percent, r.max_percent)
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tactics_blocks_apply_after_skill_rows_change() {
        let original = HomunculusAiConfig {
            stance: Stance::Assist,
            ..default()
        };
        let mut editor = Editor::default();
        editor.reset(1, &original);
        let mut incoming = original.clone();
        incoming.skills.push(HomunculusAiSkillConfig {
            skill_id: 8001,
            mode: Mode::Manual,
            ..default()
        });
        assert!(!editor.stale(&original));
        assert!(editor.stale(&incoming));
    }

    #[test]
    fn stance_only_edit_preserves_hidden_ai_fields() {
        use bevy::ecs::system::RunSystemOnce;
        let original = HomunculusAiConfig {
            stance: Stance::Assist,
            leash_distance: 8,
            auto_feed_threshold: 25,
            allowed_mob_class_ids: vec![1002],
            denied_mob_class_ids: vec![1004],
            auto_cast_sp_reserve_percent: 12,
            skills: vec![
                HomunculusAiSkillConfig {
                    skill_id: 8009,
                    mode: Mode::Auto,
                    priority: 7,
                    target_hp_range: Some(HomunculusHpRange {
                        min_percent: 0,
                        max_percent: 45,
                    }),
                    ..default()
                },
                HomunculusAiSkillConfig {
                    skill_id: 8012,
                    mode: Mode::Auto,
                    priority: 4,
                    owner_hp_threshold: Some(25),
                    ..default()
                },
            ],
            ..default()
        };
        let mut world = World::new();
        for field in [
            Field::Stance,
            Field::Leash,
            Field::Join,
            Field::Retaliate,
            Field::AvoidBoss,
            Field::AutoFeed,
            Field::FeedAt,
            Field::Allow,
            Field::Deny,
            Field::SpReserve,
            Field::Mode,
            Field::Priority,
            Field::SelfHp,
            Field::OwnerHp,
            Field::TargetHp,
        ] {
            let value = if field == Field::Stance {
                "passive".into()
            } else {
                field_text(&original, original.skills.first(), field)
            };
            world.spawn((field, EditableText::new(value)));
        }
        let mut editor = Editor::default();
        editor.reset(1, &original);
        let result = world
            .run_system_once(move |fields: Query<(&Field, &EditableText)>| {
                read_draft(&editor, &fields)
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            HomunculusAiConfig {
                stance: Stance::Passive,
                ..original
            }
        );
    }

    #[test]
    fn validator_keeps_threshold_presence_and_rejects_duplicate_ids() {
        assert_eq!(optional_hp(""), Some(None));
        assert_eq!(optional_hp("0"), None);
        assert_eq!(ids("3, 1"), Some(vec![1, 3]));
        assert_eq!(ids("3, 3"), None);
    }
}
