use bevy::{prelude::*, ui_widgets::Activate};
use bevy_feathers::controls::{FeathersButton, FeathersTextInput};
use lifthrasir_ui::{
    theme::{GLASS, ro_dark},
    widgets::{chrome::chrome_text, primitives::title_text},
};

use super::{
    CareButton, ConfirmPanel, ConfirmText, HomunculusWindow, NameField, Portrait, StatusText,
    activate,
};

pub(super) fn button(label: &'static str, action: CareButton) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { @chrome_text(label.into(), 13.0, ro_dark::BUTTON_LABEL) } }
        action
        on(activate)
    }
}

pub(super) fn window() -> impl Scene {
    bsn! {
        HomunculusWindow
        Pickable
        GlobalZIndex(40)
        BackgroundColor(GLASS)
        Node { position_type: PositionType::Absolute, top: px(120), right: px(55), width: px(408), flex_direction: FlexDirection::Column, row_gap: px(9), padding: UiRect::all(px(12)), display: Display::None }
        Children [
            Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center }
            Children [ @title_text("Homunculus", 18.0) -- @button("Close", CareButton::Close) ]
            --
            Node { flex_direction: FlexDirection::Row, column_gap: px(5) }
            Children [ @tab("Status", super::HomunculusTab::Status) -- @tab("Skills", super::HomunculusTab::Skills) -- @tab("Tactics", super::HomunculusTab::Tactics) ]
            -- @status_page()
            -- @super::skills::page()
            -- @super::tactics::page()
        ]
    }
}

fn tab(label: &'static str, page: super::HomunculusTab) -> impl Scene {
    let pick = move |_: On<Activate>, mut ui: ResMut<super::OwnerUi>| {
        ui.tab = page;
    };
    bsn! {
        @FeathersButton { @caption: bsn! { @chrome_text(label.into(), 13.0, ro_dark::BUTTON_LABEL) } }
        on(pick)
    }
}

fn status_page() -> impl Scene {
    bsn! {
        super::HomunculusTab::Status
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        Children [
            Node { flex_direction: FlexDirection::Row, column_gap: px(10), align_items: AlignItems::Start }
            Children [
                Portrait ImageNode Node { width: px(90), height: px(90), display: Display::None }
                -- StatusText @chrome_text("Loading companion…".into(), 13.0, ro_dark::TITLE)
            ]
            --
            Node { flex_direction: FlexDirection::Row, column_gap: px(5), flex_wrap: FlexWrap::Wrap }
            Children [ @button("Refresh", CareButton::Inspect) -- @button("Follow", CareButton::Follow) -- @button("Standby", CareButton::Standby) -- @button("Move", CareButton::Move) -- @button("Attack", CareButton::Attack) -- @button("Feed", CareButton::Feed) -- @button("Rest", CareButton::Rest) ]
            --
            Node { flex_direction: FlexDirection::Row, column_gap: px(5) }
            Children [ @FeathersTextInput { @visible_width: {Some(18.0)}, @max_characters: {Some(23)} } NameField -- @button("Rename", CareButton::Rename) -- @button("Delete…", CareButton::Delete) ]
            --
            ConfirmPanel Node { display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(5) }
            Children [ ConfirmText @chrome_text(String::new(), 13.0, ro_dark::TITLE) -- Node { flex_direction: FlexDirection::Row, column_gap: px(5) } Children [ @button("Confirm", CareButton::Confirm) -- @button("Cancel", CareButton::Cancel) ] ]
        ]
    }
}

pub(super) fn context_menu(pos: Vec2) -> impl Scene {
    bsn! {
        super::OwnerMenu
        Pickable
        GlobalZIndex(120)
        Node { position_type: PositionType::Absolute, left: px(pos.x), top: px(pos.y), padding: UiRect::all(px(5)) }
        BackgroundColor(GLASS)
        Children [ @FeathersButton { @caption: bsn! { @chrome_text("Homunculus".into(), 13.0, ro_dark::BUTTON_LABEL) } }
            on(|_: On<Activate>, mut ui: ResMut<super::OwnerUi>| ui.open = true) ]
    }
}
