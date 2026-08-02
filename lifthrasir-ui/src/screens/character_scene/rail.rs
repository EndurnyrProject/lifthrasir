use bevy::{color::Color, prelude::*, ui::ColorStop};

use crate::{screens::character_scene::tokens::mono_label, theme};

pub fn rail_container() -> impl Bundle {
    (
        Node {
            width: px(452),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(px(34), px(30)),
            border: UiRect::left(px(1)),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::new(
                Color::srgba(9.0 / 255.0, 14.0 / 255.0, 12.0 / 255.0, 0.90),
                percent(0),
            ),
            ColorStop::new(
                Color::srgba(6.0 / 255.0, 10.0 / 255.0, 9.0 / 255.0, 0.94),
                percent(100),
            ),
        ])),
        BorderColor::all(theme::GOLD_FAINT),
        BoxShadow::new(
            Color::BLACK.with_alpha(0.9),
            px(-40),
            px(0),
            px(-40),
            px(90),
        ),
        Pickable::IGNORE,
    )
}

pub fn rail_header(
    assets: &AssetServer,
    label_text: &str,
    realm_name: &str,
) -> impl Bundle + use<> {
    let avatar: String = realm_name
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect();
    (
        Node {
            width: percent(100),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::bottom(px(26)),
            ..default()
        },
        Pickable::IGNORE,
        children![
            theme::label(
                mono_label(label_text),
                assets.load(theme::FONT_MONO),
                9.0,
                theme::TEXT_FAINT,
            ),
            (
                Node {
                    height: px(30),
                    align_items: AlignItems::Center,
                    column_gap: px(9),
                    padding: UiRect::horizontal(px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(7)),
                    ..default()
                },
                BackgroundColor(Color::WHITE.with_alpha(0.04)),
                BorderColor::all(theme::STROKE),
                Pickable::IGNORE,
                children![
                    (
                        Node {
                            width: px(22),
                            height: px(22),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(6)),
                            ..default()
                        },
                        BackgroundColor(theme::GLASS_2),
                        BorderColor::all(theme::GOLD_FAINT),
                        Pickable::IGNORE,
                        children![theme::label(
                            avatar,
                            assets.load(theme::FONT_TITLE),
                            12.0,
                            theme::GOLD,
                        )],
                    ),
                    theme::label(
                        format!("Realm: {realm_name}"),
                        assets.load(theme::FONT_BODY),
                        11.5,
                        theme::TEXT_DIM,
                    ),
                ],
            ),
        ],
    )
}

pub fn gold_rule() -> impl Bundle {
    (
        Node {
            width: percent(100),
            height: px(1),
            margin: UiRect::bottom(px(20)),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::new(Color::NONE, percent(0)),
            ColorStop::new(theme::GOLD_FAINT, percent(22)),
            ColorStop::new(theme::GOLD_FAINT, percent(78)),
            ColorStop::new(Color::NONE, percent(100)),
        ])),
        Pickable::IGNORE,
    )
}

pub fn section_label(assets: &AssetServer, text: &str) -> impl Bundle + use<> {
    (
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: px(12),
            margin: UiRect::top(px(30)).with_bottom(px(16)),
            ..default()
        },
        Pickable::IGNORE,
        children![
            theme::label(
                mono_label(text),
                assets.load(theme::FONT_MONO),
                9.0,
                theme::TEXT_FAINT,
            ),
            (
                Node {
                    flex_grow: 1.0,
                    height: px(1),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_right(vec![
                    ColorStop::new(theme::GOLD_FAINT, percent(0)),
                    ColorStop::new(Color::NONE, percent(100)),
                ])),
                Pickable::IGNORE,
            ),
        ],
    )
}
