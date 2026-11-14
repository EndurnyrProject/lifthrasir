//! The rebuilt read-outs of the Skills window: the details of the selected skill and
//! the footer with the point bank and the plan buttons.
//!
//! NOTE: the design also lists per-level effect values and AP. The client knows
//! neither, so the panel shows SP cost, range, target, the base cast time (before DEX,
//! INT, equipment and statuses shorten it) and the cooldown. Add the rows once the
//! catalog or the protocol carries that data.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::domain::skill::{
    Form, SkillCastLevels, SkillTreeState, Target, form, target,
};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{BUTTON_SHADOW, ButtonArt, divider, divider_diamond};

use crate::screens::shared::{
    Ring, flush_button, overline, seconds, slate_lines, slot_frame, small_button, usable,
};
use crate::widgets::info_modal::description;
use crate::widgets::info_modal::view::{ColoredLine, level_value};
use crate::widgets::skills_window::{
    CastStep, RequirementLink, SkillGateSnapshot, SkillPanelStaging, SkillPanelStepper, job_label,
    on_apply, on_cast_step, on_plan_step, on_requirement_click, on_reset, skill_name,
};

const HEADER_SLOT: f32 = 56.0;
const HEADER_ICON: f32 = 40.0;
const STEP_BUTTON: f32 = 24.0;
/// Half the panel's content width, less the gap between the two stat columns.
const STAT_WIDTH: f32 = 124.0;
const DASH: &str = "\u{2014}";

/// What the details and the footer are derived from.
pub(super) struct Sources<'a> {
    pub tree: &'a SkillTreeState,
    pub staging: &'a SkillPanelStaging,
    pub gates: &'a SkillGateSnapshot,
    pub cast_levels: &'a SkillCastLevels,
    pub catalog: Option<&'a SkillCatalog>,
    pub registry: Option<&'a JobSpriteRegistry>,
}

/// One line of the "By level" table.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LevelRow {
    pub label: String,
    pub range: String,
    pub sp: String,
    pub tone: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RequirementView {
    pub skill_id: u32,
    pub label: String,
    pub status: String,
    pub met: bool,
}

