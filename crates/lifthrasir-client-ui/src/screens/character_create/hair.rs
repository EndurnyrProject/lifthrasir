//! The hair styles and colours on offer, found by scanning the `ro://` content rather
//! than fixed in code, so content packs can add their own.
//!
//! A head sprite `data/sprite/인간족/머리통/<sex>/<n>_<sex>.spr` is a style, and a palette
//! `data/palette/머리/머리<n>_<sex>_<c>.pal` is a colour of that style. Colour 0 is the
//! sprite's own palette, so every style has it. The style tiles and colour swatches
//! are drawn from those same sprites and palettes.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use lifthrasir_assets::{
    Upscaling,
    animation::RoAnimationProcessor,
    loaders::{RoPaletteAsset, RoSpriteAsset},
    paths,
    source::{AssetSource, SharedCompositeAssetSource},
};
use lifthrasir_client::domain::settings::GraphicsSettings;

use super::*;

const HEAD_DIR: &str = "data/sprite/인간족/머리통/";
const PALETTE_PREFIX: &str = "data/palette/머리/머리";

/// The hair styles per sex and the palette colours per style found in the content.
#[derive(Resource, Default, Debug, PartialEq)]
pub(super) struct HairCatalog {
    styles: HashMap<Gender, Vec<u16>>,
    palettes: HashMap<(u16, Gender), Vec<u16>>,
}

impl HairCatalog {
    /// Builds the catalog from content paths, ignoring everything that is not a human
    /// head sprite or a hair palette.
    pub(super) fn scan<'a>(files: impl IntoIterator<Item = &'a str>) -> Self {
        let mut catalog = Self::default();
        for file in files {
            if let Some((style, sex)) = head_sprite(file) {
                catalog.styles.entry(sex).or_default().push(style);
            } else if let Some((style, sex, color)) = hair_palette(file) {
                catalog
                    .palettes
                    .entry((style, sex))
                    .or_default()
                    .push(color);
            }
        }
        let lists = catalog
            .styles
            .values_mut()
            .chain(catalog.palettes.values_mut());
        for list in lists {
            list.sort_unstable();
            list.dedup();
        }
        catalog
    }

    /// The styles `sex` can wear, ascending.
    pub(super) fn styles(&self, sex: Gender) -> &[u16] {
        self.styles.get(&sex).map_or(&[], Vec::as_slice)
    }

    /// Colour 0 and every colour `style` has a palette for, ascending.
    pub(super) fn colors(&self, style: u16, sex: Gender) -> Vec<u16> {
        let palettes = self.palettes.get(&(style, sex)).into_iter().flatten();
        std::iter::once(0)
            .chain(palettes.copied().filter(|&color| color > 0))
            .collect()
    }

    /// The style `delta` steps from `current` in `sex`'s list, wrapping at both ends.
    /// `current` is returned unchanged when the list does not hold it.
    pub(super) fn step_style(&self, sex: Gender, current: u16, delta: i32) -> u16 {
        let styles = self.styles(sex);
        let Some(index) = styles.iter().position(|&style| style == current) else {
            return current;
        };
        let next = cycle(index as u16, delta, 0, styles.len() as u16 - 1);
        styles[usize::from(next)]
    }
}

fn sex_named(name: &str) -> Option<Gender> {
    [Gender::Male, Gender::Female]
        .into_iter()
        .find(|sex| sex.as_korean() == name)
}

/// `data/sprite/인간족/머리통/남/7_남.spr` is style 7 for a male.
fn head_sprite(path: &str) -> Option<(u16, Gender)> {
    let (folder, file) = path.strip_prefix(HEAD_DIR)?.split_once('/')?;
    let (style, sex) = file.strip_suffix(".spr")?.split_once('_')?;
    if folder != sex {
        return None;
    }
    Some((style.parse().ok()?, sex_named(sex)?))
}

