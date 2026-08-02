use bevy::{color::Color, prelude::*, ui::ColorStop};

use crate::screens::character_scene::tokens::GRAIN;
use crate::theme;

fn full_screen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: percent(100),
        height: percent(100),
        ..default()
    }
}

pub fn key_light(hue: Color) -> impl Bundle {
    (
        full_screen(),
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::new(Vec2::new(-0.16, -0.04), px(0), px(0)),
            shape: RadialGradientShape::Ellipse(percent(46), percent(58)),
            stops: vec![
                ColorStop::new(hue.with_alpha(0.26), percent(0)),
                ColorStop::new(Color::NONE, percent(70)),
            ],
            ..default()
        }),
        GlobalZIndex(-2),
        Pickable::IGNORE,
    )
}

pub fn gold_rim() -> impl Bundle {
    (
        full_screen(),
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::new(Vec2::new(0.24, 0.38), px(0), px(0)),
            shape: RadialGradientShape::Ellipse(percent(38), percent(44)),
            stops: vec![
                ColorStop::new(theme::GOLD.with_alpha(0.10), percent(0)),
                ColorStop::new(Color::NONE, percent(70)),
            ],
            ..default()
        }),
        GlobalZIndex(-2),
        Pickable::IGNORE,
    )
}

pub fn grade() -> impl Bundle {
    (
        full_screen(),
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::new(
                Color::srgba(3.0 / 255.0, 7.0 / 255.0, 6.0 / 255.0, 0.78),
                percent(0),
            ),
            ColorStop::new(
                Color::srgba(3.0 / 255.0, 7.0 / 255.0, 6.0 / 255.0, 0.12),
                percent(26),
            ),
            ColorStop::new(
                Color::srgba(3.0 / 255.0, 7.0 / 255.0, 6.0 / 255.0, 0.30),
                percent(60),
            ),
            ColorStop::new(
                Color::srgba(2.0 / 255.0, 5.0 / 255.0, 4.0 / 255.0, 0.92),
                percent(100),
            ),
        ])),
        GlobalZIndex(-2),
        Pickable::IGNORE,
    )
}

pub fn vignette() -> impl Bundle {
    (
        full_screen(),
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::new(Vec2::new(-0.10, -0.06), px(0), px(0)),
            shape: RadialGradientShape::Ellipse(percent(120), percent(90)),
            stops: vec![
                ColorStop::new(Color::NONE, percent(40)),
                ColorStop::new(
                    Color::srgba(2.0 / 255.0, 5.0 / 255.0, 4.0 / 255.0, 0.72),
                    percent(100),
                ),
            ],
            ..default()
        }),
        GlobalZIndex(-2),
        Pickable::IGNORE,
    )
}

pub fn grain(assets: &AssetServer) -> impl Bundle + use<> {
    (
        ImageNode {
            image: assets.load(GRAIN),
            color: Color::WHITE.with_alpha(0.15),
            image_mode: NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            },
            ..default()
        },
        full_screen(),
        GlobalZIndex(-1),
        Pickable::IGNORE,
    )
}
