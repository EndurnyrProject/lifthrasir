use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use game_engine::domain::skill::SkillTreeState;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::layout::{JobBand, Segment, TreeLayout};
use super::{
    SkillGateSnapshot, SkillPanelBank, SkillPanelCell, SkillPanelCommitButton, SkillPanelStaging,
    SkillPanelStepper, SkillPanelUi, cell_icon_color, format_level, on_apply, on_cell_click,
    on_cell_drag_start, on_reset, on_stepper, skill_name,
};

const PANE_HEIGHT: f32 = 300.0;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillJobBand;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillJobPointText(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeFrame(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeName(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeLevel(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeDimmer(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillNodeControls(pub u32);

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillPanelStagedCount;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillStepperGlyph {
    skill_id: u32,
    raise: bool,
}

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillCanvasFrame;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillCanvasViewport;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillEmptyMessage;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SkillConnector {
    pub source: u32,
    pub target: u32,
    pub minimum_level: u32,
    pub backlink: bool,
    pub segment: u8,
    pub dash: u16,
    pub horizontal: bool,
}

struct CellView {
    skill_id: u32,
    bounds: super::layout::Bounds,
    icon: Option<String>,
    name: String,
}

struct ConnectorView {
    marker: SkillConnector,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    color: Color,
}

pub(super) fn body(layout: TreeLayout, catalog: Option<&SkillCatalog>) -> impl Scene + use<> {
    let cells = cell_views(&layout, catalog);
    let connectors = connector_views(&layout);

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [ toolbar(), canvas(layout, cells, connectors), footer() ]
    }
}

fn cell_views(layout: &TreeLayout, catalog: Option<&SkillCatalog>) -> Vec<CellView> {
    layout
        .nodes
        .iter()
        .map(|placement| CellView {
            skill_id: placement.skill_id,
            bounds: placement.bounds,
            icon: catalog.and_then(|catalog| catalog.icon_path(placement.skill_id)),
            name: skill_name(placement.skill_id, catalog),
        })
        .collect()
}

fn connector_views(layout: &TreeLayout) -> Vec<ConnectorView> {
    let mut views = Vec::new();
    for connector in &layout.connectors {
        let thickness = 1.0;
        let color = if connector.backlink {
            theme::TEXT_FAINT.with_alpha(0.38)
        } else {
            theme::GOLD_FAINT
        };
        for (segment, geometry) in connector.segments.iter().enumerate() {
            for (dash, (left, top, width, height)) in
                line_pieces(*geometry, thickness, connector.backlink)
                    .into_iter()
                    .enumerate()
            {
                views.push(ConnectorView {
                    marker: SkillConnector {
                        source: connector.source,
                        target: connector.target,
                        minimum_level: connector.minimum_level,
                        backlink: connector.backlink,
                        segment: segment as u8,
                        dash: dash as u16,
                        horizontal: geometry.start.y == geometry.end.y,
                    },
                    left,
                    top,
                    width,
                    height,
                    color,
                });
            }
        }
    }
    views
}

fn line_pieces(segment: Segment, thickness: f32, dashed: bool) -> Vec<(f32, f32, f32, f32)> {
    if !dashed {
        return vec![line_bounds(segment, thickness)];
    }
    const DASH: f32 = 6.0;
    const GAP: f32 = 4.0;
    let horizontal =
        (segment.start.x - segment.end.x).abs() >= (segment.start.y - segment.end.y).abs();
    let length = if horizontal {
        (segment.start.x - segment.end.x).abs()
    } else {
        (segment.start.y - segment.end.y).abs()
    };
    if length == 0.0 {
        return vec![line_bounds(segment, thickness)];
    }
    let mut pieces = Vec::new();
    let mut offset = 0.0;
    while offset < length {
        let dash = DASH.min(length - offset);
        if horizontal {
            pieces.push((
                segment.start.x.min(segment.end.x) + offset,
                segment.start.y - thickness / 2.0,
                dash,
                thickness,
            ));
        } else {
            pieces.push((
                segment.start.x - thickness / 2.0,
                segment.start.y.min(segment.end.y) + offset,
                thickness,
                dash,
            ));
        }
        offset += DASH + GAP;
    }
    let last = pieces.last_mut().expect("a positive line has a dash");
    if horizontal {
        last.2 = segment.start.x.max(segment.end.x) - last.0;
    } else {
        last.3 = segment.start.y.max(segment.end.y) - last.1;
    }
    pieces
}

fn line_bounds(segment: Segment, thickness: f32) -> (f32, f32, f32, f32) {
    let left = segment.start.x.min(segment.end.x);
    let top = segment.start.y.min(segment.end.y);
    let width = (segment.start.x - segment.end.x).abs();
    let height = (segment.start.y - segment.end.y).abs();
    if width >= height {
        (left, top - thickness / 2.0, width.max(thickness), thickness)
    } else {
        (
            left - thickness / 2.0,
            top,
            thickness,
            height.max(thickness),
        )
    }
}

fn toolbar() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            height: px(34),
            column_gap: px(10),
        }
        ignore_picking()
        Children [
            chrome_text("Requirements flow  →  left to right".to_string(), 10.0, theme::TEXT_FAINT),
            (
                Node {
                    margin: {UiRect::left(auto())},
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    padding: {UiRect::axes(px(10), px(5))},
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.3)})
                BorderColor::all(theme::GOLD_FAINT)
                ignore_picking()
                Children [
                    chrome_text("Skill Points".to_string(), 9.0, theme::TEXT_FAINT),
                    bank_text(),
                ]
            ),
            reset_button(),
        ]
    }
}

