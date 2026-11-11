//! BSN scenes of the character creation screen: the header, the preview stage and the
//! New Hero window. Behaviour lives in the parent module.

use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::placeholder::Placeholder;
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, INPUT_SHADOW, WINDOW_SHADOW, icon_button, input_frame, scroll_grid,
    title_text, window_title, window_titled,
};

use super::*;
use crate::screens::hero_chrome::{HeroSprite, hero_on_pedestal, screen_root};
use crate::screens::shared::{flush_button, link, pill};

/// Design width of the window, excluding the baked shadow. Its content is eight hair
/// style tiles wide plus the grid's `SCROLL_GRID_GUTTER`.
const FORM_WIDTH: f32 = 412.0;
/// Close to the native size of a head.
const TILE: f32 = 39.0;
const SWATCH: f32 = 28.0;
/// Rows of hair styles and hair colours shown before their grid scrolls.
const STYLE_ROWS: f32 = 4.0;
const COLOR_ROWS: f32 = 2.0;
/// Space above a section label, and between the label and its controls.
const SECTION_GAP: f32 = 22.0;
const LABEL_GAP: f32 = 8.0;
/// Display size of the preview, 0.875x its 2x render target.
const HERO_SIZE: Vec2 = Vec2::new(252.0, 392.0);

/// The whole screen. `target` is the preview render target and `place` names the world
/// and slot the hero is forged in.
pub(super) fn create_screen(target: Handle<Image>, place: String) -> impl Scene {
    bsn! {
        @screen_root()
        Children [
            Node { align_items: AlignItems::Center, column_gap: px(14) }
            Children [
                @link("Back to Heroes")
                on(|_: On<PointerClick>, mut next: ResMut<NextState<GameState>>| {
                    next.set(GameState::CharacterSelection);
                })
                --
                @title_text("Forge a Hero", 22.0)
                TextShadow { offset: Vec2::splat(2.0), color: {Color::BLACK.with_alpha(0.85)} }
                --
                @pill(place)
            ]
            --
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                align_items: AlignItems::Center,
                column_gap: px(24),
            }
            Children [
                @stage(target)
                --
                @form_window()
            ]
        ]
    }
}

