//! A minimal custom bar for the Console identity strip: a rounded dark track with
//! a `Val::Percent` fill and a value label alongside. `bevy_feathers` 0.19 ships no
//! progress-bar widget, so this follows the `character_info` `HudBar` fill-node
//! idiom (a track node + a `Val::Percent` fill). No knob, no interaction.

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};

use crate::theme;
use crate::widgets::chrome::ignore_picking;

const TRACK_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.42);

/// `ratio` clamped to `0.0..=1.0`, as a 0..=100 percentage for the fill width.
fn fill_percent(ratio: f32) -> f32 {
    ratio.clamp(0.0, 1.0) * 100.0
}

/// A small bar: a rounded dark track holding a `fill`-colored fill node sized to
/// `ratio` (clamped), with `label` (e.g. "40 / 40") to its right.
pub fn meter(ratio: f32, fill: Color, label: String) -> impl Scene {
    let pct = fill_percent(ratio);
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(9),
            flex_grow: 1.0,
        }
        ignore_picking()
        Children [
            (
                Node {
                    flex_grow: 1.0,
                    height: px(11),
                    border_radius: BorderRadius::all(px(6)),
                    overflow: {Overflow::clip()},
                }
                BackgroundColor(TRACK_BG)
                ignore_picking()
                Children [
                    (
                        Node {
                            width: {Val::Percent(pct)},
                            height: percent(100),
                            border_radius: BorderRadius::all(px(5)),
                        }
                        BackgroundColor(fill)
                        ignore_picking()
                    )
                ]
            ),
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(11.0)},
                }
                TextColor(theme::TEXT_DIM)
                Node { min_width: px(64) }
                ignore_picking()
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_percent_clamps_to_0_100() {
        assert_eq!(fill_percent(0.5), 50.0);
        assert_eq!(fill_percent(0.0), 0.0);
        assert_eq!(fill_percent(1.0), 100.0);
        assert_eq!(fill_percent(1.5), 100.0);
        assert_eq!(fill_percent(-0.2), 0.0);
    }
}