fn canvas(layout: TreeLayout, cells: Vec<CellView>, connectors: Vec<ConnectorView>) -> impl Scene {
    let empty = layout.nodes.is_empty();
    let bands: Vec<_> = layout.bands.iter().map(job_band).collect();
    let connectors: Vec<_> = connectors.into_iter().map(connector_segment).collect();
    let nodes: Vec<_> = cells.into_iter().map(skill_cell).collect();
    let empty_message = empty.then(|| EntityScene(empty_content()));
    let content_width = if empty {
        percent(100)
    } else {
        px(layout.width)
    };
    let content_height = if empty {
        percent(100)
    } else {
        px(layout.height)
    };
    let content = tree_content(
        content_width,
        content_height,
        bands,
        connectors,
        nodes,
        empty_message,
    );

    bsn! {
        SkillCanvasFrame
        Node {
            height: px(PANE_HEIGHT),
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.18)})
        BorderColor::all(theme::GOLD_FAINT)
        ignore_picking()
        Children [
            (
                #canvas
                SkillCanvasViewport
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0),
                    width: percent(100), height: px(PANE_HEIGHT),
                    overflow: {Overflow::scroll()},
                    padding: {UiRect { left: Val::Px(7.0), right: Val::Px(15.0), top: Val::Px(7.0), bottom: Val::Px(15.0) }},
                }
                ScrollArea
                Pickable
                Children [ content ]
            ),
            (
                @FeathersScrollbar { @target: #canvas, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(3), top: px(4), bottom: px(12), width: px(6),
                }
            ),
            (
                @FeathersScrollbar { @target: #canvas, @orientation: {ControlOrientation::Horizontal} }
                Node {
                    position_type: PositionType::Absolute,
                    left: px(4), right: px(12), bottom: px(3), height: px(6),
                }
            ),
        ]
    }
}

fn tree_content(
    width: Val,
    height: Val,
    bands: Vec<impl Scene>,
    connectors: Vec<impl Scene>,
    nodes: Vec<impl Scene>,
    empty_message: Option<EntityScene<impl Scene>>,
) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Relative,
            width: {width},
            height: {height},
            flex_shrink: 0.0,
        }
        ignore_picking()
        Children [ {bands}, {connectors}, {nodes}, {empty_message} ]
    }
}

fn connector_segment(view: ConnectorView) -> impl Scene {
    bsn! {
        template_value(view.marker)
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Px(view.left)},
            top: {Val::Px(view.top)},
            width: {Val::Px(view.width)},
            height: {Val::Px(view.height)},
        }
        template_value(BackgroundColor(view.color))
        ZIndex(1)
        ignore_picking()
    }
}

fn empty_content() -> impl Scene {
    bsn! {
        SkillEmptyMessage
        Text("No skills.")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(12.0)},
        }
        TextColor(theme::TEXT_FAINT)
        Node { margin: {UiRect::all(auto())} }
        ignore_picking()
    }
}

