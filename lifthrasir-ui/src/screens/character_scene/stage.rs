use bevy::{color::Color, prelude::*, ui::ColorStop};

use crate::{
    screens::character_scene::tokens::{BEAM, RING, RING_THIN},
    theme,
};

fn centered_node(width: f32, height: f32, bottom: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(0),
        right: px(0),
        bottom: px(bottom),
        width: px(width),
        height: px(height),
        margin: UiRect::horizontal(auto()),
        ..default()
    }
}

pub fn spot_glow(hue: Color) -> impl Bundle {
    (
        centered_node(120.0, 120.0, 14.0),
        BackgroundGradient::from(RadialGradient {
            shape: RadialGradientShape::Circle(percent(50)),
            stops: vec![
                ColorStop::new(hue.with_alpha(0.55), percent(0)),
                ColorStop::new(Color::NONE, percent(66)),
            ],
            ..default()
        }),
        Pickable::IGNORE,
    )
}

pub fn spot_ring(assets: &AssetServer, hue: Color) -> impl Bundle + use<> {
    (
        ImageNode {
            image: assets.load(RING),
            color: hue.with_alpha(0.5),
            ..default()
        },
        centered_node(104.0, 30.0, 22.0),
        Pickable::IGNORE,
    )
}

pub fn spot_ring_thin(assets: &AssetServer, hue: Color) -> impl Bundle + use<> {
    (
        ImageNode {
            image: assets.load(RING_THIN),
            color: hue.with_alpha(0.34),
            ..default()
        },
        centered_node(72.0, 21.0, 22.0),
        Pickable::IGNORE,
    )
}

pub fn spot_beam(assets: &AssetServer, hue: Color) -> impl Bundle + use<> {
    (
        ImageNode {
            image: assets.load(BEAM),
            color: hue.with_alpha(0.17),
            ..default()
        },
        centered_node(104.0, 210.0, 26.0),
        Pickable::IGNORE,
    )
}

pub fn ground_shadow() -> impl Bundle {
    let mut node = centered_node(84.0, 11.0, 26.0);
    node.border_radius = BorderRadius::MAX;
    (
        node,
        BackgroundGradient::from(RadialGradient {
            shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
            stops: vec![
                ColorStop::new(Color::BLACK.with_alpha(0.5), percent(0)),
                ColorStop::new(Color::NONE, percent(100)),
            ],
            ..default()
        }),
        Pickable::IGNORE,
    )
}

pub fn horizon_line() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(28),
            right: px(14),
            bottom: px(116),
            height: px(1),
            ..default()
        },
        BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::new(Color::NONE, percent(0)),
            ColorStop::new(theme::GOLD.with_alpha(0.30), percent(15)),
            ColorStop::new(theme::GOLD.with_alpha(0.30), percent(85)),
            ColorStop::new(Color::NONE, percent(100)),
        ])),
        Pickable::IGNORE,
    )
}
