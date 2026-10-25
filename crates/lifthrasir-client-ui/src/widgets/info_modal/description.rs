//! Shared modal vellum descriptions and skill-specific prose cleanup.
//! Item stat lines stay authored; neither renderer changes dark chat text.

use bevy::color::Luminance;
use bevy::prelude::*;

use lifthrasir_ui::rich_text::parse_color_codes;
use lifthrasir_ui::widgets::chrome::ignore_picking;

use super::shell::{self, PARCHMENT as VELLUM};
use super::view::ColoredLine;

const INK: Color = Color::srgb_u8(0x30, 0x2b, 0x23);

pub(super) fn scene(lines: Vec<ColoredLine>) -> impl Scene {
    let lines = lines
        .into_iter()
        .map(|runs| {
            runs.into_iter()
                .map(|(color, text)| (vellum_ink(color), text))
                .collect()
        })
        .collect();
    bsn! {
        Node {
            width: percent(100),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            padding: px(16),
            border: px(1),
            border_radius: BorderRadius::all(px(12)),
        }
        BackgroundColor(VELLUM)
        BorderColor::all(lifthrasir_ui::theme::STROKE)
        @ignore_picking()
        Children [ @shell::description_section(lines) ]
    }
}

/// Keeps source hues but caps their luminance for at least 4.5:1 vellum contrast.
fn vellum_ink(color: Color) -> Color {
    let color = if color == lifthrasir_ui::theme::TEXT_DIM {
        INK
    } else {
        color
    };
    let max_luminance = (VELLUM.to_linear().luminance() + 0.05) / 4.6 - 0.05;
    let linear = color.to_linear();
    if linear.luminance() <= max_luminance {
        return color;
    }
    linear.with_luminance(max_luminance).into()
}

/// Drops redundant headings and joins the catalog's hard-wrapped description prose.
/// Requirement metadata and per-level rows keep their authored boundaries.
pub(super) fn parse(lines: &[String], name: &str) -> Vec<ColoredLine> {
    let mut rows: Vec<ColoredLine> = Vec::new();
    let mut in_description = false;
    for line in lines {
        let mut runs = parse_color_codes(line, INK);
        let plain: String = runs.iter().map(|(_, text)| text.as_str()).collect();
        if plain == name || plain.starts_with("Max Level:") {
            continue;
        }
        if plain
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .starts_with("Requirement:")
            && let Some((_, first)) = runs.first_mut()
        {
            *first = first
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .to_string();
            runs.retain(|(_, text)| !text.is_empty());
        }

        let is_description = plain.starts_with("Description:");
        let is_section = plain.is_empty()
            || plain.contains(':')
            || plain.trim_start().starts_with(['[', '>', '-', '*', '•'])
            || plain.starts_with("Requires ");
        if in_description
            && !is_section
            && let Some(previous) = rows.last_mut()
        {
            if let Some((_, text)) = previous.last_mut() {
                text.push(' ');
            }
            previous.extend(runs);
            continue;
        }
        in_description = is_description;
        rows.push(runs);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vellum_keeps_body_and_highlights_readable() {
        use bevy::scene::ScenePlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Font>();
        let lines = ["Body ^777777grey ^00ff00green ^ffffffwhite ^000099blue^000000 reset"]
            .map(str::to_string);
        app.world_mut()
            .spawn_scene(scene(parse(&lines, "Bash")))
            .unwrap();
        app.update();

        let background = VELLUM.to_linear().luminance();
        let colors: Vec<_> = app
            .world_mut()
            .query::<&TextColor>()
            .iter(app.world())
            .map(|color| color.0)
            .collect();
        assert_eq!(colors.len(), 6);
        assert_eq!(colors[0], INK);
        for color in colors {
            let contrast = (background + 0.05) / (color.to_linear().luminance() + 0.05);
            assert!(contrast >= 4.5, "{color:?}: contrast {contrast}");
        }
    }

    #[test]
    fn description_removes_duplicate_headers_and_reflows_only_prose() {
        let lines = [
            "Drum of Battlefield",
            "Max Level: 5",
            "19^CC3399Requirement: Apple of Idun 10^000000",
            "20^CC3399Requirement: Service For You 10^000000",
            "Range: ^77777731x31 AoE^000000",
            "Description: Increases ^CC6600ATK^000000 and DEF",
            "of yourself and party members.",
            "^CC6600 > ATK +20%",
            " > DEF +15%",
            "Requires Musical Instrument or Whip.",
            "[Lv 1]: ATK +20, DEF +15",
            "[Lv 2]: ATK +25, DEF +30",
            "10% bonus remains literal.",
        ]
        .map(str::to_string);

        let rows: Vec<String> = parse(&lines, "Drum of Battlefield")
            .into_iter()
            .map(|runs| runs.into_iter().map(|(_, text)| text).collect())
            .collect();

        assert_eq!(
            rows,
            [
                "Requirement: Apple of Idun 10",
                "Requirement: Service For You 10",
                "Range: 31x31 AoE",
                "Description: Increases ATK and DEF of yourself and party members.",
                " > ATK +20%",
                " > DEF +15%",
                "Requires Musical Instrument or Whip.",
                "[Lv 1]: ATK +20, DEF +15",
                "[Lv 2]: ATK +25, DEF +30",
                "10% bonus remains literal.",
            ]
        );
    }
}