/// `data/palette/머리/머리7_남_3.pal` is colour 3 of style 7 for a male.
fn hair_palette(path: &str) -> Option<(u16, Gender, u16)> {
    let name = path.strip_prefix(PALETTE_PREFIX)?.strip_suffix(".pal")?;
    let mut parts = name.split('_');
    let style = parts.next()?.parse().ok()?;
    let sex = sex_named(parts.next()?)?;
    let color = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((style, sex, color))
}

/// Holds the hair style grid. `.0` is the list it currently shows.
#[derive(Component, Default, Clone)]
pub(super) struct StyleGrid(Vec<u16>);

/// Holds the hair colour grid. `.0` is the list it currently shows.
#[derive(Component, Default, Clone)]
pub(super) struct ColorGrid(Vec<u16>);

/// The "7 / 45" label between the hair style steppers.
#[derive(Component, Default, Clone)]
pub(super) struct HairStyleCount;

/// The head image of the hair style tile for style `.0`.
#[derive(Component, Clone)]
#[require(HeadArt)]
pub(super) struct HairThumb(pub(super) u16);

/// The swatch of hair colour `.0`, filled with that colour's tone.
#[derive(Component, Clone)]
#[require(HeadArt)]
pub(super) struct HairSwatch(pub(super) u16);

/// A head: its hair style, sex and hair colour.
type Head = (u16, Gender, u16);

/// The sprite and palette a [`HairThumb`] or [`HairSwatch`] is drawn from. The handles
/// live here because a scene component cannot hold them.
#[derive(Component, Default, Clone)]
pub(super) struct HeadArt {
    pub(super) head: Option<Head>,
    pub(super) sprite: Handle<RoSpriteAsset>,
    pub(super) palette: Option<Handle<RoPaletteAsset>>,
    /// The loaded sprite and palette have been drawn.
    pub(super) drawn: bool,
}

impl HeadArt {
    /// Requests the sprite and the palette of `head`. Hair colour 0 is the sprite's own
    /// palette, as for in-world heads.
    fn request(&mut self, head: Head, assets: &AssetServer) {
        let (style, sex, color) = head;
        self.head = Some(head);
        self.sprite = assets.load(paths::head_sprite_path(sex, style));
        self.palette =
            (color > 0).then(|| assets.load(paths::hair_palette_path(style, sex, color)));
        self.drawn = false;
    }

    /// The sprite and its hair palette, once both are loaded.
    fn loaded<'a>(
        &self,
        assets: &'a HeadAssets,
    ) -> Option<(&'a RoSpriteAsset, Option<&'a RoPaletteAsset>)> {
        let sprite = assets.sprites.get(&self.sprite)?;
        let palette = match &self.palette {
            Some(handle) => Some(assets.palettes.get(handle)?),
            None => None,
        };
        Some((sprite, palette))
    }
}

/// The colours a head is drawn with: its hair palette, else the sprite's own.
fn tones_of<'a>(sprite: &'a RoSpriteAsset, palette: Option<&'a RoPaletteAsset>) -> &'a [[u8; 4]] {
    match (palette, &sprite.sprite.palette) {
        (Some(palette), _) => &palette.colors,
        (None, Some(own)) => &own.colors,
        (None, None) => &[],
    }
}

/// How many of a head's `pixels` use each palette entry that holds its hair. Which
/// entries those are differs per style, so they are found as the ones the style's
/// `palettes` disagree on: the entries a hair colour changes. With nothing to compare,
/// every used entry counts. Entry 0 is transparent.
fn hair_weights(pixels: &[u8], palettes: &[&[[u8; 4]]]) -> [u32; 256] {
    let mut used = [0u32; 256];
    for &pixel in pixels {
        used[usize::from(pixel)] += 1;
    }
    used[0] = 0;
    let rgb = |palette: &[[u8; 4]], index: usize| palette.get(index).map(|&[r, g, b, _]| [r, g, b]);
    let mut hair = used;
    for (index, weight) in hair.iter_mut().enumerate() {
        let varies = palettes
            .windows(2)
            .any(|pair| rgb(pair[0], index) != rgb(pair[1], index));
        if !varies {
            *weight = 0;
        }
    }
    if hair.iter().all(|&weight| weight == 0) {
        return used;
    }
    hair
}