/// The level the skill is cast at, out of the learned one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CastView {
    pub level: u32,
    pub learned: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DetailView {
    pub skill_id: u32,
    pub icon: Option<String>,
    pub name: String,
    pub category: &'static str,
    pub tone: Color,
    pub level: u32,
    pub staged: u32,
    pub max: u32,
    pub locked: bool,
    pub passive: bool,
    pub description: Vec<ColoredLine>,
    pub can_raise: bool,
    pub can_lower: bool,
    /// Why the skill cannot be raised, and whether that deserves a warning tone.
    pub block: Option<(String, bool)>,
    pub rows: Vec<LevelRow>,
    pub stats: Vec<(&'static str, String)>,
    pub requires: Vec<RequirementView>,
    pub cast: Option<CastView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FooterView {
    pub points_left: u32,
    pub staged: u32,
}

/// The details of `skill_id`, or `None` when the tree does not hold it.
pub(super) fn detail_view(skill_id: u32, sources: &Sources) -> Option<DetailView> {
    let Sources {
        tree,
        staging,
        gates,
        cast_levels,
        catalog,
        registry,
    } = *sources;
    let node = tree.skills.get(&skill_id)?;
    let meta = catalog.and_then(|catalog| catalog.get(skill_id));
    let (sp_costs, ranges) = meta.map_or((&[][..], &[][..]), |meta| {
        (&meta.sp_cost[..], &meta.attack_range[..])
    });
    let staged = staging.staged(skill_id);
    let passive = form(node.inf_type) == Form::Passive;
    let (category, tone) = category(node.inf_type);
    let cast_level = cast_levels.level(skill_id, node.level);

    let stats = if passive {
        vec![("Type", "Always on".to_string())]
    } else {
        let range = level_value(ranges, cast_level)
            .map(u32::from)
            .or((node.range > 0).then_some(node.range));
        let kind = match form(node.inf_type) {
            Form::Supportive => "Supportive",
            _ => "Active",
        };
        let cast = match node.cast_time_at(cast_level) {
            0 => "Instant".to_string(),
            cast => seconds(cast),
        };
        let cooldown = match node.cooldown_at(cast_level) {
            0 => DASH.to_string(),
            cooldown => seconds(cooldown),
        };
        vec![
            ("SP", value_or_dash(level_value(sp_costs, cast_level))),
            ("Range", range.map_or(DASH.to_string(), cells)),
            ("Target", target_label(node.inf_type).to_string()),
            ("Type", kind.to_string()),
            ("Cast", cast),
            ("Cooldown", cooldown),
        ]
    };
    let requires = node
        .requires
        .iter()
        .map(|&(required, need)| {
            let have = staging.effective_level(required, tree);
            let job = tree
                .skills
                .get(&required)
                .map(|source| source.job_id)
                .filter(|&job_id| job_id != node.job_id)
                .map(|job_id| format!(" \u{b7} {}", job_label(job_id, registry)))
                .unwrap_or_default();
            let met = have >= need;
            RequirementView {
                skill_id: required,
                label: format!("{}{job} Lv {need}", skill_name(required, catalog)),
                status: if met {
                    "Met".to_string()
                } else {
                    format!("Lv {have} of {need}")
                },
                met,
            }
        })
        .collect();

    Some(DetailView {
        skill_id,
        icon: catalog.and_then(|catalog| catalog.icon_path(skill_id)),
        name: skill_name(skill_id, catalog),
        category,
        tone,
        level: node.level,
        staged,
        max: node.max_level,
        locked: node.level + staged == 0 && !staging.requirements_met(skill_id, tree),
        passive,
        description: meta
            .map(|meta| description::parse(&meta.description, &meta.display_name))
            .unwrap_or_default(),
        can_raise: gates.can_raise(skill_id, tree, staging),
        can_lower: staging.can_lower(skill_id, tree),
        block: gates.block_reason(skill_id, tree, staging),
        rows: level_rows(sp_costs, ranges, node.level, staged, node.max_level),
        stats,
        requires,
        cast: (!passive && node.level > 1).then_some(CastView {
            level: cast_level,
            learned: node.level,
        }),
    })
}

/// The overline over the skill's name and its colour.
fn category(inf: u32) -> (&'static str, Color) {
    if form(inf) == Form::Passive {
        return ("PASSIVE", ro_dark::BRONZE_LIGHT);
    }
    match target(inf) {
        Target::Enemy => ("ACTIVE \u{b7} OFFENSIVE", ro_dark::STATUS_FULL),
        Target::SelfTarget | Target::Ally => ("ACTIVE \u{b7} SUPPORTIVE", ro_dark::FROST),
        Target::Ground => ("ACTIVE \u{b7} GROUND", ro_dark::LABEL),
        Target::None => ("ACTIVE", ro_dark::LABEL),
    }
}

fn target_label(inf: u32) -> &'static str {
    match target(inf) {
        Target::Enemy => "Enemy",
        Target::Ground => "Ground",
        Target::SelfTarget => "Self",
        Target::Ally => "Ally",
        Target::None => DASH,
    }
}

fn cells(range: impl Into<u32>) -> String {
    match range.into() {
        1 => "1 cell".to_string(),
        range => format!("{range} cells"),
    }
}

fn value_or_dash(value: Option<u16>) -> String {
    value.map_or(DASH.to_string(), |value| value.to_string())
}

/// SP cost and range at the levels that matter: the learned one, the planned one, the
/// next one and the last. Empty when the catalog has neither table for the skill.
fn level_rows(sp_costs: &[u16], ranges: &[u8], level: u32, staged: u32, max: u32) -> Vec<LevelRow> {
    if sp_costs.is_empty() && ranges.is_empty() {
        return Vec::new();
    }
    let total = level + staged;
    let marks = [
        (level > 0).then_some(("Now", level, ro_dark::LABEL)),
        (staged > 0).then_some(("Plan", total, ro_dark::FROST_GLOW)),
        (total < max).then_some(("Next", total + 1, ro_dark::MUTED)),
        (total + 1 < max).then_some(("Max", max, ro_dark::BRONZE_LIGHT)),
    ];
    marks
        .into_iter()
        .flatten()
        .map(|(name, at, tone)| LevelRow {
            label: format!("{name} \u{b7} {at}"),
            range: level_value(ranges, at).map(cells).unwrap_or_default(),
            sp: level_value(sp_costs, at).map_or(DASH.to_string(), |sp| format!("{sp} SP")),
            tone,
        })
        .collect()
}

/// The content of the details panel: the selected skill, or a hint.
pub(super) fn details(view: Option<DetailView>) -> impl Scene {
    let sections = match view {
        Some(view) => sections(view),
        None => vec![Box::new(chrome_text(
            "Select a skill to see its details".into(),
            12.0,
            ro_dark::MUTED,
        )) as Box<dyn Scene>],
    };
    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
        @ignore_picking()
        Children [ {sections} ]
    }
}

