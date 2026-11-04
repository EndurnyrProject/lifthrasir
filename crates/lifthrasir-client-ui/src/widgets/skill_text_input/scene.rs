use super::{PromptAction, SkillTextInputField, SkillTextInputRoot, on_action};
use bevy::input_focus::{AutoFocus, tab_navigation::TabIndex};
use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::TextInput;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor};
use lifthrasir_ui::theme::feathers_theme::{
    TOKEN_PANEL_BG, TOKEN_PANEL_BORDER, TOKEN_TEXT, TOKEN_TEXT_DIM, TOKEN_TITLEBAR_BG,
    TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use lifthrasir_ui::widgets::chrome::ignore_picking;

pub(super) fn window(request_id: u64, skill_id: u32, max_utf8_bytes: u32) -> impl Scene {
    let title = match skill_id {
        220 => "Graffiti".to_string(),
        125 => "Talkie Box".to_string(),
        _ => format!("Skill #{skill_id}"),
    };
    bsn! {
        SkillTextInputRoot
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        GlobalZIndex(1100)
        @ignore_picking()
        Children [
            Node {
                width: px(420),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                border: px(1),
                border_radius: BorderRadius::all(px(9)),
            }
            ThemeBackgroundColor(TOKEN_WINDOW_BG)
            ThemeBorderColor(TOKEN_WINDOW_BORDER)
            Pickable
            Children [
                Node { padding: {UiRect::axes(px(14), px(9))} }
                ThemeBackgroundColor(TOKEN_TITLEBAR_BG)
                Children [ @label(title, false) ]
                --
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    padding: px(14),
                }
                Children [
                    @label("Enter a message:".to_string(), false)
                    --
                    SkillTextInputField
                    EditableText
                    TextInput
                    TabIndex(0)
                    AutoFocus
                    TextFont {
                        font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
                        font_size: {FontSize::Px(13.0)},
                    }
                    ThemeTextColor(TOKEN_TEXT)
                    Node {
                        height: px(32),
                        padding: {UiRect::axes(px(10), px(6))},
                        border: px(1),
                        border_radius: BorderRadius::all(px(6)),
                    }
                    ThemeBackgroundColor(TOKEN_PANEL_BG)
                    ThemeBorderColor(TOKEN_PANEL_BORDER)
                    --
                    @label(format!("Only the first {max_utf8_bytes} UTF-8 bytes will be sent."), true)
                    --
                    Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: px(8),
                    }
                    Children [
                        @button("Cancel", request_id, false)
                        --
                        @button("Confirm", request_id, true)
                    ]
                ]
            ]
        ]
    }
}

fn label(text: String, dim: bool) -> impl Scene {
    let token = if dim { TOKEN_TEXT_DIM } else { TOKEN_TEXT };
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("ro://fonts/manrope.ttf"),
            font_size: {FontSize::Px(13.0)},
        }
        ThemeTextColor(token)
        @ignore_picking()
    }
}

fn button(caption: &str, request_id: u64, confirm: bool) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { @label(caption.to_string(), false) } }
        PromptAction { request_id, confirm }
        Node { width: px(80), height: px(26) }
        on(on_action)
    }
}
