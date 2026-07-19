//! Shared chrome for the info modal: the 372px card, edge-grade ribbon, close
//! button, header, section label, meta-grid cell, and footer bar. Faithful to
//! `info-modals.css` (`.im-modal` / `.im-head` / `.im-sec` / `.im-meta` / `.im-foot`
//! / `.im-edge-*`) using the project's existing window and rarity theme tokens.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor, ThemeToken};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_RARITY_COMMON, TOKEN_RARITY_FINE, TOKEN_RARITY_MAGIC, TOKEN_RARITY_RARE, TOKEN_TEXT,
    TOKEN_TEXT_FAINT, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::chrome::{glyph_icon, ignore_picking};

use super::InfoModalRoot;

/// Card width — `.im-modal { width: 372px; }`.
pub const MODAL_WIDTH: f32 = 372.0;

/// Icon box side — `.im-icon { width/height: 62px; }`.
pub const ICON_BOX_SIZE: f32 = 62.0;

/// Rarity/state accent driving the top ribbon, icon border, and tag color — mirrors
/// `.im-edge-*` in `info-modals.css`, mapped onto the project's existing rarity
/// tokens rather than the CSS's separate per-grade glow colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeGrade {
    #[default]
    Common,
    Fine,
    Magic,
    Rare,
}

impl EdgeGrade {
    pub fn token(self) -> ThemeToken {
        match self {
            EdgeGrade::Common => TOKEN_RARITY_COMMON,
            EdgeGrade::Fine => TOKEN_RARITY_FINE,
            EdgeGrade::Magic => TOKEN_RARITY_MAGIC,
            EdgeGrade::Rare => TOKEN_RARITY_RARE,
        }
    }
}

/// The card chrome: sized glass panel, edge-colored top ribbon, close button, and
/// the caller's body content underneath. Stops pointer clicks from bubbling to the
/// backdrop, so clicking inside the card does not dismiss the modal.
pub fn card(edge: EdgeGrade, body: impl Scene) -> impl Scene {
    bsn! {
        Node {
            width: px(MODAL_WIDTH),
            max_height: percent(90),
            flex_direction: FlexDirection::Column,
            border: px(1),
            border_radius: BorderRadius::all(px(16)),
            overflow: Overflow::clip(),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        on(|mut click: On<Pointer<Click>>| click.propagate(false))
        Children [
            ribbon(edge),
            close_button(),
            body,
        ]
    }
}

/// The state ribbon at the very top of the card — `.im-ribbon`.
fn ribbon(edge: EdgeGrade) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(22),
            right: px(22),
            height: px(2),
            border_radius: BorderRadius::all(px(2)),
        }
        ThemeBackgroundColor({edge.token()})
        ignore_picking()
    }
}

/// Close button — `.im-close`. Despawns the modal root on press.
fn close_button() -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_FAINT) } }
        Node {
            position_type: PositionType::Absolute,
            top: px(13),
            right: px(13),
            width: px(26),
            height: px(26),
            border_radius: BorderRadius::all(px(7)),
        }
        on(close_modal)
    }
}

fn close_modal(_: On<Activate>, root: Query<Entity, With<InfoModalRoot>>, mut commands: Commands) {
    if let Ok(root) = root.single() {
        commands.entity(root).despawn();
    }
}

/// Owned inputs for [`header`], grouped so item/skill scenes pass one value instead
/// of a long positional argument list.
pub struct HeaderView {
    pub icon_path: Option<String>,
    pub refine: Option<i32>,
    pub sockets_filled: u8,
    pub sockets_total: u8,
    pub edge: EdgeGrade,
    pub name: String,
    pub tags: Vec<String>,
}

/// The header row — `.im-head`: icon box (optional refine badge + socket pips),
/// item/skill name, and a row of tag chips.
pub fn header(view: HeaderView) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: px(14),
            padding: {UiRect { left: px(20), right: px(20), top: px(20), bottom: px(16) }},
        }
        Children [
            icon_box(view.icon_path, view.refine, view.sockets_filled, view.sockets_total, view.edge),
            titles(view.name, view.tags, view.edge),
        ]
    }
}

fn icon_box(
    icon_path: Option<String>,
    refine: Option<i32>,
    sockets_filled: u8,
    sockets_total: u8,
    edge: EdgeGrade,
) -> impl Scene {
    let icon = icon_path.map(|path| EntityScene(item_icon(path)));
    let refine_text = refine.map(|r| format!("+{r}")).unwrap_or_default();
    let refine_display = if refine.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    let pips = (0..sockets_total)
        .map(|i| socket_pip(i < sockets_filled))
        .collect::<Vec<_>>();
    let pips_display = if sockets_total > 0 {
        Display::Flex
    } else {
        Display::None
    };
    bsn! {
        Node {
            width: px(ICON_BOX_SIZE),
            height: px(ICON_BOX_SIZE),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        BackgroundColor({Color::BLACK.with_alpha(0.42)})
        ThemeBorderColor({edge.token()})
        Children [
            {icon},
            (
                Node {
                    display: {refine_display},
                    position_type: PositionType::Absolute,
                    left: px(4),
                    top: px(4),
                    padding: {UiRect::axes(px(5), px(2))},
                    border_radius: BorderRadius::all(px(4)),
                }
                BackgroundColor({Color::srgba(0.086, 0.063, 0.012, 0.88)})
                Children [
                    (
                        Text(refine_text)
                        TextFont {
                            font: FontSourceTemplate::Handle(theme::FONT_BODY),
                            font_size: {FontSize::Px(10.0)},
                        }
                        TextColor({theme::GOLD})
                        ignore_picking()
                    ),
                ]
            ),
            (
                Node {
                    display: {pips_display},
                    position_type: PositionType::Absolute,
                    bottom: px(4),
                    left: px(0),
                    right: px(0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    column_gap: px(2),
                }
                Children [ {pips} ]
            ),
        ]
    }
}

/// The icon image filling the icon box, spawned only when `icon_path` resolved —
/// an absent icon leaves the box's dark background bare rather than an empty node.
fn item_icon(icon_path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {icon_path} }
        Node { width: percent(100), height: percent(100) }
        ignore_picking()
    }
}

