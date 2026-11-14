//! How a toast is drawn: the glyph tile, the kicker, title and sub line, its button,
//! the dismiss mark and the life bar.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{ButtonArt, ghost_button};

use super::{ShowToast, ToastButton, ToastCard, ToastLife, on_toast_button};
use crate::screens::shared::{overline, row_button};

const CARD_WIDTH: f32 = 320.0;

pub(super) fn card(id: u32, show: &ShowToast) -> impl Scene + use<> {
    let (tone, title_tone) = show.kind.tones();
    let slate = BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::new(ro_dark::TOOLTIP_BODY.with_alpha(0.96), percent(0)),
        ColorStop::new(
            Color::srgb_u8(0x1e, 0x23, 0x2a).with_alpha(0.96),
            percent(100),
        ),
    ]));
    let shadow = BoxShadow::new(
        Color::BLACK.with_alpha(0.6),
        Val::ZERO,
        px(6),
        Val::ZERO,
        px(14),
    );
    let sub = (!show.sub.is_empty())
        .then(|| EntityScene(chrome_text(show.sub.clone(), 11.0, ro_dark::MUTED)));
    let action = show.action.as_ref().map(|action| {
        let (label, width) = action.button();
        EntityScene(bsn! {
            Node { margin: {UiRect::top(px(6))} }
            @ignore_picking()
            Children [
                @row_button(ButtonArt::NORMAL, label, width)
                ToastButton { id, act: true }
                on(on_toast_button)
            ]
        })
    });
    bsn! {
        ToastCard
        Hovered
        Node {
            width: px(CARD_WIDTH),
            column_gap: px(10),
            padding: {UiRect::new(px(10), px(34), px(9), px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
            position_type: PositionType::Relative,
            overflow: {Overflow::clip()},
        }
        slate
        BorderColor::all(tone)
        shadow
        Pickable
        Children [
            Node {
                width: px(34),
                height: px(34),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
                border_radius: BorderRadius::all(px(6)),
            }
            BackgroundColor({tone.with_alpha(0.14)})
            BorderColor::all(tone)
            @ignore_picking()
            Children [ @glyph_icon(show.kind.icon(), 18.0, tone) ]
            --
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
            }
            @ignore_picking()
            Children [
                @overline(show.kind.kicker().into(), tone)
                --
                @chrome_text(show.title.clone(), 13.0, title_tone)
                --
                {sub}
                --
                {action}
            ]
            --
            @ghost_button("close")
            ToastButton { id, act: false }
            Node { position_type: PositionType::Absolute, right: px(5), top: px(5) }
            on(on_toast_button)
            --
            ToastLife(id)
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                bottom: px(0),
                width: percent(100),
                height: px(2),
            }
            BackgroundColor({tone.with_alpha(0.8)})
            @ignore_picking()
        ]
    }
}
