//! The Character tab's equipment slot wells.
//!
//! Ported from the old `equipment_window/slots.rs` + its `scene.rs` slot chrome, but
//! self-contained: it owns its own UI-only types ([`CharEquipSlotKind`],
//! [`CharEquipSlotParts`], [`CharEquippedIndex`], [`CharLastSlotClick`]) so the old
//! window can be deleted whole in the integration task with zero dangling references.
//! Only the shared DOMAIN types + messages (`Inventory`, `Item`, `ItemDb`,
//! `UnequipItemRequested`, the `EQP_*` location constants, `item_icon_path`) and the
//! chrome/theme helpers are reused.
//!
//! Two columns of bordered slot wells sit either side of the preview: left column the
//! armor slots, right column the weapon/accessory slots. Each well's icon/refine sync
//! is gated on `Inventory` change; hovering a filled slot shows a `name · slot ·
//! +refine` tooltip; double-clicking a filled slot emits `UnequipItemRequested`.

use std::time::Duration;

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::equipment::location::{
    EQP_ARMOR, EQP_GARMENT, EQP_HEAD_LOW, EQP_HEAD_MID, EQP_HEAD_TOP, EQP_LEFT_ACCESSORY,
    EQP_LEFT_HAND, EQP_RIGHT_ACCESSORY, EQP_RIGHT_HAND, EQP_SHOES,
};
use game_engine::domain::equipment::UnequipItemRequested;
use game_engine::domain::inventory::{Inventory, Item};
use game_engine::infrastructure::item::ItemDb;

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_PANEL_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::{glyph_icon, ignore_picking};

/// Which paperdoll slot a well represents.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CharEquipSlotKind {
    #[default]
    HeadUpper,
    HeadMid,
    HeadLower,
    Body,
    Garment,
    RightHand,
    LeftHand,
    AccessoryRight,
    AccessoryLeft,
    Footgear,
}

impl CharEquipSlotKind {
    pub fn wear_bit(self) -> u32 {
        match self {
            CharEquipSlotKind::HeadUpper => EQP_HEAD_TOP,
            CharEquipSlotKind::HeadMid => EQP_HEAD_MID,
            CharEquipSlotKind::HeadLower => EQP_HEAD_LOW,
            CharEquipSlotKind::Body => EQP_ARMOR,
            CharEquipSlotKind::Garment => EQP_GARMENT,
            CharEquipSlotKind::RightHand => EQP_RIGHT_HAND,
            CharEquipSlotKind::LeftHand => EQP_LEFT_HAND,
            CharEquipSlotKind::AccessoryRight => EQP_RIGHT_ACCESSORY,
            CharEquipSlotKind::AccessoryLeft => EQP_LEFT_ACCESSORY,
            CharEquipSlotKind::Footgear => EQP_SHOES,
        }
    }

    /// Human-readable slot name for tooltips.
    pub fn slot_label(self) -> &'static str {
        match self {
            CharEquipSlotKind::HeadUpper => "Upper Headgear",
            CharEquipSlotKind::HeadMid => "Mid Headgear",
            CharEquipSlotKind::HeadLower => "Lower Headgear",
            CharEquipSlotKind::Body => "Body",
            CharEquipSlotKind::Garment => "Garment",
            CharEquipSlotKind::RightHand => "Right Hand",
            CharEquipSlotKind::LeftHand => "Left Hand",
            CharEquipSlotKind::AccessoryRight => "Accessory",
            CharEquipSlotKind::AccessoryLeft => "Accessory",
            CharEquipSlotKind::Footgear => "Footgear",
        }
    }
}

/// Child entities of a slot well that [`sync_console_equipment_slots`] patches when
/// the inventory changes: the empty-state glyph, the item-icon `ImageNode`, and the
/// refine badge text. `FromTemplate` lets the fields be captured from `#Name`
/// references inside the slot's `bsn!` scene.
#[derive(Component, FromTemplate, Clone)]
pub struct CharEquipSlotParts {
    pub glyph: Entity,
    pub icon: Entity,
    pub refine: Entity,
}