fn socket_pip(filled: bool) -> impl Scene {
    let color = if filled {
        theme::GOLD
    } else {
        theme::STROKE_STRONG
    };
    bsn! {
        Node { width: px(4), height: px(4), border_radius: BorderRadius::all(px(1)) }
        BackgroundColor({color})
        ignore_picking()
    }
}

fn titles(name: String, tags: Vec<String>, edge: EdgeGrade) -> impl Scene {
    let tag_scenes = tags
        .into_iter()
        .map(|tag| tag_chip(tag, edge))
        .collect::<Vec<_>>();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: px(0),
            row_gap: px(7),
        }
        Children [
            (
                Text(name)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_TITLE),
                    font_size: {FontSize::Px(18.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(8),
                    row_gap: px(6),
                }
                Children [ {tag_scenes} ]
            ),
        ]
    }
}

fn tag_chip(text: String, edge: EdgeGrade) -> impl Scene {
    bsn! {
        Node {
            padding: {UiRect::axes(px(8), px(0))},
            height: px(21),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({Color::WHITE.with_alpha(0.04)})
        ThemeBorderColor({edge.token()})
        Children [
            (
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(10.0)},
                }
                ThemeTextColor({edge.token()})
                ignore_picking()
            ),
        ]
    }
}

/// Uppercase section label with an optional right-aligned mono counter — `.im-sec`.
pub fn section_label(text: String, counter: Option<String>) -> impl Scene {
    let counter_text = counter.unwrap_or_default();
    let counter_display = if counter_text.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Baseline,
            margin: {UiRect::bottom(px(9))},
        }
        Children [
            (
                Text({text.to_uppercase()})
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(9.5)},
                }
                ThemeTextColor({TOKEN_TEXT_FAINT})
                ignore_picking()
            ),
            (
                Node { display: {counter_display} }
                Children [
                    (
                        Text(counter_text)
                        TextFont {
                            font: FontSourceTemplate::Handle(theme::FONT_BODY),
                            font_size: {FontSize::Px(10.0)},
                        }
                        TextColor({theme::GOLD})
                        ignore_picking()
                    ),
                ]
            ),
        ]
    }
}

/// One cell of the two-column meta grid — `.im-meta .cell`.
pub fn meta_cell(key: String, value: String) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: {UiRect::axes(px(12), px(10))},
        }
        BackgroundColor({Color::srgba(0.039, 0.059, 0.051, 0.9)})
        Children [
            (
                Text({key.to_uppercase()})
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(8.5)},
                }
                ThemeTextColor({TOKEN_TEXT_FAINT})
                ignore_picking()
            ),
            (
                Text(value)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(13.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                ignore_picking()
            ),
        ]
    }
}

/// The two-column meta grid wrapping a set of [`meta_cell`] entries — `.im-meta`.
/// Built from flex rows (not `Display::Grid`) so it stays consistent with the rest
/// of the window chrome, which is flex-only.
pub fn meta_grid(cells: Vec<impl Scene>) -> impl Scene {
    let slots = cells.into_iter().map(meta_grid_slot).collect::<Vec<_>>();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(1),
            row_gap: px(1),
            border: px(1),
            border_radius: BorderRadius::all(px(11)),
            overflow: Overflow::clip(),
        }
        BackgroundColor({theme::STROKE})
        BorderColor::all(theme::STROKE)
        Children [ {slots} ]
    }
}

fn meta_grid_slot(cell: impl Scene) -> impl Scene {
    bsn! {
        Node { flex_basis: percent(50), flex_grow: 1.0, min_width: px(0) }
        Children [ cell ]
    }
}

/// The footer action row — `.im-foot`: a set of caller-supplied action scenes.
pub fn footer_bar(actions: Vec<impl Scene>) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(9),
            padding: {UiRect { left: px(20), right: px(20), top: px(14), bottom: px(18) }},
            margin: {UiRect::top(px(6))},
            border: {UiRect::top(px(1))},
        }
        BorderColor::all(theme::STROKE)
        Children [ {actions} ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_grade_maps_each_variant_to_its_rarity_token() {
        assert_eq!(EdgeGrade::Common.token(), TOKEN_RARITY_COMMON);
        assert_eq!(EdgeGrade::Fine.token(), TOKEN_RARITY_FINE);
        assert_eq!(EdgeGrade::Magic.token(), TOKEN_RARITY_MAGIC);
        assert_eq!(EdgeGrade::Rare.token(), TOKEN_RARITY_RARE);
    }

    #[test]
    fn close_button_despawns_the_modal_root() {
        let mut app = App::new();
        let root = app
            .world_mut()
            .spawn(InfoModalRoot)
            .observe(close_modal)
            .id();

        app.world_mut().trigger(Activate { entity: root });
        app.world_mut().flush();

        assert!(app.world().get_entity(root).is_err());
    }
}
