//! BSN scenes of the character selection screen: the frame, the hero stage, the Hero
//! Codex, the slot cards and the delete modal. Behaviour lives in the parent module.

use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_ui::theme;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    BUTTON_SHADOW, ButtonArt, INPUT_SHADOW, WINDOW_SHADOW, display_text, input_frame, title_text,
    window_title, window_titled,
};

use super::*;
use crate::screens::character_preview::{COLUMN_PX, ROW_PX, SELECT_SCALE};
use crate::screens::hero_chrome::{HeroSprite, hero_on_pedestal, screen_root};
use crate::screens::shared::{flush_button, link, pill, usable};

/// Design sizes, excluding the baked window shadow.
const CODEX_WIDTH: f32 = 352.0;
const CODEX_HEIGHT: f32 = 440.0;
const MODAL_WIDTH: f32 = 420.0;
const CARD_SIZE: Vec2 = Vec2::new(92.0, 116.0);
const THUMB_HEIGHT: f32 = 72.0;
/// Art tint of a hero scheduled for deletion.
const DOOMED_TINT: Color = Color::srgba(0.48, 0.52, 0.50, 0.58);

/// The static frame: header, the empty showcase and strip, and the strip caption.
/// `used` of `cap` creatable slots hold a hero.
pub(super) fn frame(world: String, username: String, used: usize, cap: u8) -> impl Scene {
    bsn! {
        @screen_root()
        Children [
            @header(world, username)
            --
            Showcase
            Node { flex_grow: 1.0, min_height: px(0) }
            --
            Node { align_items: AlignItems::Center, column_gap: px(10) }
            Children [
                @chrome_text("HEROES".into(), 10.0, ro_dark::ACCENT)
                --
                @chrome_text(format!("{used} / {cap}"), 11.0, ro_dark::STRONG)
                --
                Node { flex_grow: 1.0 }
                --
                @chrome_text("Left / Right to choose · Enter to play".into(), 11.0, ro_dark::FAINT)
            ]
            --
            SlotStrip
            Node { column_gap: px(8) }
        ]
    }
}

fn header(world: String, username: String) -> impl Scene {
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(14) }
        Children [
            @title_text("Choose Your Hero", 22.0)
            TextShadow { offset: Vec2::splat(2.0), color: {Color::BLACK.with_alpha(0.85)} }
            --
            @pill(world)
            --
            @link("Change World")
            on(|_: On<PointerClick>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::ServerSelection);
            })
            --
            Node { flex_grow: 1.0 }
            --
            @chrome_text(username, 12.0, ro_dark::LABEL)
            --
            @flush_button(ButtonArt::NORMAL, "Log Out")
            Node { width: px(96.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
            on(|_: On<Activate>, mut next: ResMut<NextState<GameState>>| {
                next.set(GameState::Login);
            })
        ]
    }
}

/// The hero's preview within the shared render target, `height` px tall.
fn sprite(diorama: &CharacterDiorama, slot: usize, doomed: bool, height: f32) -> HeroSprite {
    HeroSprite {
        image: diorama.target.clone().unwrap_or_default(),
        rect: diorama.columns.get(&(slot as u8)).copied(),
        tint: if doomed { DOOMED_TINT } else { Color::WHITE },
        size: Vec2::new(height * COLUMN_PX as f32 / ROW_PX as f32, height),
    }
}

