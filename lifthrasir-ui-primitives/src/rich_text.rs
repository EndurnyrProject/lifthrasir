use bevy::prelude::*;

/// Splits a Ragnarok `^RRGGBB`-coded string into colored runs.
///
/// `^RRGGBB` sets the color of the text that follows it. The RO reset code
/// `^000000` (black) maps back to `default` so descriptions stay readable on the
/// dark UI instead of turning literally black. A `^` not followed by six hex
/// digits is kept verbatim. Empty runs are dropped.
pub fn parse_color_codes(text: &str, default: Color) -> Vec<(Color, String)> {
    let mut runs: Vec<(Color, String)> = Vec::new();
    let mut current = default;
    let mut buffer = String::new();
    let mut chars = text.char_indices();

    while let Some((i, c)) = chars.next() {
        if c != '^' {
            buffer.push(c);
            continue;
        }
        match parse_hex_color(&text[i + 1..]) {
            Some(rgb) => {
                if !buffer.is_empty() {
                    runs.push((current, std::mem::take(&mut buffer)));
                }
                current = if rgb == [0, 0, 0] {
                    default
                } else {
                    Color::srgb_u8(rgb[0], rgb[1], rgb[2])
                };
                for _ in 0..6 {
                    chars.next();
                }
            }
            None => buffer.push('^'),
        }
    }
    if !buffer.is_empty() {
        runs.push((current, buffer));
    }
    runs
}

/// Spawns RO-colored text as a child of `parent`: a `Text` root holding the first
/// run plus a `TextSpan` child per following run. Returns the root entity. Use
/// this wherever item/chat/server text may carry inline `^RRGGBB` codes.
pub fn spawn_colored_text(
    commands: &mut Commands,
    parent: Entity,
    text: &str,
    font: Handle<Font>,
    size: f32,
    default: Color,
) -> Entity {
    let make_font = |font: &Handle<Font>| TextFont {
        font: font.clone().into(),
        font_size: size.into(),
        ..Default::default()
    };

    let mut runs = parse_color_codes(text, default).into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((default, String::new()));

    let root = commands
        .spawn((
            Text::new(first_text),
            make_font(&font),
            TextColor(first_color),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id();

    for (color, segment) in runs {
        commands.spawn((
            TextSpan::new(segment),
            make_font(&font),
            TextColor(color),
            ChildOf(root),
        ));
    }
    root
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.as_bytes();
    if hex.len() < 6 {
        return None;
    }
    let mut rgb = [0u8; 3];
    for channel in 0..3 {
        let hi = hex_val(hex[channel * 2])?;
        let lo = hex_val(hex[channel * 2 + 1])?;
        rgb[channel] = hi * 16 + lo;
    }
    Some(rgb)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: Color = Color::srgb_u8(0x9f, 0xb1, 0xa6);

    #[test]
    fn plain_text_is_one_run() {
        let runs = parse_color_codes("Restores 45 HP.", DEFAULT);
        assert_eq!(runs, vec![(DEFAULT, "Restores 45 HP.".to_string())]);
    }

    #[test]
    fn color_code_splits_runs() {
        let runs = parse_color_codes("Use ^ff0000fire^000000 here", DEFAULT);
        assert_eq!(
            runs,
            vec![
                (DEFAULT, "Use ".to_string()),
                (Color::srgb_u8(0xff, 0, 0), "fire".to_string()),
                (DEFAULT, " here".to_string()),
            ]
        );
    }

    #[test]
    fn reset_code_returns_to_default_not_black() {
        let runs = parse_color_codes("^000000reset", DEFAULT);
        assert_eq!(runs, vec![(DEFAULT, "reset".to_string())]);
    }

    #[test]
    fn leading_code_drops_empty_prefix() {
        let runs = parse_color_codes("^00ff00green", DEFAULT);
        assert_eq!(
            runs,
            vec![(Color::srgb_u8(0, 0xff, 0), "green".to_string())]
        );
    }

    #[test]
    fn invalid_caret_is_literal() {
        let runs = parse_color_codes("3 ^ 5 ^zzscore", DEFAULT);
        assert_eq!(runs, vec![(DEFAULT, "3 ^ 5 ^zzscore".to_string())]);
    }
}
