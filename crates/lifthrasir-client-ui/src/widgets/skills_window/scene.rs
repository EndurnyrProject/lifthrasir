//! The job tabs and the tree canvas of the Skills window. [`body`] is respawned when
//! the open job, the filter or the tree's shape changes; the `paint_*` systems keep
//! levels, rings, lines and steppers current on the spawned nodes from
//! [`SkillLooks`]. Sizes follow the `Skills` artboard of the Lifthrasir Widgets design.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{inset, round_button, segment, tab, tab_text};

use crate::screens::shared::{Ring, glow, shown_if, slot_frame};
use crate::widgets::skills_window::layout::{Link, Placement};
use crate::widgets::skills_window::looks::{Dim, Edge, Look, SkillLooks};
use crate::widgets::skills_window::{
    SkillFilter, SkillFilterButton, SkillJobTab, SkillPanelCell, SkillPanelStepper, on_cell_click,
    on_cell_drag_start, on_stepper, pick_filter, pick_job,
};

const CANVAS_HEIGHT: f32 = 492.0;
/// Height the filter row keeps clear above the scrolling tree.
const TOOLBAR_HEIGHT: f32 = 44.0;
const PAD_X: f32 = 22.0;
const PAD_Y: f32 = 8.0;
const PITCH_X: f32 = 100.0;
const PITCH_Y: f32 = 112.0;
const NODE: f32 = 92.0;
const SLOT: f32 = 50.0;
const SLOT_LEFT: f32 = (NODE - SLOT) / 2.0;
const ICON: f32 = 36.0;
const DASH: f32 = 3.0;

/// The tree's scroll viewport.
#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillCanvasViewport;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeFrame(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeIcon(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeLevel(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeName(pub u32);

/// The wash over a skill that is out of focus.
#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeDimmer(pub u32);

/// One level of a skill's level meter, counted from 0.
#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillPip {
    pub skill_id: u32,
    pub index: u32,
}

/// The solid stroke of one requirement line segment. It hides to show the dashes
/// under it while the requirement is unmet.
#[derive(Component, Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct SkillLine {
    pub source: u32,
    pub target: u32,
    pub minimum_level: u32,
}

/// The mark of a job tab that holds planned levels.
#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillTabDot(pub u32);

pub(super) struct TabView {
    pub job_id: u32,
    pub label: String,
    pub active: bool,
}

pub(super) struct NodeView {
    pub placement: Placement,
    pub icon: Option<String>,
    pub name: String,
    pub max_level: u32,
}