/// The inventory index of the item currently shown in a slot. Inserted when the slot
/// fills, removed when it empties; the double-click handler reads it to unequip.
#[derive(Component, Clone, Copy)]
pub struct CharEquippedIndex(pub u16);

/// A slot's place in a column: which kind it is and the empty-state glyph icon name.
#[derive(Clone, Copy)]
pub struct SlotSpec {
    pub kind: CharEquipSlotKind,
    pub glyph: &'static str,
}

const fn slot(kind: CharEquipSlotKind, glyph: &'static str) -> SlotSpec {
    SlotSpec { kind, glyph }
}

/// Left column, top-to-bottom: the five armor slots.
pub const LEFT_SLOTS: [SlotSpec; 5] = [
    slot(CharEquipSlotKind::HeadUpper, "head"),
    slot(CharEquipSlotKind::HeadMid, "headm"),
    slot(CharEquipSlotKind::HeadLower, "headl"),
    slot(CharEquipSlotKind::Body, "armor"),
    slot(CharEquipSlotKind::Garment, "garment"),
];

/// Right column, top-to-bottom: hands, accessories, footgear.
pub const RIGHT_SLOTS: [SlotSpec; 5] = [
    slot(CharEquipSlotKind::RightHand, "sword"),
    slot(CharEquipSlotKind::LeftHand, "shield"),
    slot(CharEquipSlotKind::AccessoryRight, "ring"),
    slot(CharEquipSlotKind::AccessoryLeft, "ring"),
    slot(CharEquipSlotKind::Footgear, "boot"),
];

// ---------------------------------------------------------------------------
// Pure helpers (unit-tested).
// ---------------------------------------------------------------------------

pub fn item_in_slot(inventory: &Inventory, kind: CharEquipSlotKind) -> Option<&Item> {
    inventory
        .equipped()
        .find(|item| item.wear_state & kind.wear_bit() != 0)
}

fn tooltip_text(name: &str, slot: &str, refine: u8) -> String {
    if refine > 0 {
        format!("{name} \u{00b7} {slot} \u{00b7} +{refine}")
    } else {
        format!("{name} \u{00b7} {slot}")
    }
}

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

/// Last slot click, for double-click detection.
#[derive(Resource, Default)]
pub struct CharLastSlotClick {
    index: u16,
    at: Duration,
}

