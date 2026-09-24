use bevy::input_focus::{AutoFocus, tab_navigation::TabIndex};
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate};
use bevy::ui_widgets::ScrollArea;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use game_engine::domain::inventory::Item;
use game_engine::infrastructure::assets::item_icon_path;
use game_engine::infrastructure::item::ItemDb;
use net_contract::events::zone::ZoneInventoryItem;

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::{chrome_text, drag_window, ignore_picking};

use super::*;

#[derive(Component, Default, Clone)]
pub struct TradeTitlebar;

pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

pub(crate) fn window() -> impl Scene {
    bsn! {
        TradeWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(200), top: px(90), width: px(740),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1), border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [
            (
                TradeTitlebar
                Node { padding: {UiRect::axes(px(15), px(13))}, height: px(48) }
                ThemeBackgroundColor({crate::theme::feathers_theme::TOKEN_TITLEBAR_BG})
                Pickable
                on(drag_window::<TradeTitlebar, TradeWindowRoot>)
                Children [
                    (TradeWindowTitle Text("Trade")
                     TextFont { font: FontSourceTemplate::Handle(theme::FONT_TITLE), font_size: {FontSize::Px(16.0)} }
                     TextColor({theme::TEXT}) ignore_picking()),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Row, column_gap: px(10), padding: {UiRect::all(px(12))} }
                ignore_picking()
                Children [
                    bag_pane(),
                    own_pane(),
                    partner_pane(),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), padding: {UiRect::axes(px(14), px(8))} }
                Children [
                    chrome_text("Zeny".to_string(), 12.0, theme::TEXT),
                    (
                        TradeZenyField
                        EditableText
                        template_value(EditableTextFilter::new(|c| c.is_ascii_digit()))
                        TabIndex(0)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.0)} }
                        TextColor({theme::TEXT})
                        Node { width: px(160), height: px(30), padding: {UiRect::axes(px(8), px(5))}, border: px(1), border_radius: BorderRadius::all(px(6)) }
                        BackgroundColor({theme::FIELD}) BorderColor::all(theme::STROKE)
                    ),
                ]
            ),
            footer(),
            (TradeOverlayHost Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0) } ignore_picking()),
            (TradeErrorHost Node { position_type: PositionType::Absolute, top: px(320), left: px(14), min_height: px(20) } ignore_picking()),
        ]
    }
}

fn bag_pane() -> impl Scene {
    bsn! {
        TradeBagHost
        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), height: px(290), flex_direction: FlexDirection::Column, row_gap: px(4), padding: {UiRect::all(px(6))}, border: px(1), overflow: {Overflow::scroll_y()} }
        ScrollArea BackgroundColor({theme::FIELD}) BorderColor::all(theme::STROKE) Pickable
    }
}

fn own_pane() -> impl Scene {
    bsn! {
        TradeOwnHost
        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), height: px(290), flex_direction: FlexDirection::Column, row_gap: px(4), padding: {UiRect::all(px(6))}, border: px(1), overflow: {Overflow::scroll_y()} }
        ScrollArea BackgroundColor({theme::FIELD}) BorderColor::all(theme::STROKE) Pickable
    }
}

fn partner_pane() -> impl Scene {
    bsn! {
        TradePartnerHost
        Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), height: px(290), flex_direction: FlexDirection::Column, row_gap: px(4), padding: {UiRect::all(px(6))}, border: px(1), overflow: {Overflow::scroll_y()} }
        ScrollArea BackgroundColor({theme::FIELD}) BorderColor::all(theme::STROKE) Pickable
    }
}

pub(crate) fn bag_header() -> impl Scene {
    bsn! {
        Node { width: percent(100), height: px(24) }
        ignore_picking()
        Children [ chrome_text("Bag".to_string(), 12.0, theme::TEXT_DIM) ]
    }
}

pub(crate) fn pane_header(label: &'static str, zeny: u64, locked: bool) -> impl Scene {
    let badge = if locked { "  • Locked" } else { "" };
    bsn! {
        Node { width: percent(100), height: px(24) }
        ignore_picking()
        Children [ chrome_text(format!("{label}: {zeny} z{badge}"), 10.0, theme::GOLD) ]
    }
}