/// The stage and the codex for the selected slot, or their empty states for a roster
/// without heroes.
pub(super) fn showcase(
    data: &CharacterSelectionData,
    diorama: &CharacterDiorama,
    selected: usize,
    deletion_error: Option<&str>,
) -> impl Scene + use<> {
    let hero = featured(&data.characters, selected);
    let stage: Vec<_> = hero
        .map(|info| {
            let doomed = info.base.delete_date != 0;
            hero_stage(
                info,
                sprite(diorama, selected, doomed, ROW_PX as f32 * SELECT_SCALE),
            )
        })
        .into_iter()
        .collect();
    let empty_stage: Vec<_> = hero.is_none().then(empty_stage).into_iter().collect();
    let codex: Vec<_> = hero
        .map(|info| codex_hero(info, deletion_error))
        .into_iter()
        .collect();
    let empty_codex: Vec<_> = hero
        .is_none()
        .then(|| codex_empty(selected))
        .into_iter()
        .collect();
    bsn! {
        Node { flex_grow: 1.0, align_items: AlignItems::Center, column_gap: px(24) }
        Children [
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            Children [ {stage} -- {empty_stage} ]
            --
            @window_titled()
            Node {
                width: px(CODEX_WIDTH + 2.0 * WINDOW_SHADOW),
                min_height: px(CODEX_HEIGHT + 2.0 * WINDOW_SHADOW),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Children [
                @window_title("Hero Codex")
                --
                {codex}
                --
                {empty_codex}
            ]
        ]
    }
}

fn hero_stage(info: &CharacterInfoWithJobName, sprite: HeroSprite) -> impl Scene + use<> {
    let halo = if info.base.delete_date != 0 {
        ro_dark::STATUS_FULL.with_alpha(0.35)
    } else {
        ro_dark::FROST_GLOW.with_alpha(0.32)
    };
    let name = info.base.name.clone();
    let line = format!(
        "{} · Base {} · Job {}",
        info.job_name, info.base.base_level, info.base.job_level
    );
    bsn! {
        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center }
        Children [
            @hero_on_pedestal(sprite, halo)
            --
            @display_text(name, 30.0, ro_dark::HERO)
            TextShadow { offset: Vec2::splat(2.0), color: {Color::BLACK.with_alpha(0.85)} }
            Node { margin: {UiRect::top(px(10))} }
            --
            @chrome_text(line, 13.0, ro_dark::LABEL)
            TextShadow { offset: Vec2::splat(1.0), color: {Color::BLACK.with_alpha(0.9)} }
        ]
    }
}

fn empty_stage() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
        }
        Children [
            @title_text("No Heroes Yet", 22.0)
            TextShadow { offset: Vec2::splat(2.0), color: {Color::BLACK.with_alpha(0.85)} }
            --
            @chrome_text("Pick New Hero below to forge your first.".into(), 13.0, ro_dark::LABEL)
            TextShadow { offset: Vec2::splat(1.0), color: {Color::BLACK.with_alpha(0.9)} }
        ]
    }
}