/// Everything [`body`] draws. `scroll` is the offset the new tree starts at.
pub(super) struct TreeView {
    pub tabs: Vec<TabView>,
    pub filter: SkillFilter,
    pub nodes: Vec<NodeView>,
    pub links: Vec<Link>,
    pub rows: u32,
    pub scroll: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bar {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

pub(super) fn body(view: TreeView) -> impl Scene {
    let TreeView {
        tabs,
        filter,
        nodes,
        links,
        rows,
        scroll,
    } = view;
    let tabs: Vec<_> = tabs.into_iter().map(tab_button).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [
            Node { column_gap: px(4) }
            @ignore_picking()
            Children [ {tabs} ]
            --
            @canvas(filter, nodes, links, rows, scroll)
        ]
    }
}

fn tab_button(view: TabView) -> impl Scene {
    let (job_id, active) = (view.job_id, view.active);
    let (label, _) = tab_text(active);
    bsn! {
        SkillJobTab(job_id)
        @tab(active)
        Node { overflow: {Overflow::clip()} }
        on(pick_job)
        Children [
            @chrome_text(view.label, 13.0, label)
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            SkillTabDot(job_id)
            Node {
                width: px(6),
                height: px(6),
                flex_shrink: 0.0,
                border_radius: BorderRadius::MAX,
                display: Display::None,
            }
            BackgroundColor(ro_dark::FROST_GLOW)
            @ignore_picking()
        ]
    }
}

/// The panel under the tabs: the filter row pinned over a vertically scrolling tree.
fn canvas(
    filter: SkillFilter,
    nodes: Vec<NodeView>,
    links: Vec<Link>,
    rows: u32,
    scroll: Vec2,
) -> impl Scene {
    let cells: HashMap<u32, (f32, f32)> = nodes
        .iter()
        .map(|node| (node.placement.skill_id, origin(node.placement)))
        .collect();
    let mut dashes = Vec::new();
    let mut lines = Vec::new();
    for link in links {
        for bar in link_bars(cells[&link.source], cells[&link.target]) {
            dashes.extend(dash_bars(bar).into_iter().map(dash));
            lines.push(line(link, bar));
        }
    }
    let empty = nodes.is_empty().then(|| EntityScene(empty_message()));
    let nodes: Vec<_> = nodes.into_iter().map(skill_node).collect();
    let height = PAD_Y + rows.saturating_sub(1) as f32 * PITCH_Y + NODE + PAD_Y;
    bsn! {
        @inset()
        Node {
            height: px(CANVAS_HEIGHT),
            position_type: PositionType::Relative,
            border_radius: {BorderRadius::all(px(6)).with_top_left(px(0))},
        }
        @ignore_picking()
        Children [
            #tree
            SkillCanvasViewport
            ScrollArea
            ScrollPosition(scroll)
            Pickable
            Node {
                position_type: PositionType::Absolute,
                left: px(0), top: px(TOOLBAR_HEIGHT), right: px(0), bottom: px(0),
                overflow: {Overflow::scroll_y()},
            }
            Children [
                Node {
                    position_type: PositionType::Relative,
                    width: percent(100),
                    height: px(height),
                    flex_shrink: 0.0,
                }
                @ignore_picking()
                Children [ {dashes} -- {lines} -- {nodes} -- {empty} ]
            ]
            --
            @FeathersScrollbar { @target: #tree, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(6),
                top: px(TOOLBAR_HEIGHT + 4.0),
                bottom: px(8),
                width: px(8),
            }
            --
            @toolbar(filter)
        ]
    }
}

fn empty_message() -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0), right: px(0), top: px(160),
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [ @chrome_text("No skills.".into(), 13.0, ro_dark::MUTED) ]
    }
}

/// The filter buttons and the line legend.
fn toolbar(filter: SkillFilter) -> impl Scene {
    let segments: Vec<_> = SkillFilter::ALL
        .into_iter()
        .map(|option| filter_button(option, option == filter))
        .collect();
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(14), top: px(12), right: px(14),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        @ignore_picking()
        Children [
            Node { column_gap: px(4) }
            @ignore_picking()
            Children [ {segments} ]
            --
            Node { column_gap: px(14) }
            @ignore_picking()
            Children [
                @legend("Unlocked", bsn! { @swatch(14.0, ro_dark::DIVIDER) })
                --
                @legend("Locked", bsn! {
                    Node { column_gap: px(DASH) }
                    @ignore_picking()
                    Children [
                        @swatch(DASH, ro_dark::MUTED_BRONZE)
                        --
                        @swatch(DASH, ro_dark::MUTED_BRONZE)
                        --
                        @swatch(DASH, ro_dark::MUTED_BRONZE)
                    ]
                })
                --
                @legend("Path", bsn! { @swatch(14.0, ro_dark::FROST_GLOW) })
            ]
        ]
    }
}

fn filter_button(filter: SkillFilter, active: bool) -> impl Scene {
    let (label, _) = tab_text(active);
    bsn! {
        SkillFilterButton(filter)
        @segment(active)
        on(pick_filter)
        Children [ @chrome_text(filter.label().into(), 12.0, label) ]
    }
}

fn legend(label: &'static str, mark: impl Scene) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(6) }
        @ignore_picking()
        Children [ @mark -- @chrome_text(label.into(), 11.0, ro_dark::MUTED) ]
    }
}

