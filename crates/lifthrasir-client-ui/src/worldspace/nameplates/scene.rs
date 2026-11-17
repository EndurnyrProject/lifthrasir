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
/// RO's emblem size: one native emblem pixel per screen pixel.
const EMBLEM_SIZE: f32 = 24.0;

/// One unit's plate, hanging from the unit's feet: the hero's bars, then the guild
/// emblem beside the name over the guild name, as RO draws it, then a party
/// member's health.
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
    let emblem: Vec<_> = view
        .guild
        .is_some()
        .then(plate_emblem)
        .into_iter()
        .collect();
    let guild: Vec<_> = view.guild.into_iter().map(guild_name).collect();
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
                    {emblem}
                    --
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexStart,
                    }
                    @ignore_picking()
                    Children [
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
                    ]
                ]
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

/// The guild emblem beside the name. It takes no room until its image arrives.
fn plate_emblem() -> impl Scene {
    bsn! {
        PlateEmblem
        ImageNode
        Node {
            width: px(EMBLEM_SIZE),
            height: px(EMBLEM_SIZE),
            display: Display::None,
        }
        @ignore_picking()
    }
}

/// The guild's name under the unit's name.
fn guild_name(guild: GuildLine) -> impl Scene {
    let color = guild.color();
    outlined_text(guild.name, 11.0, color)
}
