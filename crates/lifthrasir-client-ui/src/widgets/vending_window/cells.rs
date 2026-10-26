//! The vending item cell: an item icon with amount/refine badges, optionally
//! dimmed and ticked. Visuals follow the pushcart cell; callers add their own
//! marker, `Pickable` and observers on top of [`item_cell`].

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::paths::item_icon_path;
use lifthrasir_ui::theme;
use lifthrasir_ui::widgets::chrome::{glyph_icon, ignore_picking};

pub const CELL_SIZE: f32 = 32.0;
/// Icon opacity of a dimmed cell (fully listed or fully staged).
const DIM_ALPHA: f32 = 0.35;

/// What one cell shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemCellView {
    pub icon: Option<String>,
    pub amount: u32,
    pub refine: u32,
    pub dimmed: bool,
    pub ticked: bool,
}

/// The `ro://` icon path for an item, or `None` when the catalog has no art.
pub fn item_icon(item_db: &ItemDb, nameid: u32, identified: bool) -> Option<String> {
    item_db
        .icon_resource(nameid, identified)
        .map(item_icon_path)
}

/// The display name for an item, falling back to its id.
pub fn item_name(item_db: &ItemDb, nameid: u32, identified: bool) -> String {
    item_db
        .name(nameid, identified)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{nameid}"))
}

pub fn item_cell(view: ItemCellView) -> impl Scene {
    let tint = if view.dimmed {
        Color::WHITE.with_alpha(DIM_ALPHA)
    } else {
        Color::WHITE
    };
    let icon = view.icon.map(|path| EntityScene(cell_icon(path, tint)));
    let amount = (view.amount > 1).then(|| EntityScene(badge(view.amount.to_string(), false)));
    let refine = (view.refine > 0).then(|| EntityScene(badge(format!("+{}", view.refine), true)));
    let tick = view.ticked.then(|| EntityScene(tick()));
    bsn! {
        Node {
            width: px(CELL_SIZE),
            height: px(CELL_SIZE),
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(theme::GOLD_FAINT)
        Children [ {icon} -- {amount} -- {refine} -- {tick} ]
    }
}

fn cell_icon(path: String, tint: Color) -> impl Scene {
    bsn! {
        ImageNode { image: {path}, color: tint }
        Node {
            position_type: PositionType::Absolute,
            left: px(0), top: px(0), right: px(0), bottom: px(0),
            width: percent(100),
            height: percent(100),
        }
        @ignore_picking()
    }
}

/// A 9px badge: refine pinned top-left in gold, amount bottom-right.
fn badge(text: String, refine: bool) -> impl Scene {
    let (color, node) = if refine {
        (
            theme::GOLD,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(1.0),
                top: Val::Px(0.0),
                ..default()
            },
        )
    } else {
        (
            theme::TEXT,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(1.0),
                bottom: Val::Px(0.0),
                ..default()
            },
        )
    };
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(9.0)},
        }
        TextColor(color)
        node
        @ignore_picking()
    }
}

/// The emerald "listed" tick, pinned top-right.
fn tick() -> impl Scene {
    bsn! {
        Node { position_type: PositionType::Absolute, right: px(1), top: px(1) }
        @ignore_picking()
        Children [ @glyph_icon("check", 10.0, theme::EMERALD_BRI) ]
    }
}