fn swatch(width: f32, color: Color) -> impl Scene {
    bsn! {
        Node { width: px(width), height: px(2) }
        BackgroundColor(color)
        @ignore_picking()
    }
}

/// Top-left corner of a skill's cell inside the tree.
fn origin(placement: Placement) -> (f32, f32) {
    (
        PAD_X + placement.column as f32 * PITCH_X,
        PAD_Y + placement.row as f32 * PITCH_Y,
    )
}

/// The segments of the line from under the cell at `source` to the top of the one at
/// `target`: straight down when they share a column, otherwise an elbow that turns
/// 10px under the source.
fn link_bars(source: (f32, f32), target: (f32, f32)) -> Vec<Bar> {
    let (from_x, from_y) = (source.0 + NODE / 2.0, source.1 + NODE);
    let (to_x, to_y) = (target.0 + NODE / 2.0, target.1);
    if from_x == to_x {
        return vec![Bar {
            x: from_x - 1.0,
            y: from_y - 2.0,
            width: 2.0,
            height: to_y - from_y + 2.0,
        }];
    }
    let turn = from_y + 10.0;
    vec![
        Bar {
            x: from_x - 1.0,
            y: from_y - 2.0,
            width: 2.0,
            height: turn - from_y + 3.0,
        },
        Bar {
            x: from_x.min(to_x) - 1.0,
            y: turn - 1.0,
            width: (to_x - from_x).abs() + 2.0,
            height: 2.0,
        },
        Bar {
            x: to_x - 1.0,
            y: turn,
            width: 2.0,
            height: to_y - turn,
        },
    ]
}

/// `bar` cut along its long side into [`DASH`]-long pieces with gaps of the same size.
fn dash_bars(bar: Bar) -> Vec<Bar> {
    let horizontal = bar.width > bar.height;
    let length = if horizontal { bar.width } else { bar.height };
    let count = (length / (2.0 * DASH)).ceil() as u32;
    (0..count)
        .map(|index| {
            let at = index as f32 * 2.0 * DASH;
            let piece = DASH.min(length - at);
            if horizontal {
                Bar {
                    x: bar.x + at,
                    width: piece,
                    ..bar
                }
            } else {
                Bar {
                    y: bar.y + at,
                    height: piece,
                    ..bar
                }
            }
        })
        .collect()
}

fn bar_node(bar: Bar) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(bar.x),
            top: px(bar.y),
            width: px(bar.width),
            height: px(bar.height),
        }
        @ignore_picking()
    }
}

fn dash(bar: Bar) -> impl Scene {
    bsn! {
        @bar_node(bar)
        BackgroundColor(ro_dark::MUTED_BRONZE)
    }
}

fn line(link: Link, bar: Bar) -> impl Scene {
    let marker = SkillLine {
        source: link.source,
        target: link.target,
        minimum_level: link.minimum_level,
    };
    let halo = glow(Color::NONE);
    bsn! {
        marker
        @bar_node(bar)
        BackgroundColor(ro_dark::DIVIDER)
        halo
        Visibility::Hidden
    }
}

