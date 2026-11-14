//! The name plate's scene.

use bevy::prelude::*;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{glyph_icon, ignore_picking};

use super::view::{GuildLine, PlateKind, PlateView};
use super::{PLATE_WIDTH, PLATE_Z, PlateEmblem, Vital};
use crate::screens::shared::outlined_text;
use crate::worldspace::{hung_root, plate_frame, world_bar};

const OWN_HP: Color = Color::srgb_u8(0xc4, 0x55, 0x4b);
const OWN_SP: Color = Color::srgb_u8(0x4a, 0x8a, 0xc8);
const PARTY_HP: Color = Color::srgb_u8(0x6f, 0xb3, 0x5a);
const EMBLEM_SIZE: f32 = 16.0;

/// One unit's plate: the hero's bars, the name line, the guild line and a party
/// member's health, hanging from the unit's feet.
pub(super) fn plate(view: PlateView) -> impl Scene {
    let kind = view.kind;
    let own_bars: Vec<_> = (kind == PlateKind::Own)
        .then(own_bars)
        .into_iter()
        .collect();
    let crown: Vec<_> = (kind == PlateKind::Boss)
        .then(|| glyph_icon("crown", 12.0, ro_dark::RARE_RING))
        .into_iter()
        .collect();
    let party: Vec<_> = view
        .party
        .into_iter()
        .map(|party| outlined_text(format!("({party})"), 11.0, ro_dark::BRONZE_LIGHT))
        .collect();
    let guild: Vec<_> = view.guild.into_iter().map(guild_line).collect();
    let party_bar: Vec<_> = (kind == PlateKind::Party)
        .then(|| vital_bar(Vec2::new(46.0, 4.0), Vital::Hp, PARTY_HP, 2.0))
        .into_iter()
        .collect();
    bsn! {
        @hung_root(PLATE_WIDTH, PLATE_Z)
        Children [
            @plate_frame(view.backed)
            Children [
                {own_bars}
                --
                Node { align_items: AlignItems::Center, column_gap: px(4) }
                @ignore_picking()
                Children [
                    {crown}
                    --
                    @outlined_text(view.name, 13.0, kind.name_color())
                    --
                    {party}
                ]
                --
                {guild}
                --
                {party_bar}
            ]
        ]
    }
}

/// The hero's health over their spirit, above the name.
fn own_bars() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(1),
            margin: {UiRect::bottom(px(2))},
        }
        @ignore_picking()
        Children [
            @vital_bar(Vec2::new(54.0, 4.0), Vital::Hp, OWN_HP, 0.0)
            --
            @vital_bar(Vec2::new(54.0, 3.0), Vital::Sp, OWN_SP, 0.0)
        ]
    }
}

/// A bar that `fill_vitals` keeps at the unit's `vital`, `lift` pixels under the
/// line above it.
fn vital_bar(size: Vec2, vital: Vital, color: Color, lift: f32) -> impl Scene {
    let ring = Color::BLACK.with_alpha(0.6);
    bsn! {
        @world_bar(size, ring, bsn! { vital BackgroundColor(color) })
        Node { margin: {UiRect::top(px(lift))} }
    }
}

/// The guild's emblem and name. The emblem takes no room until its image arrives.
fn guild_line(guild: GuildLine) -> impl Scene {
    let color = guild.color();
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(4) }
        @ignore_picking()
        Children [
            PlateEmblem
            ImageNode
            Node {
                width: px(EMBLEM_SIZE),
                height: px(EMBLEM_SIZE),
                display: Display::None,
            }
            @ignore_picking()
            --
            @outlined_text(guild.name, 11.0, color)
        ]
    }
}
