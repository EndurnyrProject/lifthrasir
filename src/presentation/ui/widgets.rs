use super::theme::*;
use bevy::prelude::*;

// Helper functions to create common UI component bundles

// Label bundle
pub fn ro_label(text: impl Into<String>) -> impl Bundle {
    ro_label_styled(text, FONT_SIZE_LABEL, TEXT_PRIMARY, SPACING_XS)
}

pub fn ro_label_styled(
    text: impl Into<String>,
    font_size: f32,
    color: Color,
    margin_bottom: f32,
) -> impl Bundle {
    (
        Text::new(text.into()),
        TextFont::from_font_size(font_size),
        TextColor(color),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::bottom(Val::Px(margin_bottom)),
            ..default()
        },
        RoText,
    )
}

// Title bundles
pub fn ro_title(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text.into()),
        TextFont::from_font_size(FONT_SIZE_TITLE),
        TextColor(TEXT_GOLD),
        Node {
            margin: UiRect::bottom(Val::Px(SPACING_XS)),
            ..default()
        },
        RoText,
    )
}

pub fn ro_subtitle(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text.into()),
        TextFont::from_font_size(FONT_SIZE_SUBTITLE),
        TextColor(TEXT_PRIMARY),
        Node {
            margin: UiRect::bottom(Val::Px(SPACING_XXL)),
            ..default()
        },
        RoText,
    )
}

// Input bundle
pub fn ro_text_input() -> impl Bundle {
    ro_text_input_styled(INPUT_HEIGHT, ELEMENT_SPACING)
}

pub fn ro_text_input_styled(height: f32, margin_bottom: f32) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            border: UiRect::all(Val::Px(BORDER_WIDTH)),
            padding: UiRect::all(Val::Px(INPUT_PADDING)),
            margin: UiRect::bottom(Val::Px(margin_bottom)),
            ..default()
        },
        BackgroundColor(INPUT_BACKGROUND_TRANSPARENT),
        BorderColor(INPUT_BORDER),
        BorderRadius::all(Val::Px(RADIUS_SM)),
        RoInput,
    )
}

// Button bundle
pub fn ro_button(text: impl Into<String>) -> impl Bundle {
    ro_button_styled(text, None, BUTTON_HEIGHT)
}

pub fn ro_button_with_width(text: impl Into<String>, width: f32) -> impl Bundle {
    ro_button_styled(text, Some(width), BUTTON_HEIGHT)
}

pub fn ro_button_styled(text: impl Into<String>, width: Option<f32>, height: f32) -> impl Bundle {
    let node_width = match width {
        Some(w) => Val::Px(w),
        None => Val::Auto,
    };

    (
        Button,
        Node {
            width: node_width,
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(SPACING_XL)),
            ..default()
        },
        BackgroundColor(BUTTON_NORMAL_TRANSPARENT),
        BorderRadius::all(Val::Px(RADIUS_MD)),
        RoButton,
    )
}

// Panel bundles with configurable size and transparency
pub fn ro_panel() -> impl Bundle {
    ro_panel_preset(PANEL_SIZE_FULL, PANEL_BACKGROUND_LIGHT)
}

pub fn ro_panel_small() -> impl Bundle {
    ro_panel_preset(PANEL_SIZE_SMALL, PANEL_BACKGROUND_MEDIUM)
}

pub fn ro_panel_medium() -> impl Bundle {
    ro_panel_preset(PANEL_SIZE_MEDIUM, PANEL_BACKGROUND_LIGHT)
}

pub fn ro_panel_large() -> impl Bundle {
    ro_panel_preset(PANEL_SIZE_LARGE, PANEL_BACKGROUND_SUBTLE)
}

pub fn ro_panel_preset(size: (f32, f32), background_color: Color) -> impl Bundle {
    ro_panel_custom(size.0, size.1, background_color)
}

pub fn ro_panel_sized(width: f32, height: f32) -> impl Bundle {
    ro_panel_custom(width, height, BACKGROUND_SECONDARY_TRANSPARENT)
}

pub fn ro_panel_custom(width: f32, height: f32, background_color: Color) -> impl Bundle {
    (
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(WINDOW_PADDING)),
            border: UiRect::all(Val::Px(BORDER_WIDTH * 2.0)),
            ..default()
        },
        BackgroundColor(background_color),
        BorderColor(BORDER_COLOR),
        BorderRadius::all(Val::Px(RADIUS_LG)),
        RoPanel,
    )
}

// Checkbox bundle (just the container)
pub fn ro_checkbox_container() -> impl Bundle {
    (Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        margin: UiRect::vertical(Val::Px(ELEMENT_SPACING)),
        ..default()
    },)
}

// Checkbox box bundle
pub fn ro_checkbox_box() -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(CHECKBOX_SIZE),
            height: Val::Px(CHECKBOX_SIZE),
            border: UiRect::all(Val::Px(BORDER_WIDTH)),
            margin: UiRect::right(Val::Px(SPACING_SM)),
            ..default()
        },
        BackgroundColor(INPUT_BACKGROUND_TRANSPARENT),
        BorderColor(INPUT_BORDER),
        BorderRadius::all(Val::Px(RADIUS_SM)),
    )
}