/// One skill of the tree: its slot with the icon and level, the plan steppers on the
/// slot's top corners, the level meter and the name.
fn skill_node(view: NodeView) -> impl Scene {
    let skill_id = view.placement.skill_id;
    let (left, top) = origin(view.placement);
    let icon = view
        .icon
        .map(|path| EntityScene(skill_icon(skill_id, path)));
    let pips: Vec<_> = (0..view.max_level)
        .map(|index| pip(skill_id, index))
        .collect();
    bsn! {
        SkillPanelCell(skill_id)
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(NODE),
            height: px(NODE),
        }
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        Children [
            SkillNodeFrame(skill_id)
            @slot_frame(SLOT, Ring::None)
            Node { position_type: PositionType::Absolute, left: px(SLOT_LEFT), top: px(0) }
            @ignore_picking()
            Children [
                {icon}
                --
                SkillNodeLevel(skill_id)
                @chrome_text(String::new(), 10.0, ro_dark::VALUE)
                TextShadow { offset: Vec2::splat(1.0), color: Color::BLACK }
                Node { position_type: PositionType::Absolute, right: px(3), bottom: px(2) }
            ]
            --
            @stepper(skill_id, false)
            --
            @stepper(skill_id, true)
            --
            Node {
                position_type: PositionType::Absolute,
                left: px(SLOT_LEFT),
                top: px(SLOT + 4.0),
                width: px(SLOT),
                height: px(4),
                column_gap: px(1),
            }
            @ignore_picking()
            Children [ {pips} ]
            --
            SkillNodeName(skill_id)
            @chrome_text(view.name, 11.0, ro_dark::LABEL)
            TextLayout { justify: Justify::Center }
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(SLOT + 12.0),
                max_height: px(28),
                overflow: {Overflow::clip()},
            }
            --
            SkillNodeDimmer(skill_id)
            Node {
                position_type: PositionType::Absolute,
                left: px(0), top: px(0), right: px(0), bottom: px(0),
            }
            BackgroundColor(Color::NONE)
            Visibility::Hidden
            @ignore_picking()
        ]
    }
}

fn skill_icon(skill_id: u32, path: String) -> impl Scene {
    bsn! {
        SkillNodeIcon(skill_id)
        ImageNode { image: {path} }
        Node { width: px(ICON), height: px(ICON) }
        @ignore_picking()
    }
}

fn pip(skill_id: u32, index: u32) -> impl Scene {
    bsn! {
        SkillPip { skill_id, index }
        Node { flex_grow: 1.0, flex_basis: px(0), height: px(4), border: px(1) }
        BackgroundColor(ro_dark::BAR_TROUGH)
        BorderColor::all(ro_dark::INSET_RULE)
        @ignore_picking()
    }
}

/// A round plan button on a slot corner. It shows only while its step is allowed.
fn stepper(skill_id: u32, raise: bool) -> impl Scene {
    let (icon, left) = if raise {
        ("plus", SLOT_LEFT + SLOT - 9.0)
    } else {
        ("minus", SLOT_LEFT - 9.0)
    };
    bsn! {
        SkillPanelStepper { skill_id, raise }
        @round_button(icon, raise)
        Node { position_type: PositionType::Absolute, left: px(left), top: px(-7) }
        Visibility::Hidden
        on(on_stepper)
    }
}

/// Fill, stroke, stroke width and halo of a slot.
fn frame_style(look: &Look) -> (Color, Color, f32, BoxShadow) {
    let fill = match (look.locked, look.edge) {
        (true, _) => ro_dark::SLOT_DISABLED,
        (false, Edge::Hover) => ro_dark::SLOT_HOVER,
        (false, _) => ro_dark::SLOT,
    };
    let none = glow(Color::NONE);
    let (stroke, width, halo) = match look.edge {
        Edge::Selected => Ring::Selected.style(),
        Edge::Chain => (ro_dark::FROST_GLOW, 1.0, none),
        Edge::Max => (ro_dark::BRONZE_LIGHT, 1.0, none),
        Edge::Hover => (ro_dark::BUTTON_STROKE, 1.0, none),
        Edge::Rest if look.locked => (ro_dark::INSET_RULE, 1.0, none),
        Edge::Rest => (ro_dark::SLOT_STROKE, 1.0, none),
    };
    (fill, stroke, width, halo)
}

/// The slot's level badge: `MAX` once mastered with nothing planned, else `3/10`.
pub(super) fn level_badge(look: &Look) -> (String, Color) {
    let (total, maxed) = (look.total(), look.total() >= look.max);
    let text = if maxed && look.staged == 0 {
        "MAX".to_string()
    } else {
        format!("{total}/{}", look.max)
    };
    let color = if look.staged > 0 {
        ro_dark::FROST_GLOW
    } else if maxed {
        ro_dark::BRONZE_LIGHT
    } else if look.locked {
        ro_dark::FAINT
    } else {
        ro_dark::VALUE
    };
    (text, color)
}