/// The codex of a hero: identity, vitals, attributes, where it was last seen, and the
/// Rename / Delete / Enter actions. `deletion_error` is the server's refusal of the
/// last deletion request for this hero.
fn codex_hero(info: &CharacterInfoWithJobName, deletion_error: Option<&str>) -> impl Scene + use<> {
    let hero = &info.base;
    let doomed = hero.delete_date != 0;
    let character_id = hero.char_id;
    let cap = job_level_cap(hero.class);
    let name = hero.name.clone();
    let job = info.job_name.clone();
    let base = format!("Base {}", hero.base_level);
    let job_level = format!("Job {}", hero.job_level);
    let bars = vec![
        bar(
            "HP",
            thousands(hero.hp),
            fraction(hero.hp, hero.max_hp),
            ro_dark::HP,
        ),
        bar(
            "SP",
            thousands(hero.sp),
            fraction(hero.sp, hero.max_sp),
            ro_dark::SP,
        ),
        bar(
            "JOB LEVEL",
            format!("{} / {cap}", hero.job_level),
            job_level_fraction(hero.job_level, cap),
            ro_dark::JOB_EXP,
        ),
    ];
    let stats: Vec<_> = [
        ("STR", hero.str),
        ("AGI", hero.agi),
        ("VIT", hero.vit),
        ("INT", hero.int),
        ("DEX", hero.dex),
        ("LUK", hero.luk),
    ]
    .into_iter()
    .map(|(key, value)| stat(key, value))
    .collect();
    let last_seen = map_label(&hero.last_map);
    let zeny = format!("{} z", thousands(hero.zeny.into()));
    let nudge: Vec<_> = nudge(hero)
        .map(|line| chrome_text(line, 11.0, ro_dark::FROST_GLOW))
        .into_iter()
        .collect();
    let doom: Vec<_> = doomed
        .then(|| doom_banner(hero.delete_date))
        .into_iter()
        .collect();
    let error: Vec<_> = deletion_error
        .map(|error| {
            chrome_text(
                format!("Deletion failed · {error}"),
                11.0,
                ro_dark::DANGER_GLOW,
            )
        })
        .into_iter()
        .collect();
    let can_delete = can_request_deletion(hero.delete_date);
    let enter_label = if doomed {
        "Deletion Pending"
    } else {
        "Enter Game"
    };
    bsn! {
        Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(12) }
        Children [
            Node { flex_direction: FlexDirection::Column, row_gap: px(4) }
            Children [
                Node { align_items: AlignItems::Baseline, column_gap: px(8) }
                Children [
                    @display_text(name, 20.0, ro_dark::HERO)
                    --
                    @chrome_text(job, 12.0, ro_dark::MUTED)
                ]
                --
                Node { column_gap: px(6) }
                Children [
                    @chip(base, ro_dark::BRONZE_LIGHT)
                    --
                    @chip(job_level, ro_dark::STATUS_ONLINE)
                ]
            ]
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(14), row_gap: px(10) }
            Children [ {bars} ]
            --
            Node { column_gap: px(4) }
            Children [ {stats} ]
            --
            Node { flex_direction: FlexDirection::Column, row_gap: px(5) }
            Children [
                @fact("Last seen", last_seen, ro_dark::STRONG)
                --
                @fact("Zeny", zeny, ro_dark::BRONZE_LIGHT)
            ]
            --
            {nudge}
            --
            Node { flex_grow: 1.0 }
            --
            {doom}
            --
            {error}
            --
            Node { column_gap: px(6) }
            Children [
                // NOTE: display only, the protocol has no rename command; wire it when one lands.
                @flush_button(ButtonArt::NORMAL, "Rename")
                @usable(false, Color::WHITE)
                --
                @flush_button(ButtonArt::NORMAL, "Delete")
                @usable(can_delete, DANGER_TINT)
                on(move |_: On<Activate>,
                         mut pending: ResMut<PendingDeletion>,
                         mut feedback: ResMut<DeletionFeedback>| {
                    feedback.clear();
                    arm_delete(&mut pending.0, character_id);
                })
            ]
            --
            @flush_button(ButtonArt::PRIMARY, enter_label)
            @usable(!doomed, Color::WHITE)
            Node { height: px(42.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
            on(|_: On<Activate>, mut commands: Commands| commands.trigger(EnterGame))
        ]
    }
}

fn codex_empty(slot: usize) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(6),
        }
        Children [
            @chrome_text(format!("SLOT {}", slot + 1), 10.0, ro_dark::ACCENT)
            --
            @title_text("No hero here", 18.0)
            --
            @chrome_text("Pick New Hero to forge one.".into(), 12.0, ro_dark::MUTED)
        ]
    }
}

fn chip(label: String, color: Color) -> impl Scene {
    bsn! {
        Node {
            height: px(20),
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(8))},
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor(ro_dark::SLOT)
        Children [ @chrome_text(label, 11.0, color) ]
    }
}

/// A labelled stat bar filled to `fill` (0..=1). Two bars share a row and an odd last
/// one fills it.
fn bar(key: &'static str, value: String, fill: f32, color: Color) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: percent(40),
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
        }
        Children [
            Node { justify_content: JustifyContent::SpaceBetween }
            Children [
                @chrome_text(key.into(), 10.0, ro_dark::MUTED)
                --
                @chrome_text(value, 10.0, ro_dark::STRONG)
            ]
            --
            Node {
                height: px(6),
                border: px(1),
                border_radius: BorderRadius::all(px(3)),
                overflow: {Overflow::clip()},
            }
            BackgroundColor(ro_dark::BAR_TROUGH)
            BorderColor::all(ro_dark::INSET_RULE)
            Children [
                Node { width: percent(fill * 100.0), height: percent(100) }
                BackgroundColor(color)
            ]
        ]
    }
}