fn job_band(band: &JobBand) -> impl Scene + use<> {
    let label = band
        .label
        .clone()
        .unwrap_or_else(|| format!("Job #{}", band.job_id));
    bsn! {
        SkillJobBand
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Px(band.x)},
            top: px(0),
            width: {Val::Px(band.width)},
            height: {Val::Px(band.height)},
            padding: {UiRect::all(px(7))},
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({Color::srgba(1.0, 1.0, 1.0, 0.014)})
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.05))
        ZIndex(0)
        ignore_picking()
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(25),
                    align_items: AlignItems::Center,
                    padding: {UiRect::horizontal(px(5))},
                    border_radius: BorderRadius::all(px(7)),
                }
                BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.3)})
                ignore_picking()
                Children [
                    chrome_text(label, 10.5, theme::TEXT_DIM),
                    job_point_text(band.job_id),
                ]
            ),
        ]
    }
}

fn skill_cell(view: CellView) -> impl Scene {
    let icon = view.icon.map(|path| EntityScene(skill_icon(path)));
    bsn! {
        template_value(SkillPanelCell(view.skill_id))
        Node {
            position_type: PositionType::Absolute,
            left: {Val::Px(view.bounds.x)},
            top: {Val::Px(view.bounds.y)},
            width: {Val::Px(view.bounds.width)},
            height: {Val::Px(view.bounds.height)},
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            padding: {UiRect::vertical(px(3))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(Color::NONE)
        ZIndex(2)
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        Children [
            (
                template_value(SkillNodeFrame(view.skill_id))
                Node {
                    position_type: PositionType::Relative,
                    width: px(44), height: px(44),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(10)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(theme::STROKE)
                Outline { width: px(0), offset: px(2), color: Color::NONE }
                ignore_picking()
                Children [ {icon} ]
            ),
            skill_name_text(view.skill_id, view.name),
            stepper_row(view.skill_id),
            dimmer(view.skill_id),
        ]
    }
}

fn skill_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node { width: px(30), height: px(30) }
        ignore_picking()
    }
}

fn stepper_row(skill_id: u32) -> impl Scene {
    bsn! {
        template_value(SkillNodeControls(skill_id))
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(3),
        }
        Visibility::Hidden
        ignore_picking()
        Children [
            stepper(skill_id, false, false),
            skill_level_text(skill_id),
            stepper(skill_id, true, false),
        ]
    }
}

pub(super) fn stepper(skill_id: u32, raise: bool, enabled: bool) -> impl Scene {
    let glyph = if raise { "+" } else { "−" };
    let background = if enabled {
        theme::EMERALD
    } else {
        theme::FIELD
    };
    let pickable = if enabled {
        Pickable::default()
    } else {
        Pickable::IGNORE
    };
    bsn! {
        template_value(SkillPanelStepper { skill_id, raise })
        Node {
            width: px(14), height: px(14),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor(background)
        template_value(pickable)
        on(on_stepper)
        Children [ stepper_glyph(skill_id, raise, glyph.to_string(), enabled) ]
    }
}

fn skill_name_text(skill_id: u32, name: String) -> impl Scene {
    bsn! {
        template_value(SkillNodeName(skill_id))
        Text({name})
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(8.5)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

fn skill_level_text(skill_id: u32) -> impl Scene {
    bsn! {
        template_value(SkillNodeLevel(skill_id))
        Text("0/0")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

fn stepper_glyph(skill_id: u32, raise: bool, glyph: String, enabled: bool) -> impl Scene {
    bsn! {
        template_value(SkillStepperGlyph { skill_id, raise })
        Text({glyph})
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(10.0)},
        }
        TextColor({if enabled { theme::EMERALD_INK } else { theme::TEXT_FAINT }})
        ignore_picking()
    }
}

fn dimmer(skill_id: u32) -> impl Scene {
    bsn! {
        template_value(SkillNodeDimmer(skill_id))
        Node {
            position_type: PositionType::Absolute,
            left: px(0), right: px(0), top: px(0), bottom: px(0),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.5)})
        Visibility::Hidden
        Pickable::IGNORE
        ZIndex(3)
    }
}

fn job_point_text(job_id: u32) -> impl Scene {
    bsn! {
        template_value(SkillJobPointText(job_id))
        Text("0 points")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(theme::TEXT_FAINT)
        Node { margin: {UiRect::left(auto())} }
        ignore_picking()
    }
}

fn staged_count_text() -> impl Scene {
    bsn! {
        SkillPanelStagedCount
        Text("0 changes staged")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(10.0)},
        }
        TextColor(theme::TEXT_FAINT)
        ignore_picking()
    }
}