/// The average of `palette`'s colours over the weighted entries: the hair's overall
/// tone. `None` when nothing is weighted.
fn hair_tone(palette: &[[u8; 4]], weights: &[u32; 256]) -> Option<Color> {
    let mut sum = [0u64; 3];
    let mut total = 0;
    for (color, &weight) in palette.iter().zip(weights) {
        let weight = u64::from(weight);
        for (channel, &value) in sum.iter_mut().zip(color) {
            *channel += weight * u64::from(value);
        }
        total += weight;
    }
    let [red, green, blue] = sum.map(|channel| channel.checked_div(total));
    Some(Color::srgb_u8(red? as u8, green? as u8, blue? as u8))
}

/// The asset stores head sprites and hair palettes load into.
#[derive(SystemParam)]
pub(super) struct HeadAssets<'w> {
    server: Res<'w, AssetServer>,
    sprites: Res<'w, Assets<RoSpriteAsset>>,
    palettes: Res<'w, Assets<RoPaletteAsset>>,
}

/// Scans the content for hair styles and colours, once per run.
///
/// NOTE: this lists every content path (about 255k for a retail pak), a one-off hitch on
/// the first visit. Give the sources a prefix listing if it shows.
pub(super) fn scan_hair_catalog(
    mut catalog: ResMut<HairCatalog>,
    source: Res<SharedCompositeAssetSource>,
) {
    if !catalog.styles.is_empty() {
        return;
    }
    let Ok(content) = source.0.read() else {
        error!("The ro:// content index is unavailable: no hair styles to offer");
        return;
    };
    let files = content.list_files();
    *catalog = HairCatalog::scan(files.iter().map(String::as_str));
    if catalog.styles.is_empty() {
        error!("No head sprites under ro://{HEAD_DIR}: no hair styles to offer");
    }
}

/// Keeps the form's hair on what the catalog offers and rebuilds a grid when the list
/// it shows changes: the styles with the sex, the colours with the style.
pub(super) fn sync_hair_grids(
    mut commands: Commands,
    catalog: Res<HairCatalog>,
    mut form: ResMut<CreationForm>,
    mut styles: Single<(Entity, &mut StyleGrid), Without<ColorGrid>>,
    mut colors: Single<(Entity, &mut ColorGrid), Without<StyleGrid>>,
    mut count: Single<&mut Text, With<HairStyleCount>>,
) {
    let sex = form.0.sex;
    let offered = catalog.styles(sex);
    if !offered.contains(&form.0.hair_style)
        && let Some(&first) = offered.first()
    {
        form.0.hair_style = first;
    }
    let style = form.0.hair_style;
    let palette = catalog.colors(style, sex);
    if !palette.contains(&form.0.hair_color) {
        form.0.hair_color = 0;
    }

    let (grid, shown) = &mut *styles;
    if shown.0 != offered {
        shown.0 = offered.to_vec();
        commands.entity(*grid).despawn_children();
        commands
            .spawn_scene(scene::style_grid(offered))
            .insert(ChildOf(*grid));
    }
    let (grid, shown) = &mut *colors;
    if shown.0 != palette {
        commands.entity(*grid).despawn_children();
        commands
            .spawn_scene(scene::color_grid(&palette))
            .insert(ChildOf(*grid));
        shown.0 = palette;
    }

    let position = offered.iter().position(|&offer| offer == style);
    let label = format!(
        "{} / {}",
        position.map_or(0, |index| index + 1),
        offered.len()
    );
    if count.0 != label {
        count.0 = label;
    }
}