/// One attribute cell of the six-wide grid.
fn stat(key: &'static str, value: u8) -> impl Scene {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::vertical(px(5))},
            border_radius: BorderRadius::all(px(4)),
        }
        BackgroundColor(ro_dark::INSET)
        Children [
            @chrome_text(key.into(), 9.0, ro_dark::ACCENT)
            --
            @chrome_text(value.to_string(), 14.0, ro_dark::STRONG)
        ]
    }
}

fn fact(key: &'static str, value: String, color: Color) -> impl Scene {
    bsn! {
        Node { justify_content: JustifyContent::SpaceBetween }
        Children [
            @chrome_text(key.into(), 12.0, ro_dark::MUTED)
            --
            @chrome_text(value, 12.0, color)
        ]
    }
}

/// The pending-deletion notice. `tick_doom_countdown` fills the time left.
fn doom_banner(delete_date: u32) -> impl Scene {
    let countdown = DoomCountdown(delete_date);
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect::axes(px(10), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
        }
        BackgroundColor({ro_dark::STATUS_FULL.with_alpha(0.12)})
        BorderColor::all(ro_dark::STATUS_FULL.with_alpha(0.5))
        Children [
            @chrome_text("DELETING IN".into(), 10.0, ro_dark::DANGER_GLOW)
            --
            countdown
            @chrome_text(String::new(), 14.0, ro_dark::STRONG)
        ]
    }
}