fn footer() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            height: px(38),
            padding: {UiRect::horizontal(px(12))},
            column_gap: px(9),
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        BorderColor::all(theme::STROKE)
        ignore_picking()
        Children [
            staged_count_text(),
            (
                Node {
                    margin: {UiRect::left(auto())},
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                }
                ignore_picking()
                Children [ apply_button() ]
            ),
        ]
    }
}

fn bank_text() -> impl Scene {
    bsn! {
        SkillPanelBank
        Text("0")
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(16.0)},
        }
        TextColor(theme::GOLD)
        ignore_picking()
    }
}

fn reset_button() -> impl Scene {
    bsn! {
        SkillPanelCommitButton { apply: false }
        Node {
            height: px(28),
            padding: {UiRect::horizontal(px(13))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor({theme::FIELD.with_alpha(0.3)})
        Pickable::IGNORE
        on(on_reset)
        Children [ chrome_text("Reset Plan".to_string(), 11.0, theme::TEXT_DIM) ]
    }
}

fn apply_button() -> impl Scene {
    bsn! {
        SkillPanelCommitButton { apply: true }
        Node {
            height: px(28),
            padding: {UiRect::horizontal(px(16))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor({theme::EMERALD.with_alpha(0.3)})
        Pickable::IGNORE
        on(on_apply)
        Children [ chrome_text("Apply".to_string(), 11.5, theme::EMERALD_INK) ]
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn project_live(
    tree: Res<SkillTreeState>,
    ui: Res<SkillPanelUi>,
    staging: Res<SkillPanelStaging>,
    gates: Res<SkillGateSnapshot>,
    mut backgrounds: ParamSet<(
        Query<(&SkillPanelCell, &mut BackgroundColor)>,
        Query<(&SkillPanelStepper, &mut BackgroundColor, &mut Pickable)>,
        Query<(&SkillConnector, &mut Node, &mut BackgroundColor)>,
        Query<(&SkillPanelCommitButton, &mut BackgroundColor, &mut Pickable)>,
    )>,
    mut frames: Query<(&SkillNodeFrame, &mut BorderColor, &mut Outline)>,
    mut focus_nodes: ParamSet<(
        Query<(&SkillNodeDimmer, &mut Visibility)>,
        Query<(&SkillNodeControls, &mut Visibility)>,
    )>,
    mut text_colors: ParamSet<(
        Query<(&SkillNodeName, &mut TextColor)>,
        Query<(&SkillStepperGlyph, &mut TextColor)>,
    )>,
    mut texts: ParamSet<(
        Query<(&SkillNodeLevel, &mut Text)>,
        Query<(&SkillJobPointText, &mut Text)>,
        Query<&mut Text, With<SkillPanelBank>>,
        Query<&mut Text, With<SkillPanelStagedCount>>,
    )>,
) {
    let can_raise = |skill_id| {
        gates.values.is_some_and(|gates| {
            staging.can_raise_with_gates(
                skill_id,
                &tree,
                gates.base_level,
                gates.job_level,
                gates.skill_point,
            )
        })
    };
    let focused = ui.hovered.or(ui.selected);
    let focus = focused.and_then(|skill_id| super::layout::focus(&tree, skill_id));
    let related_node = |skill_id| {
        focus.as_ref().is_none_or(|focus| {
            focus.focused == skill_id
                || focus.prerequisite_nodes.contains(&skill_id)
                || focus.unlock_nodes.contains(&skill_id)
        })
    };
    let related_edge = |source, target| {
        focus.as_ref().is_some_and(|focus| {
            focus.prerequisite_edges.contains(&(source, target))
                || focus.unlock_edges.contains(&(source, target))
        })
    };
    for (cell, mut background) in &mut backgrounds.p0() {
        let color = if ui.selected == Some(cell.0) {
            theme::EMERALD_INK
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    for (marker, mut border, mut outline) in &mut frames {
        let Some(node) = tree.skills.get(&marker.0) else {
            continue;
        };
        let level = staging.effective_level(marker.0, &tree);
        let color = if node.max_level > 0 && level >= node.max_level {
            theme::GOLD
        } else if level > 0 {
            theme::EMERALD
        } else if can_raise(marker.0) {
            theme::GOLD_FAINT
        } else {
            theme::STROKE
        };
        if border.top != color {
            *border = BorderColor::all(color);
        }
        let (outline_width, outline_color) = if focused == Some(marker.0) {
            (px(2), theme::EMERALD_BRI)
        } else if focus.as_ref().is_some_and(|focus| {
            focus.prerequisite_nodes.contains(&marker.0) || focus.unlock_nodes.contains(&marker.0)
        }) {
            (px(1), theme::GOLD)
        } else {
            (px(0), Color::NONE)
        };
        if outline.width != outline_width {
            outline.width = outline_width;
        }
        if outline.color != outline_color {
            outline.color = outline_color;
        }
    }
    for (marker, mut visibility) in &mut focus_nodes.p0() {
        let next = if related_node(marker.0) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (marker, mut visibility) in &mut focus_nodes.p1() {
        let next = if focused == Some(marker.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (marker, mut color) in &mut text_colors.p0() {
        let Some(node) = tree.skills.get(&marker.0) else {
            continue;
        };
        let level = staging.effective_level(marker.0, &tree);
        let next = cell_icon_color(level > 0, node.max_level > 0 && level >= node.max_level);
        if color.0 != next {
            color.0 = next;
        }
    }
    for (marker, mut text) in &mut texts.p0() {
        let Some(node) = tree.skills.get(&marker.0) else {
            continue;
        };
        let next = format_level(staging.effective_level(marker.0, &tree), node.max_level);
        if text.0 != next {
            text.0 = next;
        }
    }
    for (stepper, mut background, mut pickable) in &mut backgrounds.p1() {
        let enabled = if stepper.raise {
            can_raise(stepper.skill_id)
        } else {
            staging.can_lower(stepper.skill_id, &tree)
        };
        let color = if enabled {
            theme::EMERALD
        } else {
            theme::FIELD
        };
        if background.0 != color {
            background.0 = color;
        }
        let next = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
        if *pickable != next {
            *pickable = next;
        }
    }
    for (glyph, mut color) in &mut text_colors.p1() {
        let enabled = if glyph.raise {
            can_raise(glyph.skill_id)
        } else {
            staging.can_lower(glyph.skill_id, &tree)
        };
        let next = if enabled {
            theme::EMERALD_INK
        } else {
            theme::TEXT_FAINT
        };
        if color.0 != next {
            color.0 = next;
        }
    }
    for (connector, mut node, mut background) in &mut backgrounds.p2() {
        let met = staging.effective_level(connector.source, &tree) >= connector.minimum_level;
        if !connector.backlink {
            let thickness = if met { 2.0 } else { 1.0 };
            set_connector_thickness(&mut node, connector.horizontal, thickness);
        }
        let color = match (
            focus.is_some(),
            related_edge(connector.source, connector.target),
        ) {
            (true, true) if connector.backlink => theme::TEXT_FAINT.with_alpha(0.7),
            (true, true) if met => theme::EMERALD.with_alpha(0.9),
            (true, true) => theme::GOLD.with_alpha(0.85),
            (true, false) if connector.backlink => theme::TEXT_FAINT.with_alpha(0.18),
            (true, false) if met => theme::EMERALD.with_alpha(0.16),
            (true, false) => theme::GOLD.with_alpha(0.14),
            (false, _) if connector.backlink => theme::TEXT_FAINT.with_alpha(0.38),
            (false, _) if met => theme::EMERALD.with_alpha(0.45),
            (false, _) => theme::GOLD_FAINT,
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    let mut totals = std::collections::HashMap::<u32, u32>::new();
    for (&skill_id, node) in &tree.skills {
        *totals.entry(node.job_id).or_default() += staging.effective_level(skill_id, &tree);
    }
    for (job, mut text) in &mut texts.p1() {
        let next = format!("{} points", totals.get(&job.0).copied().unwrap_or(0));
        if text.0 != next {
            text.0 = next;
        }
    }
    if let Ok(mut text) = texts.p2().single_mut() {
        let next = gates
            .values
            .map(|gates| staging.points_left(gates.skill_point))
            .unwrap_or(0)
            .to_string();
        if text.0 != next {
            text.0 = next;
        }
    }
    if let Ok(mut text) = texts.p3().single_mut() {
        let spent = staging.spent();
        let next = if spent == 1 {
            "1 change staged".to_string()
        } else {
            format!("{spent} changes staged")
        };
        if text.0 != next {
            text.0 = next;
        }
    }
    let enabled = !staging.is_empty();
    for (button, mut background, mut pickable) in &mut backgrounds.p3() {
        let color = if button.apply {
            theme::EMERALD.with_alpha(if enabled { 1.0 } else { 0.3 })
        } else {
            theme::FIELD.with_alpha(if enabled { 1.0 } else { 0.3 })
        };
        if background.0 != color {
            background.0 = color;
        }
        let next = if enabled {
            Pickable::default()
        } else {
            Pickable::IGNORE
        };
        if *pickable != next {
            *pickable = next;
        }
    }
}

fn set_connector_thickness(node: &mut Node, horizontal: bool, thickness: f32) {
    let (position, size) = if horizontal {
        (&mut node.top, &mut node.height)
    } else {
        (&mut node.left, &mut node.width)
    };
    let (Val::Px(old_position), Val::Px(old_thickness)) = (*position, *size) else {
        panic!("skill connector geometry must use pixel values");
    };
    if old_thickness == thickness {
        return;
    }
    *position = px(old_position + old_thickness / 2.0 - thickness / 2.0);
    *size = px(thickness);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashed_line_pieces_cover_both_endpoints_for_non_aligned_lengths() {
        let cases = [
            Segment {
                start: super::super::layout::Point { x: 2.0, y: 5.0 },
                end: super::super::layout::Point { x: 20.0, y: 5.0 },
            },
            Segment {
                start: super::super::layout::Point { x: 20.0, y: 5.0 },
                end: super::super::layout::Point { x: 2.0, y: 5.0 },
            },
            Segment {
                start: super::super::layout::Point { x: 5.0, y: 2.0 },
                end: super::super::layout::Point { x: 5.0, y: 20.0 },
            },
            Segment {
                start: super::super::layout::Point { x: 5.0, y: 20.0 },
                end: super::super::layout::Point { x: 5.0, y: 2.0 },
            },
        ];

        for segment in cases {
            let pieces = line_pieces(segment, 1.0, true);
            assert_eq!(pieces.len(), 2);
            let horizontal = segment.start.y == segment.end.y;
            let first_start = if horizontal {
                pieces.first().expect("first dash").0
            } else {
                pieces.first().expect("first dash").1
            };
            let last = pieces.last().expect("last dash");
            let last_end = if horizontal {
                last.0 + last.2
            } else {
                last.1 + last.3
            };
            let minimum = if horizontal {
                segment.start.x.min(segment.end.x)
            } else {
                segment.start.y.min(segment.end.y)
            };
            let maximum = if horizontal {
                segment.start.x.max(segment.end.x)
            } else {
                segment.start.y.max(segment.end.y)
            };

            assert_eq!(first_start, minimum);
            assert_eq!(last_end, maximum);
            assert!(pieces.windows(2).all(|pair| {
                let first_end = if horizontal {
                    pair[0].0 + pair[0].2
                } else {
                    pair[0].1 + pair[0].3
                };
                let second_start = if horizontal { pair[1].0 } else { pair[1].1 };
                first_end <= second_start
            }));
        }
    }

    #[test]
    fn connector_thickness_preserves_horizontal_and_vertical_midpoints() {
        let mut horizontal = Node {
            top: px(10),
            height: px(1),
            ..default()
        };
        set_connector_thickness(&mut horizontal, true, 2.0);
        assert_eq!((horizontal.top, horizontal.height), (px(9.5), px(2)));
        set_connector_thickness(&mut horizontal, true, 1.0);
        assert_eq!((horizontal.top, horizontal.height), (px(10), px(1)));

        let mut vertical = Node {
            left: px(20),
            width: px(1),
            ..default()
        };
        set_connector_thickness(&mut vertical, false, 2.0);
        assert_eq!((vertical.left, vertical.width), (px(19.5), px(2)));
        set_connector_thickness(&mut vertical, false, 1.0);
        assert_eq!((vertical.left, vertical.width), (px(20), px(1)));
    }

    #[test]
    fn unresolved_job_label_uses_job_id() {
        let band = JobBand {
            job_id: 42,
            label: None,
            x: 0.0,
            width: 100.0,
            height: 100.0,
            cycle_break: false,
        };
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::scene::ScenePlugin,
        ));
        app.init_asset::<Font>();
        app.world_mut().spawn_scene(job_band(&band)).unwrap();
        app.update();

        let world = app.world_mut();
        assert!(
            world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0 == "Job #42")
        );
    }
}