fn item_row(name: String, icon: String, amount: u32, refine: u32, identified: bool) -> impl Scene {
    let details = format!(
        "x{amount}{}{}",
        if refine > 0 {
            format!("  +{refine}")
        } else {
            String::new()
        },
        if identified { "" } else { "  Unidentified" }
    );
    bsn! {
        Node {
            height: px(48), width: percent(100), flex_shrink: 0.0,
            flex_direction: FlexDirection::Row, align_items: AlignItems::Center,
            column_gap: px(6), padding: {UiRect::horizontal(px(5))},
            border: px(1), border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor({theme::GLASS}) BorderColor::all(theme::STROKE)
        Children [
            (ImageNode { image: {icon} } Node { width: px(32), height: px(32), flex_shrink: 0.0 } ignore_picking()),
            (
                Node { flex_direction: FlexDirection::Column, min_width: px(0), overflow: {Overflow::clip()} }
                ignore_picking()
                Children [ chrome_text(name, 10.0, theme::TEXT), chrome_text(details, 9.0, theme::TEXT_DIM) ]
            ),
        ]
    }
}

fn item_view(id: u32, identified: bool, db: &ItemDb) -> (String, String) {
    let name = db
        .name(id, identified)
        .expect("trade item must have a name")
        .to_string();
    let icon = item_icon_path(
        db.icon_resource(id, identified)
            .expect("trade item must have an icon"),
    );
    (name, icon)
}

pub(crate) fn bag_cell(item: &Item, db: &ItemDb, enabled: bool) -> impl Scene {
    let (name, icon) = item_view(item.item_id, item.identified, db);
    let index = item.index;
    let amount = u32::from(item.amount);
    let refine = u32::from(item.refine);
    let identified = item.identified;
    let label = if enabled { "" } else { "Unavailable" };
    bsn! {
        template_value(TradeBagCell(index))
        Node { width: percent(100), height: px(48), flex_shrink: 0.0 }
        Pickable
        on(on_bag_click)
        Children [
            item_row(name, icon, amount, refine, identified),
            chrome_text(label.to_string(), 8.0, theme::TEXT_FAINT),
        ]
    }
}

pub(crate) fn offer_cell(
    item: &ZoneInventoryItem,
    db: &ItemDb,
    index: Option<u32>,
    enabled: bool,
) -> impl Scene {
    let (name, icon) = item_view(item.nameid, item.identified, db);
    let amount = item.amount;
    let refine = item.refine;
    let identified = item.identified;
    let label = if enabled { "Remove" } else { "" };
    bsn! {
        Node { width: percent(100), height: px(48), flex_shrink: 0.0 }
        Pickable
        Children [
            item_row(name, icon, amount, refine, identified),
            {index.map(|index| bevy::scene::EntityScene(own_marker(index)))},
            chrome_text(label.to_string(), 8.0, theme::TEXT_FAINT),
        ]
    }
}

fn own_marker(index: u32) -> impl Scene {
    bsn! {
        template_value(TradeOwnCell(index))
        Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute }
        Pickable
        on(on_own_click)
    }
}

fn footer() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, column_gap: px(8), padding: {UiRect::all(px(12))} }
        Children [
            (
                TradeLockButton
                @FeathersButton { @caption: bsn! { chrome_text("OK".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(92), height: px(34) } on(on_lock)
            ),
            (
                TradeConfirmButton
                @FeathersButton { @caption: bsn! { chrome_text("Trade".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(92), height: px(34) } on(on_confirm)
            ),
            (
                TradeCancelButton
                @FeathersButton { @caption: bsn! { chrome_text("Cancel".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(92), height: px(34) } on(on_cancel)
            ),
        ]
    }
}

pub(crate) fn error_message(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(11.0)} }
        TextColor({theme::BAD})
        ignore_picking()
    }
}

pub(crate) fn amount_prompt(prompt: AmountPrompt) -> impl Scene {
    bsn! {
        Node { width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.55)}) Pickable
        Children [
            (
                Node { width: px(270), flex_direction: FlexDirection::Column, row_gap: px(10), padding: {UiRect::all(px(18))}, border: px(1), border_radius: BorderRadius::all(px(9)) }
                BackgroundColor({theme::GLASS_2}) BorderColor::all(theme::STROKE_STRONG)
                Children [
                    chrome_text(format!("Offer amount (max {})", prompt.max), 12.0, theme::TEXT),
                    (
                        TradeAmountField
                        template_value(EditableText::new("1"))
                        template_value(EditableTextFilter::new(|c| c.is_ascii_digit()))
                        AutoFocus
                        TabIndex(0)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.0)} }
                        TextColor({theme::TEXT})
                        Node { height: px(32), padding: {UiRect::axes(px(9), px(6))}, border: px(1) }
                        BackgroundColor({theme::FIELD}) BorderColor::all(theme::STROKE)
                    ),
                    (
                        Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
                        Children [
                            (
                                TradeAmountConfirm
                                @FeathersButton { @caption: bsn! { chrome_text("Add".to_string(), 11.0, theme::TEXT) } }
                                Node { flex_grow: 1.0, height: px(32) } on(on_amount_confirm)
                            ),
                            (
                                TradeAmountCancel
                                @FeathersButton { @caption: bsn! { chrome_text("Cancel".to_string(), 11.0, theme::TEXT) } }
                                Node { flex_grow: 1.0, height: px(32) } on(on_amount_cancel)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}