/// Draws each hair style tile from the first frame of its head sprite (the front view)
/// in the form's sex and hair colour. A change of either requests the matching sprite
/// and palette, and the tile is redrawn once both are loaded.
pub(super) fn draw_hair_thumbs(
    form: Res<CreationForm>,
    settings: Option<Res<GraphicsSettings>>,
    assets: HeadAssets,
    mut images: ResMut<Assets<Image>>,
    mut thumbs: Query<(&HairThumb, &mut HeadArt, &mut ImageNode, &mut Node)>,
) {
    let upscaling = settings.map_or(Upscaling::Off, |settings| settings.upscaling);
    for (thumb, mut art, mut image, mut node) in &mut thumbs {
        let head = (thumb.0, form.0.sex, form.0.hair_color);
        if art.head != Some(head) {
            art.request(head, &assets.server);
        }
        if art.drawn {
            continue;
        }
        let Some((sprite, palette)) = art.loaded(&assets) else {
            continue;
        };
        art.drawn = true;
        let Some(frame) = sprite.sprite.frames.first() else {
            continue;
        };
        let drawn = RoAnimationProcessor::frame_to_image(
            frame,
            sprite.sprite.palette.as_ref(),
            palette,
            upscaling,
        );
        image.image = images.add(drawn);
        node.width = px(f32::from(frame.width));
        node.height = px(f32::from(frame.height));
    }
}

/// Fills the colour swatches with the hair tone each colour gives the form's style and
/// sex. It waits for every swatch's palette, because comparing them is what tells
/// which palette entries hold the hair ([`hair_weights`]).
pub(super) fn tint_hair_swatches(
    form: Res<CreationForm>,
    assets: HeadAssets,
    mut swatches: Query<(&HairSwatch, &mut HeadArt, &mut BackgroundColor)>,
) {
    for (swatch, mut art, _) in &mut swatches {
        let head = (form.0.hair_style, form.0.sex, swatch.0);
        if art.head != Some(head) {
            art.request(head, &assets.server);
        }
    }
    if swatches.iter().all(|(_, art, _)| art.drawn) {
        return;
    }
    let loaded: Option<Vec<_>> = swatches
        .iter()
        .map(|(_, art, _)| art.loaded(&assets))
        .collect();
    let Some(loaded) = loaded else {
        return;
    };
    let palettes: Vec<_> = loaded
        .iter()
        .map(|&(sprite, palette)| tones_of(sprite, palette))
        .collect();
    let weights = loaded
        .first()
        .and_then(|(sprite, _)| sprite.sprite.frames.first())
        .filter(|frame| !frame.is_rgba)
        .map(|frame| hair_weights(&frame.data, &palettes));

    for ((_, mut art, mut fill), palette) in swatches.iter_mut().zip(palettes) {
        art.drawn = true;
        let Some(tone) = weights.and_then(|weights| hair_tone(palette, &weights)) else {
            continue;
        };
        *fill = BackgroundColor(tone);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hair_tone_averages_the_entries_the_palettes_disagree_on() {
        // Entry 5 (skin) covers the most pixels; entries 9 and 3 (hair) are what a
        // colour changes, used by three pixels and one.
        let pixels = [0, 0, 0, 0, 5, 5, 5, 5, 9, 9, 9, 3];
        let mut blond = [[10, 10, 10, 0]; 16];
        let mut red = blond;
        blond[9] = [240, 200, 80, 0];
        red[9] = [200, 40, 40, 0];
        blond[3] = [200, 160, 40, 0];
        red[3] = [120, 0, 0, 0];

        let weights = hair_weights(&pixels, &[&blond, &red]);
        assert_eq!((weights[5], weights[9], weights[3]), (0, 3, 1));
        assert_eq!(
            hair_tone(&blond, &weights),
            Some(Color::srgb_u8(230, 190, 70))
        );
        assert_eq!(hair_tone(&red, &weights), Some(Color::srgb_u8(180, 30, 30)));

        // A lone palette has nothing to compare against, so every used entry counts.
        assert_eq!(hair_weights(&pixels, &[&blond])[5], 4);
        assert_eq!(hair_tone(&blond, &hair_weights(&[0, 0], &[&blond])), None);
    }
}