/// The preview on its pedestal, under the name being typed and over the Turn Around and
/// Surprise Me actions.
fn stage(target: Handle<Image>) -> impl Scene {
    let sprite = HeroSprite {
        image: target,
        rect: None,
        tint: Color::WHITE,
        size: HERO_SIZE,
    };
    let halo = ro_dark::FROST_GLOW.with_alpha(0.32);
    bsn! {
        Node {
            flex_grow: 1.0,
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(12),
        }
        Children [
            Node {
                padding: {UiRect::axes(px(8), px(2))},
                border_radius: BorderRadius::all(px(3)),
            }
            BackgroundColor({Color::BLACK.with_alpha(0.35)})
            Children [
                NameText::Tag
                @chrome_text(String::new(), 14.0, ro_dark::STRONG)
            ]
            --
            @hero_on_pedestal(sprite, halo)
            --
            Node { column_gap: px(8) }
            Children [
                @flush_button(ButtonArt::NORMAL, "Turn Around")
                Node { width: px(124.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                on(turn_around)
                --
                @flush_button(ButtonArt::NORMAL, "Surprise Me")
                Node { width: px(124.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                on(randomize_form)
            ]
            --
            @chrome_text("Starts as a Novice · All stats begin at 1".into(), 12.0, ro_dark::LABEL)
            TextShadow { offset: Vec2::splat(1.0), color: {Color::BLACK.with_alpha(0.9)} }
        ]
    }
}

/// The New Hero window. `sync_hair_grids` fills its two hair grids from the catalog.
fn form_window() -> impl Scene {
    bsn! {
        @window_titled()
        Node {
            width: px(FORM_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Children [
            @window_title("New Hero")
            --
            @section("NAME")
            --
            @name_field()
            --
            NameText::Status
            @chrome_text(String::new(), 11.0, ro_dark::MUTED)
            Node { min_height: px(15) }
            --
            @section("BODY")
            Node { margin: {UiRect::new(px(0), px(0), px(SECTION_GAP), px(LABEL_GAP))} }
            --
            Node { column_gap: px(4) }
            Children [
                @segment(Gender::Female, "Female")
                --
                @segment(Gender::Male, "Male")
            ]
            --
            Node {
                align_items: AlignItems::Center,
                column_gap: px(6),
                margin: {UiRect::new(px(0), px(0), px(SECTION_GAP), px(LABEL_GAP))},
            }
            Children [
                @section("HAIR STYLE")
                --
                Node { flex_grow: 1.0 }
                --
                @step_button("chevron-left")
                on(|_: On<Activate>, mut form: ResMut<CreationForm>, catalog: Res<HairCatalog>| {
                    step_hair_style(&mut form.0, &catalog, -1);
                })
                --
                Node { min_width: px(52), justify_content: JustifyContent::Center }
                Children [
                    HairStyleCount
                    @chrome_text(String::new(), 12.0, ro_dark::STRONG)
                ]
                --
                @step_button("chevron-right")
                on(|_: On<Activate>, mut form: ResMut<CreationForm>, catalog: Res<HairCatalog>| {
                    step_hair_style(&mut form.0, &catalog, 1);
                })
            ]
            --
            StyleGrid
            Node { flex_direction: FlexDirection::Column }
            --
            @section("HAIR COLOUR")
            Node { margin: {UiRect::new(px(0), px(0), px(SECTION_GAP), px(LABEL_GAP))} }
            --
            ColorGrid
            Node { flex_direction: FlexDirection::Column }
            --
            Node { align_items: AlignItems::Center, margin: {UiRect::top(px(SECTION_GAP))} }
            Children [
                @flush_button(ButtonArt::NORMAL, "Cancel")
                Node { width: px(88.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                on(|_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                    next.set(GameState::CharacterSelection);
                })
                --
                Node { flex_grow: 1.0 }
                --
                CreateButton
                @flush_button(ButtonArt::PRIMARY, "Forge Hero")
                Node { width: px(150.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                on(|_: On<Activate>, mut commands: Commands| commands.trigger(ForgeHero))
            ]
        ]
    }
}

/// A small-caps section label.
fn section(label: &'static str) -> impl Scene {
    chrome_text(label.into(), 10.0, ro_dark::ACCENT)
}

/// The name input: the editable text with its placeholder, and the character count.
/// The negative margin cancels the frame's baked shadow.
fn name_field() -> impl Scene {
    bsn! {
        NameFrame
        @input_frame()
        Node {
            column_gap: px(8),
            margin: {UiRect::axes(px(-INPUT_SHADOW), px(5.0 - INPUT_SHADOW))},
        }
        Children [
            #NameInput
            NameField
            EditableText { max_characters: {Some(NAME_MAX)} }
            TextInput
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_BODY),
                font_size: {FontSize::Px(13.0)},
            }
            TextColor(ro_dark::STRONG)
            Node { flex_grow: 1.0, height: px(20) }
            Children [
                Placeholder(#NameInput)
                @chrome_text("Name your hero".into(), 13.0, ro_dark::FAINT)
                Node { position_type: PositionType::Absolute }
            ]
            --
            NameText::Count
            @chrome_text(String::new(), 11.0, ro_dark::FAINT)
        ]
    }
}

/// The shared look of an option. [`style_choices`] recolours it on hover and selection.
fn choice_tile(choice: Choice) -> impl Scene {
    let (fill, ring) = choice_style(false, false);
    bsn! {
        choice
        Hovered
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor(fill)
        BorderColor::all(ring)
        BoxShadow
        on(pick_choice)
    }
}

/// One half of the body selector.
fn segment(sex: Gender, label: &'static str) -> impl Scene {
    bsn! {
        @choice_tile(Choice::Sex(sex))
        Node { flex_grow: 1.0, flex_basis: px(0), height: px(30) }
        Children [ @chrome_text(label.into(), 12.0, ro_dark::BUTTON_LABEL) ]
    }
}

/// The scrolling grid of the hair styles on offer.
pub(super) fn style_grid(styles: &[u16]) -> impl Scene + use<> {
    let tiles = styles.iter().copied().map(style_tile).collect();
    scroll_grid(tiles, TILE, STYLE_ROWS, 5.0)
}

/// The scrolling grid of the hair colours on offer.
pub(super) fn color_grid(colors: &[u16]) -> impl Scene + use<> {
    let swatches = colors.iter().copied().map(swatch).collect();
    scroll_grid(swatches, SWATCH, COLOR_ROWS, 6.0)
}

/// A hair style tile. [`draw_hair_thumbs`] fills it with the style's head at native
/// size; a head larger than the tile is cropped around its top centre, where the face is.
fn style_tile(style: u16) -> impl Scene {
    let thumb = HairThumb(style);
    bsn! {
        @choice_tile(Choice::HairStyle(style))
        Node {
            width: px(TILE),
            height: px(TILE),
            align_items: AlignItems::FlexStart,
            padding: {UiRect::top(px(2))},
            overflow: {Overflow::clip()},
        }
        Children [
            thumb
            ImageNode
            Node { flex_shrink: 0.0 }
            @ignore_picking()
        ]
    }
}

/// A hair colour diamond. [`tint_hair_swatches`] fills it with the colour's hair tone.
fn swatch(color: u16) -> impl Scene {
    let choice = Choice::HairColor(color);
    let swatch = HairSwatch(color);
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    bsn! {
        Node {
            width: px(SWATCH),
            height: px(SWATCH),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        Children [
            choice
            swatch
            Hovered
            tilt
            Node { width: px(18), height: px(18), border: px(2) }
            BackgroundColor(ro_dark::SLOT)
            BorderColor::all(ro_dark::SLOT_STROKE)
            BoxShadow
            on(pick_choice)
        ]
    }
}

/// A square art button with a chevron, for stepping through the hair styles.
fn step_button(icon: &'static str) -> impl Scene {
    bsn! {
        @icon_button(ButtonArt::NORMAL, icon)
        Node {
            width: px(28.0 + 2.0 * BUTTON_SHADOW),
            height: px(28.0 + 2.0 * BUTTON_SHADOW),
            flex_grow: 0.0,
            margin: {UiRect::all(px(-BUTTON_SHADOW))},
        }
    }
}