pub(super) fn paint_frames(
    looks: Res<SkillLooks>,
    mut frames: Query<(
        &SkillNodeFrame,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut BoxShadow,
    )>,
) {
    for (frame, mut node, mut fill, mut stroke, mut halo) in &mut frames {
        let Some(look) = looks.skills.get(&frame.0) else {
            continue;
        };
        let (background, border, width, shadow) = frame_style(look);
        fill.set_if_neq(BackgroundColor(background));
        stroke.set_if_neq(BorderColor::all(border));
        halo.set_if_neq(shadow);
        let border = UiRect::all(px(width));
        if node.border != border {
            node.border = border;
        }
    }
}

pub(super) fn paint_labels(
    looks: Res<SkillLooks>,
    mut icons: Query<(&SkillNodeIcon, &mut ImageNode)>,
    mut levels: Query<(&SkillNodeLevel, &mut Text, &mut TextColor), Without<SkillNodeName>>,
    mut names: Query<(&SkillNodeName, &mut TextColor), Without<SkillNodeLevel>>,
) {
    for (icon, mut image) in &mut icons {
        let Some(look) = looks.skills.get(&icon.0) else {
            continue;
        };
        let color = Color::WHITE.with_alpha(if look.locked { 0.5 } else { 1.0 });
        if image.color != color {
            image.color = color;
        }
    }
    for (level, mut text, mut color) in &mut levels {
        let Some(look) = looks.skills.get(&level.0) else {
            continue;
        };
        let (badge, tone) = level_badge(look);
        if text.0 != badge {
            text.0 = badge;
        }
        color.set_if_neq(TextColor(tone));
    }
    for (name, mut color) in &mut names {
        let Some(look) = looks.skills.get(&name.0) else {
            continue;
        };
        let tone = match (look.locked, look.edge) {
            (true, _) => ro_dark::FAINT,
            (false, Edge::Selected) => ro_dark::STRONG,
            (false, _) => ro_dark::LABEL,
        };
        color.set_if_neq(TextColor(tone));
    }
}

/// Learned levels are bronze, planned ones frost and the rest empty.
pub(super) fn paint_pips(
    looks: Res<SkillLooks>,
    mut pips: Query<(&SkillPip, &mut BackgroundColor, &mut BorderColor)>,
) {
    for (pip, mut fill, mut stroke) in &mut pips {
        let Some(look) = looks.skills.get(&pip.skill_id) else {
            continue;
        };
        let (background, border) = if pip.index < look.level {
            (ro_dark::BASE_EXP, ro_dark::BASE_EXP)
        } else if pip.index < look.total() {
            (ro_dark::FROST_GLOW, ro_dark::FROST_GLOW)
        } else {
            (ro_dark::BAR_TROUGH, ro_dark::INSET_RULE)
        };
        fill.set_if_neq(BackgroundColor(background));
        stroke.set_if_neq(BorderColor::all(border));
    }
}

pub(super) fn paint_overlays(
    looks: Res<SkillLooks>,
    mut steppers: Query<(&SkillPanelStepper, &mut Visibility), Without<SkillNodeDimmer>>,
    mut dimmers: Query<(&SkillNodeDimmer, &mut Visibility, &mut BackgroundColor)>,
) {
    for (stepper, mut visibility) in &mut steppers {
        let allowed = looks.skills.get(&stepper.skill_id).is_some_and(|look| {
            if stepper.raise {
                look.can_raise
            } else {
                look.can_lower
            }
        });
        visibility.set_if_neq(shown_if(allowed));
    }
    for (dimmer, mut visibility, mut wash) in &mut dimmers {
        let alpha = match looks.skills.get(&dimmer.0).map(|look| look.dim) {
            Some(Dim::Faded) => 0.78,
            Some(Dim::Unrelated) => 0.55,
            Some(Dim::None) | None => 0.0,
        };
        visibility.set_if_neq(shown_if(alpha > 0.0));
        wash.set_if_neq(BackgroundColor(ro_dark::INSET.with_alpha(alpha)));
    }
}