fn sections(view: DetailView) -> Vec<Box<dyn Scene>> {
    let mut sections: Vec<Box<dyn Scene>> = vec![Box::new(header(&view))];
    if !view.description.is_empty() {
        sections.push(Box::new(bsn! {
            @slate_lines(view.description.clone())
            Node { margin: {UiRect::top(px(12))} }
        }));
    }
    sections.push(Box::new(plan_row(&view)));
    if let Some((reason, warn)) = view.block.clone() {
        let tone = if warn {
            ro_dark::STATUS_FULL
        } else {
            ro_dark::MUTED
        };
        sections.push(Box::new(bsn! {
            @chrome_text(reason, 11.0, tone)
            Node { margin: {UiRect::top(px(6))} }
        }));
    }
    sections.push(Box::new(bsn! {
        @divider(divider_diamond())
        Node { margin: {UiRect::new(px(0), px(0), px(14), px(10))} }
    }));
    if !view.rows.is_empty() {
        sections.push(Box::new(by_level(view.rows.clone())));
    }
    sections.push(Box::new(stats(view.stats.clone())));
    if !view.requires.is_empty() {
        sections.push(Box::new(requirements(view.requires.clone())));
    }
    if let Some(cast) = view.cast {
        sections.push(Box::new(cast_row(view.skill_id, cast)));
    }
    let hint = if view.passive {
        "Passive \u{b7} Works whenever you meet its conditions"
    } else {
        "Double-click to cast \u{b7} Drag to the hotbar"
    };
    sections.push(Box::new(bsn! {
        Node {
            margin: {UiRect::top(px(14))},
            padding: {UiRect::top(px(10))},
            border: {UiRect::top(px(1))},
        }
        BorderColor::all(ro_dark::SLOT_STROKE)
        @ignore_picking()
        Children [ @chrome_text(hint.into(), 11.0, ro_dark::MUTED) ]
    }));
    sections
}

/// The skill's slot beside its category, name and level.
fn header(view: &DetailView) -> impl Scene + use<> {
    let total = view.level + view.staged;
    let (fill, stroke) = if view.locked {
        (ro_dark::SLOT_DISABLED, ro_dark::INSET_RULE)
    } else if total >= view.max {
        (ro_dark::SLOT, ro_dark::BRONZE_LIGHT)
    } else {
        (ro_dark::SLOT, ro_dark::SLOT_STROKE)
    };
    let alpha = if view.locked { 0.5 } else { 1.0 };
    let icon = view
        .icon
        .clone()
        .map(|path| EntityScene(header_icon(path, alpha)));
    let plan = (view.staged > 0).then(|| {
        EntityScene(chrome_text(
            format!("\u{2192} {total}"),
            12.0,
            ro_dark::FROST_GLOW,
        ))
    });
    let (category, tone, name) = (view.category.to_string(), view.tone, view.name.clone());
    let level = format!("Lv {} / {}", view.level, view.max);
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(12) }
        @ignore_picking()
        Children [
            @slot_frame(HEADER_SLOT, Ring::None)
            BackgroundColor(fill)
            BorderColor::all(stroke)
            @ignore_picking()
            Children [ {icon} ]
            --
            Node { flex_direction: FlexDirection::Column, flex_shrink: 1.0, min_width: px(0) }
            @ignore_picking()
            Children [
                @overline(category, tone)
                --
                @chrome_text(name, 16.0, ro_dark::STRONG)
                --
                Node { column_gap: px(4) }
                @ignore_picking()
                Children [ @chrome_text(level, 12.0, ro_dark::MUTED) -- {plan} ]
            ]
        ]
    }
}