fn is_slot_double_click(last: &CharLastSlotClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

fn item_name(item_db: Option<&ItemDb>, item: &Item) -> String {
    item_db
        .and_then(|db| db.name(item.item_id, item.identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", item.item_id))
}

// ---------------------------------------------------------------------------
// Scene: two slot columns.
// ---------------------------------------------------------------------------

/// One slot column (left or right).
pub fn slot_column(slots: &'static [SlotSpec]) -> impl Scene {
    let wells: Vec<_> = slots.iter().map(|spec| slot_well(*spec)).collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(6),
        }
        ignore_picking()
        Children [ {wells} ]
    }
}

/// One slot well: a bordered icon box holding the empty-state glyph, the item icon,
/// and the refine badge. The three patched children are named with `#` references and
/// captured into [`CharEquipSlotParts`]; the well itself carries the slot kind and the
/// hover / double-click observers. The item name shows on hover, not as a caption.
fn slot_well(spec: SlotSpec) -> impl Scene {
    bsn! {
        template_value(spec.kind)
        Node {
            width: px(40),
            height: px(40),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        ThemeBackgroundColor({TOKEN_PANEL_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        CharEquipSlotParts { glyph: #Glyph, icon: #Icon, refine: #Refine }
        on(on_slot_click)
        on(on_slot_hover_over)
        on(on_slot_hover_out)
        Children [
            ( #Glyph glyph_icon(spec.glyph, 22.0, theme::TEXT_FAINT) ),
            (
                #Icon
                ImageNode
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                }
                Visibility::Hidden
                ignore_picking()
            ),
            (
                #Refine
                Text("")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
                    font_size: {FontSize::Px(9.0)},
                }
                TextColor({theme::GOLD})
                Node { position_type: PositionType::Absolute, left: px(3), top: px(2) }
                Visibility::Hidden
                ignore_picking()
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Sync + observers.
// ---------------------------------------------------------------------------

fn set_visibility(visibilities: &mut Query<&mut Visibility>, entity: Entity, value: Visibility) {
    if let Ok(mut vis) = visibilities.get_mut(entity) {
        *vis = value;
    }
}

/// Patches every slot from live equipped state whenever the `Inventory` resource
/// changes: shows the item icon + refine badge on filled slots, restores the
/// empty-state glyph otherwise, and tracks the worn item's index so the double-click
/// handler can unequip it.
#[allow(clippy::too_many_arguments)]
pub fn sync_console_equipment_slots(
    mut commands: Commands,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    slots: Query<(Entity, &CharEquipSlotKind, &CharEquipSlotParts)>,
    added: Query<(), Added<CharEquipSlotParts>>,
    mut images: Query<&mut ImageNode>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    // Run on inventory change or when a fresh well appears: the body is built once
    // (deferred), so newly mounted slots must sync even if the inventory has settled.
    if !inventory.is_changed() && added.is_empty() {
        return;
    }
    let db = item_db.as_deref();
    for (entity, kind, parts) in &slots {
        match item_in_slot(&inventory, *kind) {
            Some(item) => fill_slot(
                &mut commands,
                entity,
                item,
                parts,
                db,
                &asset_server,
                &mut images,
                &mut visibilities,
                &mut texts,
            ),
            None => clear_slot(&mut commands, entity, parts, &mut visibilities),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_slot(
    commands: &mut Commands,
    entity: Entity,
    item: &Item,
    parts: &CharEquipSlotParts,
    item_db: Option<&ItemDb>,
    asset_server: &AssetServer,
    images: &mut Query<&mut ImageNode>,
    visibilities: &mut Query<&mut Visibility>,
    texts: &mut Query<(&mut Text, &mut TextColor)>,
) {
    commands
        .entity(entity)
        .insert(CharEquippedIndex(item.index));

    match item_db.and_then(|db| db.icon_resource(item.item_id, item.identified)) {
        Some(resource) => {
            if let Ok(mut node) = images.get_mut(parts.icon) {
                node.image = asset_server.load(item_icon_path(resource));
            }
            set_visibility(visibilities, parts.icon, Visibility::Inherited);
            set_visibility(visibilities, parts.glyph, Visibility::Hidden);
        }
        None => {
            set_visibility(visibilities, parts.icon, Visibility::Hidden);
            set_visibility(visibilities, parts.glyph, Visibility::Inherited);
        }
    }

    if item.refine > 0 {
        if let Ok((mut text, mut color)) = texts.get_mut(parts.refine) {
            *text = Text::new(format!("+{}", item.refine));
            color.0 = theme::GOLD;
        }
        set_visibility(visibilities, parts.refine, Visibility::Inherited);
    } else {
        set_visibility(visibilities, parts.refine, Visibility::Hidden);
    }
}

fn clear_slot(
    commands: &mut Commands,
    entity: Entity,
    parts: &CharEquipSlotParts,
    visibilities: &mut Query<&mut Visibility>,
) {
    commands.entity(entity).remove::<CharEquippedIndex>();
    set_visibility(visibilities, parts.icon, Visibility::Hidden);
    set_visibility(visibilities, parts.glyph, Visibility::Inherited);
    set_visibility(visibilities, parts.refine, Visibility::Hidden);
}

/// Marker on the transient hover tooltip so [`on_slot_hover_out`] can despawn it.
#[derive(Component)]
pub struct CharEquipSlotTooltip;

/// Double-clicking a filled slot unequips it; the resulting `Inventory` change re-runs
/// [`sync_console_equipment_slots`], which empties the slot. Empty slots are inert.
fn on_slot_click(
    click: On<Pointer<Click>>,
    slots: Query<&CharEquippedIndex>,
    time: Res<Time>,
    mut last: ResMut<CharLastSlotClick>,
    mut unequip: MessageWriter<UnequipItemRequested>,
) {
    let Ok(equipped) = slots.get(click.entity) else {
        return;
    };
    let now = time.elapsed();
    if is_slot_double_click(&last, equipped.0, now) {
        unequip.write(UnequipItemRequested { index: equipped.0 });
    }
    *last = CharLastSlotClick {
        index: equipped.0,
        at: now,
    };
}

/// Hovering a filled slot spawns a `name · slot type · refine` tooltip.
fn on_slot_hover_over(
    over: On<Pointer<Over>>,
    slots: Query<(&CharEquipSlotKind, &CharEquippedIndex)>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok((kind, equipped)) = slots.get(over.entity) else {
        return;
    };
    let Some(item) = inventory.get(equipped.0) else {
        return;
    };
    let name = item_name(item_db.as_deref(), item);
    let text = tooltip_text(&name, kind.slot_label(), item.refine);
    let font = asset_server.load(theme::FONT_BODY);
    commands.spawn((
        CharEquipSlotTooltip,
        theme::label(text, font, 11.0, theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(48.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(theme::GLASS_2),
        BorderColor::all(theme::GOLD_FAINT),
        ChildOf(over.entity),
    ));
}

fn on_slot_hover_out(
    _: On<Pointer<Out>>,
    tooltips: Query<Entity, With<CharEquipSlotTooltip>>,
    mut commands: Commands,
) {
    for tooltip in &tooltips {
        commands.entity(tooltip).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equipped(index: u16, wear_state: u32) -> Item {
        Item {
            index,
            wear_state,
            amount: 1,
            ..Default::default()
        }
    }

    #[test]
    fn wear_bit_matches_location_constant() {
        let pairs = [
            (CharEquipSlotKind::HeadUpper, EQP_HEAD_TOP),
            (CharEquipSlotKind::HeadMid, EQP_HEAD_MID),
            (CharEquipSlotKind::HeadLower, EQP_HEAD_LOW),
            (CharEquipSlotKind::Body, EQP_ARMOR),
            (CharEquipSlotKind::Garment, EQP_GARMENT),
            (CharEquipSlotKind::RightHand, EQP_RIGHT_HAND),
            (CharEquipSlotKind::LeftHand, EQP_LEFT_HAND),
            (CharEquipSlotKind::AccessoryRight, EQP_RIGHT_ACCESSORY),
            (CharEquipSlotKind::AccessoryLeft, EQP_LEFT_ACCESSORY),
            (CharEquipSlotKind::Footgear, EQP_SHOES),
        ];
        for (kind, bit) in pairs {
            assert_eq!(kind.wear_bit(), bit, "{kind:?}");
        }
    }

    #[test]
    fn item_in_slot_returns_item_for_filled_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        let item = item_in_slot(&inv, CharEquipSlotKind::Body).expect("armor in body slot");

        assert_eq!(item.index, 2);
    }

    #[test]
    fn item_in_slot_returns_none_for_empty_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        assert!(item_in_slot(&inv, CharEquipSlotKind::Footgear).is_none());
    }

    #[test]
    fn two_handed_weapon_resolves_into_both_hand_slots() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(3, EQP_RIGHT_HAND | EQP_LEFT_HAND));

        let right = item_in_slot(&inv, CharEquipSlotKind::RightHand).expect("right hand");
        let left = item_in_slot(&inv, CharEquipSlotKind::LeftHand).expect("left hand");

        assert_eq!(right.index, 3);
        assert_eq!(left.index, 3);
    }

    #[test]
    fn tooltip_text_includes_refine_only_when_present() {
        assert_eq!(
            tooltip_text("Cap", "Upper Headgear", 0),
            "Cap \u{00b7} Upper Headgear"
        );
        assert_eq!(
            tooltip_text("Cap", "Upper Headgear", 5),
            "Cap \u{00b7} Upper Headgear \u{00b7} +5"
        );
    }

    #[test]
    fn every_slot_kind_has_a_label() {
        for spec in LEFT_SLOTS.iter().chain(RIGHT_SLOTS.iter()) {
            assert!(!spec.kind.slot_label().is_empty(), "{:?}", spec.kind);
        }
    }

    #[test]
    fn slot_table_covers_every_equip_slot_kind() {
        let kinds: Vec<CharEquipSlotKind> = LEFT_SLOTS
            .iter()
            .chain(RIGHT_SLOTS.iter())
            .map(|spec| spec.kind)
            .collect();
        assert_eq!(kinds.len(), 10);
        let expected = [
            CharEquipSlotKind::HeadUpper,
            CharEquipSlotKind::HeadMid,
            CharEquipSlotKind::HeadLower,
            CharEquipSlotKind::Body,
            CharEquipSlotKind::Garment,
            CharEquipSlotKind::RightHand,
            CharEquipSlotKind::LeftHand,
            CharEquipSlotKind::AccessoryRight,
            CharEquipSlotKind::AccessoryLeft,
            CharEquipSlotKind::Footgear,
        ];
        for kind in expected {
            assert!(kinds.contains(&kind), "missing {kind:?}");
        }
    }

    #[test]
    fn slot_double_click_within_window_is_true() {
        let last = CharLastSlotClick {
            index: 3,
            at: Duration::from_millis(100),
        };
        assert!(is_slot_double_click(&last, 3, Duration::from_millis(350)));
        assert!(!is_slot_double_click(&last, 4, Duration::from_millis(350)));
        assert!(!is_slot_double_click(&last, 3, Duration::from_millis(500)));
    }

    fn armor_db() -> ItemDb {
        use lifthrasir_data::{ItemData, ItemInfo};
        let mut data = ItemData::default();
        data.items.insert(
            2301,
            ItemInfo {
                identified_name: "Cotton Shirt".to_string(),
                identified_resource: "COTTON_SHIRT".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    #[test]
    fn sync_fills_then_empties_a_slot() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Font>();
        app.insert_resource(armor_db());

        let mut inv = Inventory::default();
        inv.upsert(Item {
            index: 4,
            item_id: 2301,
            wear_state: EQP_ARMOR,
            refine: 3,
            identified: true,
            amount: 1,
            ..Default::default()
        });
        app.insert_resource(inv);

        let glyph = app.world_mut().spawn(Visibility::Inherited).id();
        let icon = app
            .world_mut()
            .spawn((ImageNode::new(Handle::default()), Visibility::Hidden))
            .id();
        let refine = app
            .world_mut()
            .spawn((Text::new(""), TextColor(theme::GOLD), Visibility::Hidden))
            .id();
        let slot = app
            .world_mut()
            .spawn((
                CharEquipSlotKind::Body,
                CharEquipSlotParts {
                    glyph,
                    icon,
                    refine,
                },
            ))
            .id();

        app.add_systems(Update, sync_console_equipment_slots);
        app.update();

        assert_eq!(
            app.world().get::<CharEquippedIndex>(slot).map(|e| e.0),
            Some(4)
        );
        assert_eq!(app.world().get::<Text>(refine).unwrap().0, "+3");
        assert_eq!(
            app.world().get::<Visibility>(icon),
            Some(&Visibility::Inherited)
        );

        app.world_mut()
            .resource_mut::<Inventory>()
            .set_wear_state(4, 0);
        app.update();

        assert!(app.world().get::<CharEquippedIndex>(slot).is_none());
        assert_eq!(
            app.world().get::<Visibility>(icon),
            Some(&Visibility::Hidden)
        );
    }
}
