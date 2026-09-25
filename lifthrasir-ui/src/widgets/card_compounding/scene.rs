//! The server-driven card target picker, built as a single BSN modal.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::{ButtonVariant, FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::chrome::ignore_picking;
use crate::widgets::info_modal::INFO_MODAL_Z;

use super::{CardPickerAction, CardPickerRoot, ConfirmEnabled, on_action, on_backdrop_click};

pub(super) struct CardRowView {
    pub index: u32,
    pub label: String,
    pub icon_path: Option<String>,
    pub selected: bool,
}

pub(super) fn window(
    card_name: String,
    rows: Vec<CardRowView>,
    pending: bool,
    can_confirm: bool,
) -> impl Scene {
    let empty = rows.is_empty();
    let entries: Vec<_> = rows.into_iter().map(target_row).collect();
    let empty_text = empty.then(|| EntityScene(text("No compatible equipment available.".into())));
    let status = pending.then(|| EntityScene(text("Compounding card…".into())));
    bsn! {
        CardPickerRoot
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.016, 0.031, 0.027, 0.5)})
        GlobalZIndex({INFO_MODAL_Z})
        Pickable
        on(on_backdrop_click)
        Children [
            (
                Node {
                    width: px(390),
                    max_height: percent(80),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: {UiRect::all(px(16))},
                    border: px(1),
                    border_radius: BorderRadius::all(px(12)),
                }
                ThemeBackgroundColor({TOKEN_WINDOW_BG})
                ThemeBorderColor({TOKEN_WINDOW_BORDER})
                Pickable
                on(|mut click: On<Pointer<Click>>| click.propagate(false))
                Children [
                    title(format!("Compound {card_name}")),
                    text("Select equipment for this card.".into()),
                    (
                        Node { height: px(280), min_height: px(0), position_type: PositionType::Relative }
                        ignore_picking()
                        Children [
                            (
                                #viewport
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0), right: px(0), top: px(0), bottom: px(0),
                                    overflow: {Overflow::scroll_y()},
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(4),
                                }
                                ScrollArea
                                Pickable
                                Children [ {entries}, {empty_text} ]
                            ),
                            @FeathersScrollbar { @target: #viewport, @orientation: {ControlOrientation::Vertical} }
                            Node { position_type: PositionType::Absolute, top: px(0), bottom: px(0), right: px(0), width: px(6) }
                        ]
                    ),
                    {status},
                    actions(can_confirm),
                ]
            )
        ]
    }
}

fn target_row(view: CardRowView) -> impl Scene {
    let bg = if view.selected {
        theme::EMERALD_INK
    } else {
        theme::FIELD
    };
    let icon = view.icon_path.map(|path| EntityScene(item_icon(path)));
    let label = if view.selected {
        format!("✓ {}", view.label)
    } else {
        view.label
    };
    bsn! {
        @FeathersButton { @caption: bsn! {
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8) }
            ignore_picking()
            Children [ {icon}, text(label) ]
        } }
        template_value(CardPickerAction::Select(view.index))
        template_value(ButtonVariant::Plain)
        Node { width: percent(100), min_height: px(36), justify_content: JustifyContent::FlexStart, padding: {UiRect::horizontal(px(8))} }
        BackgroundColor(bg)
        on(on_action)
    }
}

fn item_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node { width: px(26), height: px(26) }
        ignore_picking()
    }
}

fn actions(can_confirm: bool) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, column_gap: px(8) }
        ignore_picking()
        Children [
            action_button("Cancel", CardPickerAction::Cancel, true),
            (
                @FeathersButton { @caption: bsn! { text("Confirm".into()) } }
                template_value(CardPickerAction::Confirm)
                template_value(ConfirmEnabled(can_confirm))
                template_value(ButtonVariant::Primary)
                Node { min_width: px(90), height: px(30) }
                on(on_action)
            ),
        ]
    }
}

fn action_button(label: &str, action: CardPickerAction, enabled: bool) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { text(label.to_string()) } }
        template_value(action)
        template_value(ConfirmEnabled(enabled))
        template_value(ButtonVariant::Normal)
        Node { min_width: px(80), height: px(30) }
        on(on_action)
    }
}

fn title(value: String) -> impl Scene {
    bsn! {
        Text(value)
        TextFont { font: FontSourceTemplate::Handle("ro://fonts/cinzel.ttf"), font_size: {FontSize::Px(16.0)} }
        ThemeTextColor({TOKEN_TEXT})
        ignore_picking()
    }
}

fn text(value: String) -> impl Scene {
    bsn! {
        Text(value)
        TextFont { font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"), font_size: {FontSize::Px(12.0)} }
        ThemeTextColor({TOKEN_TEXT_DIM})
        ignore_picking()
    }
}