fn header_icon(path: String, alpha: f32) -> impl Scene {
    bsn! {
        ImageNode { image: {path}, color: {Color::WHITE.with_alpha(alpha)} }
        Node { width: px(HEADER_ICON), height: px(HEADER_ICON) }
        @ignore_picking()
    }
}

/// A square stepper button, greyed out when its step is not allowed.
fn step_button(icon: &'static str, enabled: bool) -> impl Scene {
    bsn! {
        @small_button(icon, STEP_BUTTON)
        @usable(enabled, Color::WHITE)
    }
}

/// The value between the two buttons of a stepper row.
fn step_value(text: String, tone: Color, width: f32) -> impl Scene {
    bsn! {
        @chrome_text(text, 14.0, tone)
        TextLayout { justify: Justify::Center }
        Node { min_width: px(width) }
    }
}

/// "Plan levels" with the buttons that stage and unstage a level.
fn plan_row(view: &DetailView) -> impl Scene + use<> {
    let skill_id = view.skill_id;
    let (can_lower, can_raise) = (view.can_lower, view.can_raise);
    let (value, tone) = if view.staged > 0 {
        (
            format!("{} \u{2192} {}", view.level, view.level + view.staged),
            ro_dark::FROST_GLOW,
        )
    } else {
        (view.level.to_string(), ro_dark::STRONG)
    };
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: {UiRect::top(px(14))},
        }
        @ignore_picking()
        Children [
            @chrome_text("Plan levels".into(), 12.0, ro_dark::LABEL)
            Node { flex_grow: 1.0 }
            --
            SkillPanelStepper { skill_id, raise: false }
            @step_button("minus", can_lower)
            on(on_plan_step)
            --
            @step_value(value, tone, 54.0)
            --
            SkillPanelStepper { skill_id, raise: true }
            @step_button("plus", can_raise)
            on(on_plan_step)
        ]
    }
}

fn by_level(rows: Vec<LevelRow>) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(level_row).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(1) }
        @ignore_picking()
        Children [ @overline("BY LEVEL".into(), ro_dark::ACCENT) -- {rows} ]
    }
}

fn level_row(row: LevelRow) -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Baseline,
            column_gap: px(8),
            padding: {UiRect::vertical(px(3))},
            border: {UiRect::bottom(px(1))},
        }
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
        Children [
            @chrome_text(row.label, 12.0, row.tone)
            Node { width: px(64), flex_shrink: 0.0 }
            --
            @chrome_text(row.range, 12.0, ro_dark::STRONG)
            Node { flex_grow: 1.0 }
            --
            @chrome_text(row.sp, 12.0, ro_dark::MUTED)
        ]
    }
}

/// The `(label, value)` facts, two per line.
fn stats(entries: Vec<(&'static str, String)>) -> impl Scene {
    let cells: Vec<_> = entries
        .into_iter()
        .map(|(key, value)| stat(key, value))
        .collect();
    bsn! {
        Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(16),
            row_gap: px(2),
            margin: {UiRect::top(px(12))},
        }
        @ignore_picking()
        Children [ {cells} ]
    }
}

fn stat(key: &'static str, value: String) -> impl Scene {
    bsn! {
        Node {
            width: px(STAT_WIDTH),
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(6),
        }
        @ignore_picking()
        Children [
            @chrome_text(key.into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(value, 12.0, ro_dark::STRONG)
        ]
    }
}

fn requirements(rows: Vec<RequirementView>) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(requirement).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            margin: {UiRect::top(px(14))},
        }
        @ignore_picking()
        Children [ @overline("REQUIRES".into(), ro_dark::ACCENT) -- {rows} ]
    }
}

