//! Korean glyphs for egui.
//!
//! egui's bundled fonts cover only Latin and Cyrillic, so RO's Korean asset
//! names render as boxes. The editor installs the first Korean system font it
//! finds as the lowest-priority fallback for every egui family.

use bevy::prelude::*;
use bevy_egui::{
    EguiContexts,
    egui::{
        FontData, FontFamily,
        epaint::text::{FontInsert, FontPriority, InsertFontFamily},
    },
};

/// Korean-capable system fonts, tried in order: `(path, face index)`.
const KOREAN_FONTS: &[(&str, u32)] = &[
    ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
    ("C:\\Windows\\Fonts\\malgun.ttf", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
    (
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        0,
    ),
    ("/usr/share/fonts/truetype/nanum/NanumGothic.ttf", 0),
];

/// Adds a Korean fallback font to the primary egui context, once.
///
/// Missing fonts only cost glyphs, so it warns instead of failing.
pub fn install_korean_font(mut contexts: EguiContexts, mut done: Local<bool>) -> Result {
    if *done {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    *done = true;
    let Some((path, index, bytes)) = KOREAN_FONTS
        .iter()
        .find_map(|&(path, index)| std::fs::read(path).ok().map(|bytes| (path, index, bytes)))
    else {
        warn!("no Korean system font found; Korean text renders as boxes");
        return Ok(());
    };
    info!("egui Korean fallback font: {path}");
    ctx.add_font(korean_font_insert(bytes, index));
    Ok(())
}

fn korean_font_insert(bytes: Vec<u8>, index: u32) -> FontInsert {
    let fallback = |family| InsertFontFamily {
        family,
        priority: FontPriority::Lowest,
    };
    FontInsert::new(
        "korean",
        FontData {
            index,
            ..FontData::from_owned(bytes)
        },
        vec![
            fallback(FontFamily::Proportional),
            fallback(FontFamily::Monospace),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui;

    #[test]
    fn the_korean_font_is_a_fallback_for_both_families() {
        let insert = korean_font_insert(vec![1, 2, 3], 2);

        assert_eq!(insert.data.index, 2);
        assert_eq!(insert.families.len(), 2);
        assert!(
            insert
                .families
                .iter()
                .all(|family| matches!(family.priority, FontPriority::Lowest))
        );
    }

    #[test]
    fn egui_renders_hangul_once_the_font_is_installed() {
        let Some(bytes) = KOREAN_FONTS
            .iter()
            .find_map(|&(path, _)| std::fs::read(path).ok())
        else {
            return;
        };
        let ctx = egui::Context::default();
        ctx.add_font(korean_font_insert(bytes, 0));
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();

        let has_glyph =
            ctx.fonts_mut(|fonts| fonts.has_glyph(&egui::FontId::proportional(14.0), '외'));
        assert!(has_glyph);
    }
}