/// The shared look of a slot card. [`style_cards`] recolours it on hover and selection.
fn card_frame(card: SlotCard) -> impl Scene {
    let (fill, ring, _) = card_style(&card, false, false);
    bsn! {
        card
        Hovered
        Node {
            width: px(CARD_SIZE.x),
            height: px(CARD_SIZE.y),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::new(px(4), px(4), px(6), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(fill)
        BorderColor::all(ring)
        BoxShadow
        on(pick_card)
    }
}

/// An occupied slot: the hero's thumbnail, name and level.
pub(super) fn hero_card(
    slot: usize,
    info: &CharacterInfoWithJobName,
    diorama: &CharacterDiorama,
) -> impl Scene + use<> {
    let doomed = info.base.delete_date != 0;
    let card = SlotCard {
        slot,
        kind: SlotKind::Occupied,
        doomed,
    };
    let HeroSprite {
        image,
        rect,
        tint,
        size,
    } = sprite(diorama, slot, doomed, THUMB_HEIGHT);
    let name = info.base.name.clone();
    let (caption, caption_color) = if doomed {
        ("Deleting".to_string(), ro_dark::DANGER_GLOW)
    } else {
        (
            format!("Lv {}", info.base.base_level),
            ro_dark::BRONZE_LIGHT,
        )
    };
    bsn! {
        @card_frame(card)
        Children [
            ImageNode { image: {image}, rect: {rect}, color: {tint} }
            Node { width: px(size.x), height: px(size.y), flex_shrink: 0.0 }
            @ignore_picking()
            --
            @chrome_text(name, 12.0, ro_dark::STRONG)
            Node { max_width: percent(100), overflow: {Overflow::clip()} }
            TextLayout { linebreak: LineBreak::NoWrap }
            --
            @chrome_text(caption, 10.0, caption_color)
        ]
    }
}

/// The next free slot: opens the creation screen.
pub(super) fn new_hero_card(slot: usize) -> impl Scene {
    let card = SlotCard {
        slot,
        kind: SlotKind::NewHero,
        doomed: false,
    };
    let label = vacant_slot_label(SlotKind::NewHero, SlotTier::Normal, true);
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    let upright = UiTransform::from_rotation(Rot2::degrees(-45.0));
    bsn! {
        @card_frame(card)
        Children [
            Node {
                width: px(30),
                height: px(30),
                margin: {UiRect::top(px(20))},
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
            }
            tilt
            BorderColor::all(ro_dark::FROST_GLOW)
            @ignore_picking()
            Children [
                @glyph_icon("plus", 12.0, ro_dark::CHECK_MARK)
                upright
            ]
            --
            @chrome_text(label.into(), 11.0, ro_dark::CHECK_MARK)
            Node { margin: {UiRect::top(auto())} }
        ]
    }
}

/// A slot no hero can be forged in yet: empty behind the next free one, or locked
/// behind a paid tier.
pub(super) fn vacant_card(slot: usize, tier: SlotTier, can_create: bool) -> impl Scene {
    let card = SlotCard {
        slot,
        kind: SlotKind::Vacant,
        doomed: false,
    };
    let label = vacant_slot_label(SlotKind::Vacant, tier, can_create);
    let icon = if tier == SlotTier::Normal && can_create {
        "minus"
    } else {
        "lock"
    };
    let color = if tier == SlotTier::Premium {
        ro_dark::ACCENT
    } else {
        ro_dark::FAINT
    };
    bsn! {
        @card_frame(card)
        Children [
            @glyph_icon(icon, 18.0, ro_dark::STATUS_OFFLINE)
            Node { margin: {UiRect::top(px(26))} }
            --
            @chrome_text(label.into(), 11.0, color)
            Node { margin: {UiRect::top(auto())} }
        ]
    }
}

/// The delete confirmation: a click-eating backdrop under a small titled window that
/// asks for the hero's name.
pub(super) fn delete_modal(name: String) -> impl Scene {
    let body = format!(
        "{name} will be scheduled for deletion and removed for good once the server's \
         waiting period ends. Their items and zeny go with them."
    );
    let label = format!("Type {name} to confirm");
    let modal = DeleteModal { name };
    bsn! {
        modal
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({Color::srgba_u8(5, 7, 10, 158)})
        GlobalZIndex(1)
        Children [
            @window_titled()
            Node {
                width: px(MODAL_WIDTH + 2.0 * WINDOW_SHADOW),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
            }
            Children [
                @window_title("Delete Hero")
                --
                @chrome_text(body, 13.0, ro_dark::BODY)
                --
                @chrome_text(label, 11.0, ro_dark::MUTED)
                Node { margin: {UiRect::top(px(14))} }
                --
                @input_frame()
                Node { margin: {UiRect::axes(px(-INPUT_SHADOW), px(5.0 - INPUT_SHADOW))} }
                Children [
                    ConfirmField
                    EditableText { max_characters: {Some(24)} }
                    TextInput
                    TextFont {
                        font: FontSourceTemplate::Handle(theme::FONT_BODY),
                        font_size: {FontSize::Px(13.0)},
                    }
                    TextColor(ro_dark::BODY)
                    Node { flex_grow: 1.0, height: px(20) }
                ]
                --
                ConfirmHint
                @chrome_text(String::new(), 11.0, ro_dark::DANGER_GLOW)
                Node { min_height: px(15) }
                --
                Node {
                    column_gap: px(8),
                    margin: {UiRect::top(px(14))},
                    justify_content: JustifyContent::FlexEnd,
                }
                Children [
                    @flush_button(ButtonArt::NORMAL, "Cancel")
                    Node { width: px(88.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                    on(|_: On<Activate>, mut pending: ResMut<PendingDeletion>| pending.0 = None)
                    --
                    ConfirmButton
                    @flush_button(ButtonArt::NORMAL, "Delete")
                    Node { width: px(112.0 + 2.0 * BUTTON_SHADOW), flex_grow: 0.0 }
                    on(|_: On<Activate>, mut commands: Commands| commands.trigger(ConfirmDelete))
                ]
            ]
        ]
    }
}