/// A required skill and whether the plan meets it. Clicking its name opens it.
fn requirement(row: RequirementView) -> impl Scene {
    let skill_id = row.skill_id;
    let tone = if row.met {
        ro_dark::STATUS_ONLINE
    } else {
        ro_dark::STATUS_FULL
    };
    bsn! {
        Node {
            align_items: AlignItems::Baseline,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            RequirementLink(skill_id)
            @chrome_text(row.label, 12.0, ro_dark::BODY)
            Pickable { should_block_lower: true, is_hoverable: true }
            on(on_requirement_click)
            --
            @chrome_text(row.status, 11.0, tone)
        ]
    }
}

/// "Cast at": the level the skill is used at from the tree and the hotbar.
fn cast_row(skill_id: u32, cast: CastView) -> impl Scene {
    let level = format!("Lv {}", cast.level);
    let (can_lower, can_raise) = (cast.level > 1, cast.level < cast.learned);
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(8),
            margin: {UiRect::top(px(14))},
        }
        @ignore_picking()
        Children [
            Node { flex_direction: FlexDirection::Column, flex_grow: 1.0 }
            @ignore_picking()
            Children [
                @overline("CAST AT".into(), ro_dark::ACCENT)
                --
                @chrome_text("Hotbar uses this level".into(), 11.0, ro_dark::MUTED)
            ]
            --
            CastStep { skill_id, up: false }
            @step_button("minus", can_lower)
            on(on_cast_step)
            --
            @step_value(level, ro_dark::STRONG, 40.0)
            --
            CastStep { skill_id, up: true }
            @step_button("plus", can_raise)
            on(on_cast_step)
        ]
    }
}

/// The point bank, the planned-levels tag and the plan buttons. Button widths include
/// the art's baked shadow, which `flush_button` cancels with a negative margin.
pub(super) fn footer(view: FooterView) -> impl Scene {
    let enabled = view.staged > 0;
    let planned = enabled.then(|| EntityScene(planned_tag(view.staged)));
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(12) }
        @ignore_picking()
        Children [
            @chrome_text("Skill Points".into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(view.points_left.to_string(), 18.0, ro_dark::BRONZE_LIGHT)
            --
            {planned}
            --
            @chrome_text(
                "Double-click a skill to cast \u{b7} Drag it to the hotbar \u{b7} + and \u{2212} plan levels".into(),
                11.0,
                ro_dark::MUTED,
            )
            TextLayout { justify: Justify::Center }
            Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) }
            --
            @flush_button(ButtonArt::NORMAL, "Reset Plan")
            Node { width: px(104.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            @usable(enabled, Color::WHITE)
            on(on_reset)
            --
            @flush_button(ButtonArt::PRIMARY, "Apply")
            Node { width: px(96.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0, flex_shrink: 0.0 }
            @usable(enabled, Color::WHITE)
            on(on_apply)
        ]
    }
}

fn planned_tag(staged: u32) -> impl Scene {
    bsn! {
        Node {
            height: px(18),
            padding: {UiRect::horizontal(px(7))},
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor(ro_dark::PRIMARY)
        @ignore_picking()
        Children [ @chrome_text(format!("+{staged} planned"), 11.0, ro_dark::CHECK_MARK) ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_rows_mark_the_learned_planned_next_and_last_levels() {
        let sp = [10, 12, 14, 16, 18];
        let rows = level_rows(&sp, &[], 1, 1, 5);
        let read: Vec<_> = rows
            .iter()
            .map(|row| (row.label.as_str(), row.sp.as_str()))
            .collect();
        assert_eq!(
            read,
            [
                ("Now \u{b7} 1", "10 SP"),
                ("Plan \u{b7} 2", "12 SP"),
                ("Next \u{b7} 3", "14 SP"),
                ("Max \u{b7} 5", "18 SP"),
            ]
        );

        let mastered = level_rows(&sp, &[], 5, 0, 5);
        assert_eq!(mastered.len(), 1);
        assert!(level_rows(&[], &[], 1, 0, 5).is_empty());
    }
}
