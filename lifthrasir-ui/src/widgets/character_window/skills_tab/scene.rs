use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::skill::SkillTreeState;
use game_engine::infrastructure::skill::SkillCatalog;

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::layout::{JobBand, Segment, TreeLayout};
use super::{
    SkillPanelBank, SkillPanelCell, SkillPanelCommitButton, SkillPanelStaging, SkillPanelStepper,
    SkillPanelUi, cell_icon_color, format_level, on_apply, on_cell_click, on_cell_drag_start,
    on_reset, on_stepper, skill_name,
};

const PANE_HEIGHT: f32 = 300.0;

#[derive(Component, Clone, Copy, Default)]
pub(super) struct SkillJobBand;

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
}

struct CellView {
    skill_id: u32,
    bounds: super::layout::Bounds,
    icon: Option<String>,
    level: u32,
    max_level: u32,
    name: String,
    learned: bool,
    icon_color: Color,
    can_raise: bool,
    can_lower: bool,
    selected: bool,
}

struct ConnectorView {
    marker: SkillConnector,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    color: Color,
}

pub(super) fn body(
    layout: TreeLayout,
    tree: &SkillTreeState,
    ui: &SkillPanelUi,
    staging: &SkillPanelStaging,
    status: Option<&CharacterStatus>,
    catalog: Option<&SkillCatalog>,
) -> impl Scene + use<> {
    let points_left = status
        .map(|status| staging.points_left(status.skill_point))
        .unwrap_or(0);
    let cells = cell_views(&layout, tree, ui, staging, status, catalog);
    let connectors = connector_views(&layout, tree, staging);

    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [
            toolbar(points_left, staging.is_empty()),
            canvas(layout, cells, connectors),
            footer(staging.spent(), staging.is_empty()),
        ]
    }
}

fn cell_views(
    layout: &TreeLayout,
    tree: &SkillTreeState,
    ui: &SkillPanelUi,
    staging: &SkillPanelStaging,
    status: Option<&CharacterStatus>,
    catalog: Option<&SkillCatalog>,
) -> Vec<CellView> {
    layout
        .nodes
        .iter()
        .filter_map(|placement| {
            let node = tree.skills.get(&placement.skill_id)?;
            let level = staging.effective_level(placement.skill_id, tree);
            let learned = level > 0;
            let maxed = level >= node.max_level && node.max_level > 0;
            Some(CellView {
                skill_id: placement.skill_id,
                bounds: placement.bounds,
                icon: catalog.and_then(|catalog| catalog.icon_path(placement.skill_id)),
                level,
                max_level: node.max_level,
                name: skill_name(placement.skill_id, catalog),
                learned,
                icon_color: cell_icon_color(learned, maxed),
                can_raise: status.is_some_and(|status| {
                    staging.can_raise(placement.skill_id, tree, status, status.skill_point)
                }),
                can_lower: staging.can_lower(placement.skill_id, tree),
                selected: ui.selected == Some(placement.skill_id),
            })
        })
        .collect()
}

fn connector_views(
    layout: &TreeLayout,
    tree: &SkillTreeState,
    staging: &SkillPanelStaging,
) -> Vec<ConnectorView> {
    let mut views = Vec::new();
    for connector in &layout.connectors {
        let met = staging.effective_level(connector.source, tree) >= connector.minimum_level;
        let (thickness, color) = if connector.backlink {
            (1.0, theme::TEXT_FAINT.with_alpha(0.38))
        } else if met {
            (2.0, theme::EMERALD.with_alpha(0.45))
        } else {
            (1.0, theme::GOLD_FAINT)
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

fn toolbar(points_left: u32, empty: bool) -> impl Scene {
    let reset_bg = theme::FIELD.with_alpha(if empty { 0.3 } else { 1.0 });
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
                    bank_text(points_left.to_string()),
                ]
            ),
            reset_button(reset_bg, !empty),
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
                Children [ chrome_text(label, 10.5, theme::TEXT_DIM) ]
            ),
        ]
    }
}

fn skill_cell(view: CellView) -> impl Scene {
    let background = if view.selected {
        theme::EMERALD_INK
    } else {
        Color::NONE
    };
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
        BackgroundColor(background)
        ZIndex(2)
        Pickable
        on(on_cell_click)
        on(on_cell_drag_start)
        Children [
            (
                Node {
                    position_type: PositionType::Relative,
                    width: px(44), height: px(44),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(10)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(if view.learned { theme::EMERALD } else { theme::STROKE })
                ignore_picking()
                Children [ {icon} ]
            ),
            chrome_text(view.name, 8.5, view.icon_color),
            stepper_row(
                view.skill_id,
                format_level(view.level, view.max_level),
                view.can_raise,
                view.can_lower,
            ),
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

fn stepper_row(skill_id: u32, level: String, can_raise: bool, can_lower: bool) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(3),
        }
        ignore_picking()
        Children [
            stepper(skill_id, false, can_lower),
            chrome_text(level, 9.0, theme::TEXT_FAINT),
            stepper(skill_id, true, can_raise),
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
        Children [ chrome_text(glyph.to_string(), 10.0, if enabled { theme::EMERALD_INK } else { theme::TEXT_FAINT }) ]
    }
}

fn footer(staged: u32, empty: bool) -> impl Scene {
    let alpha = if empty { 0.3 } else { 1.0 };
    let staged_text = if staged == 1 {
        "1 change staged".to_string()
    } else {
        format!("{staged} changes staged")
    };
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
            chrome_text(staged_text, 10.0, theme::TEXT_FAINT),
            (
                Node {
                    margin: {UiRect::left(auto())},
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                }
                ignore_picking()
                Children [ apply_button(theme::EMERALD.with_alpha(alpha), !empty) ]
            ),
        ]
    }
}

fn bank_text(value: String) -> impl Scene {
    bsn! {
        SkillPanelBank
        Text(value)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(16.0)},
        }
        TextColor(theme::GOLD)
        ignore_picking()
    }
}

fn reset_button(background: Color, enabled: bool) -> impl Scene {
    bsn! {
        SkillPanelCommitButton
        Node {
            height: px(28),
            padding: {UiRect::horizontal(px(13))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(background)
        template_value(if enabled { Pickable::default() } else { Pickable::IGNORE })
        on(on_reset)
        Children [ chrome_text("Reset Plan".to_string(), 11.0, theme::TEXT_DIM) ]
    }
}

fn apply_button(background: Color, enabled: bool) -> impl Scene {
    bsn! {
        SkillPanelCommitButton
        Node {
            height: px(28),
            padding: {UiRect::horizontal(px(16))},
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(background)
        template_value(if enabled { Pickable::default() } else { Pickable::IGNORE })
        on(on_apply)
        Children [ chrome_text("Apply".to_string(), 11.5, theme::EMERALD_INK) ]
    }
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
