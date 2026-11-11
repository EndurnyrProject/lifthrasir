//! BSN furniture of the character selection and creation screens: the shaded screen
//! root and the pedestal a hero stands on. Helpers any screen can use live in
//! [`super::shared`].

use bevy::prelude::*;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;

const SHADE: Color = Color::srgb_u8(6, 8, 12);
const PEDESTAL: Vec2 = Vec2::new(260.0, 60.0);
/// How far above the sprite's bottom edge its feet are drawn.
const FOOT_INSET: f32 = 4.0;

/// The full-screen column both screens lay out in. Its gradients darken the menu
/// background towards the edges and the window side so text stays readable.
pub fn screen_root() -> impl Scene {
    let shade = BackgroundGradient(vec![
        LinearGradient::to_right(vec![
            ColorStop::new(SHADE.with_alpha(0.55), percent(0)),
            ColorStop::new(SHADE.with_alpha(0.2), percent(40)),
            ColorStop::new(SHADE.with_alpha(0.75), percent(72)),
        ])
        .into(),
        LinearGradient::to_bottom(vec![
            ColorStop::new(SHADE.with_alpha(0.5), percent(0)),
            ColorStop::new(SHADE.with_alpha(0.0), percent(25)),
            ColorStop::new(SHADE.with_alpha(0.0), percent(65)),
            ColorStop::new(SHADE.with_alpha(0.9), percent(100)),
        ])
        .into(),
    ]);
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(12),
            padding: {UiRect::axes(px(40), px(24))},
        }
        shade
    }
}

/// A hero preview: the render target, the hero's crop within it, the art tint and the
/// displayed size.
pub struct HeroSprite {
    pub image: Handle<Image>,
    pub rect: Option<Rect>,
    pub tint: Color,
    pub size: Vec2,
}

/// A hero standing on the bronze pedestal, lit from below in `halo`. The preview camera
/// frames a hero with the feet near the bottom edge, which is where the pedestal sits.
pub fn hero_on_pedestal(sprite: HeroSprite, halo: Color) -> impl Scene {
    let HeroSprite {
        image,
        rect,
        tint,
        size,
    } = sprite;
    let light = BackgroundGradient::from(RadialGradient {
        shape: RadialGradientShape::Ellipse(percent(50), percent(50)),
        stops: vec![
            ColorStop::new(halo, percent(0)),
            ColorStop::new(Color::NONE, percent(70)),
        ],
        ..default()
    });
    bsn! {
        Node {
            width: px(PEDESTAL.x),
            height: px(size.y - FOOT_INSET + PEDESTAL.y / 2.0),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
        }
        Children [
            @centered(PEDESTAL, 0.0)
            light
            --
            @centered(Vec2::new(236.0, 44.0), 8.0)
            Node { border: px(1.5), border_radius: BorderRadius::MAX_ELLIPTICAL }
            BackgroundColor({ro_dark::SCROLL_TRACK.with_alpha(0.75)})
            BorderColor::all(ro_dark::DIVIDER)
            --
            @centered(Vec2::new(192.0, 32.0), 14.0)
            Node { border: px(1), border_radius: BorderRadius::MAX_ELLIPTICAL }
            BorderColor::all(ro_dark::FROST_GLOW.with_alpha(0.35))
            --
            ImageNode { image: {image}, rect: {rect}, color: {tint} }
            Node { width: px(size.x), height: px(size.y) }
            @ignore_picking()
        ]
    }
}

/// A decoration centred on its parent's width, `bottom` px above its base.
fn centered(size: Vec2, bottom: f32) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(bottom),
            width: px(size.x),
            height: px(size.y),
            margin: {UiRect::horizontal(auto())},
        }
        @ignore_picking()
    }
}