/// A met requirement is a solid bronze line, an unmet one shows its dashes, and the
/// lines inside the focused skill's chain glow frost.
pub(super) fn paint_lines(
    looks: Res<SkillLooks>,
    mut lines: Query<(
        &SkillLine,
        &mut Visibility,
        &mut BackgroundColor,
        &mut BoxShadow,
    )>,
) {
    for (line, mut visibility, mut fill, mut halo) in &mut lines {
        let lit = looks.chain.contains(&line.source) && looks.chain.contains(&line.target);
        let met = looks
            .skills
            .get(&line.source)
            .is_some_and(|look| look.total() >= line.minimum_level);
        let (color, shadow) = if lit {
            (
                ro_dark::FROST_GLOW,
                BoxShadow::new(
                    ro_dark::FROST_GLOW.with_alpha(0.7),
                    Val::ZERO,
                    Val::ZERO,
                    Val::ZERO,
                    px(5),
                ),
            )
        } else {
            (ro_dark::DIVIDER, glow(Color::NONE))
        };
        visibility.set_if_neq(shown_if(lit || met));
        fill.set_if_neq(BackgroundColor(color));
        halo.set_if_neq(shadow);
    }
}

pub(super) fn paint_tab_dots(looks: Res<SkillLooks>, mut dots: Query<(&SkillTabDot, &mut Node)>) {
    for (dot, mut node) in &mut dots {
        let display = if looks.planned_jobs.contains(&dot.0) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_start_at_the_bar_and_never_overrun_it() {
        let across = Bar {
            x: 10.0,
            y: 5.0,
            width: 20.0,
            height: 2.0,
        };
        let pieces = dash_bars(across);
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[0].x, 10.0);
        let last = pieces.last().unwrap();
        assert_eq!((last.x, last.width), (28.0, 2.0));
        assert!(pieces.iter().all(|piece| piece.height == 2.0));

        let down = Bar {
            x: 5.0,
            y: 10.0,
            width: 2.0,
            height: 9.0,
        };
        let pieces = dash_bars(down);
        assert_eq!(pieces.len(), 2);
        assert_eq!((pieces[1].y, pieces[1].height), (16.0, 3.0));
    }

    #[test]
    fn a_link_is_one_bar_in_a_column_and_an_elbow_across_columns() {
        let straight = link_bars((22.0, 8.0), (22.0, 120.0));
        assert_eq!(straight.len(), 1);
        assert_eq!(straight[0].y + straight[0].height, 120.0);

        let elbow = link_bars((22.0, 8.0), (222.0, 120.0));
        assert_eq!(elbow.len(), 3);
        assert_eq!(elbow[1].width, 202.0);
        assert_eq!(elbow[2].x, 222.0 + NODE / 2.0 - 1.0);
        assert_eq!(elbow[2].y + elbow[2].height, 120.0);
    }

    #[test]
    fn level_badge_reads_max_only_without_a_plan() {
        let look = Look {
            level: 5,
            staged: 0,
            max: 5,
            locked: false,
            edge: Edge::Max,
            dim: Dim::None,
            can_raise: false,
            can_lower: false,
        };
        assert_eq!(
            level_badge(&look),
            ("MAX".to_string(), ro_dark::BRONZE_LIGHT)
        );

        let planned = Look {
            level: 3,
            staged: 2,
            ..look
        };
        assert_eq!(
            level_badge(&planned),
            ("5/5".to_string(), ro_dark::FROST_GLOW)
        );
    }
}
