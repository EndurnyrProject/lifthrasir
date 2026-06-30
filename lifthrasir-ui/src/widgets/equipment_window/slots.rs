use std::time::Duration;

use bevy::prelude::*;
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::equipment::location::{
    EQP_ARMOR, EQP_GARMENT, EQP_HEAD_LOW, EQP_HEAD_MID, EQP_HEAD_TOP, EQP_LEFT_ACCESSORY,
    EQP_LEFT_HAND, EQP_RIGHT_ACCESSORY, EQP_RIGHT_HAND, EQP_SHOES,
};
use game_engine::domain::equipment::UnequipItemRequested;
use game_engine::domain::inventory::{Inventory, Item};
use game_engine::infrastructure::item::ItemDb;

use crate::theme;

use super::{EquipSlotParts, EquippedIndex};

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EquipSlotKind {
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

impl EquipSlotKind {
    pub fn wear_bit(self) -> u32 {
        match self {
            EquipSlotKind::HeadUpper => EQP_HEAD_TOP,
            EquipSlotKind::HeadMid => EQP_HEAD_MID,
            EquipSlotKind::HeadLower => EQP_HEAD_LOW,
            EquipSlotKind::Body => EQP_ARMOR,
            EquipSlotKind::Garment => EQP_GARMENT,
            EquipSlotKind::RightHand => EQP_RIGHT_HAND,
            EquipSlotKind::LeftHand => EQP_LEFT_HAND,
            EquipSlotKind::AccessoryRight => EQP_RIGHT_ACCESSORY,
            EquipSlotKind::AccessoryLeft => EQP_LEFT_ACCESSORY,
            EquipSlotKind::Footgear => EQP_SHOES,
        }
    }
}

pub fn item_in_slot(inventory: &Inventory, kind: EquipSlotKind) -> Option<&Item> {
    inventory
        .equipped()
        .find(|item| item.wear_state & kind.wear_bit() != 0)
}

impl EquipSlotKind {
    /// Human-readable slot name for tooltips.
    pub fn slot_label(self) -> &'static str {
        match self {
            EquipSlotKind::HeadUpper => "Upper Headgear",
            EquipSlotKind::HeadMid => "Mid Headgear",
            EquipSlotKind::HeadLower => "Lower Headgear",
            EquipSlotKind::Body => "Body",
            EquipSlotKind::Garment => "Garment",
            EquipSlotKind::RightHand => "Right Hand",
            EquipSlotKind::LeftHand => "Left Hand",
            EquipSlotKind::AccessoryRight => "Accessory",
            EquipSlotKind::AccessoryLeft => "Accessory",
            EquipSlotKind::Footgear => "Footgear",
        }
    }
}

fn tooltip_text(name: &str, slot: &str, refine: u8) -> String {
    if refine > 0 {
        format!("{name} \u{00b7} {slot} \u{00b7} +{refine}")
    } else {
        format!("{name} \u{00b7} {slot}")
    }
}

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Resource, Default)]
pub struct LastSlotClick {
    index: u16,
    at: Duration,
}

fn is_slot_double_click(last: &LastSlotClick, index: u16, now: Duration) -> bool {
    last.index == index && now.saturating_sub(last.at) <= DOUBLE_CLICK
}

/// Marker on the transient hover tooltip so [`on_slot_hover_out`] can despawn it.
#[derive(Component)]
pub struct EquipSlotTooltip;

fn item_name(item_db: Option<&ItemDb>, item: &Item) -> String {
    item_db
        .and_then(|db| db.name(item.item_id, item.identified))
        .map(str::to_string)
        .unwrap_or_else(|| format!("#{}", item.item_id))
}

fn set_visibility(visibilities: &mut Query<&mut Visibility>, entity: Entity, value: Visibility) {
    if let Ok(mut vis) = visibilities.get_mut(entity) {
        *vis = value;
    }
}

/// Patches every slot from live equipped state whenever the `Inventory` resource
/// changes: shows the item icon + refine badge + rarity-tinted name on filled slots,
/// restores the empty-state glyph + `Empty` caption otherwise, and tracks the worn
/// item's index so the double-click handler can unequip it.
#[allow(clippy::too_many_arguments)]
pub fn sync_equipment_slots(
    mut commands: Commands,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    asset_server: Res<AssetServer>,
    slots: Query<(Entity, &EquipSlotKind, &EquipSlotParts)>,
    mut images: Query<&mut ImageNode>,
    mut visibilities: Query<&mut Visibility>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    if !inventory.is_changed() {
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
    parts: &EquipSlotParts,
    item_db: Option<&ItemDb>,
    asset_server: &AssetServer,
    images: &mut Query<&mut ImageNode>,
    visibilities: &mut Query<&mut Visibility>,
    texts: &mut Query<(&mut Text, &mut TextColor)>,
) {
    commands.entity(entity).insert(EquippedIndex(item.index));

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
    parts: &EquipSlotParts,
    visibilities: &mut Query<&mut Visibility>,
) {
    commands.entity(entity).remove::<EquippedIndex>();
    set_visibility(visibilities, parts.icon, Visibility::Hidden);
    set_visibility(visibilities, parts.glyph, Visibility::Inherited);
    set_visibility(visibilities, parts.refine, Visibility::Hidden);
}

/// Double-clicking a filled slot unequips it; the resulting `Inventory` change
/// re-runs [`sync_equipment_slots`], which empties the slot. Empty slots are inert.
pub fn on_slot_click(
    click: On<Pointer<Click>>,
    slots: Query<&EquippedIndex>,
    time: Res<Time>,
    mut last: ResMut<LastSlotClick>,
    mut unequip: MessageWriter<UnequipItemRequested>,
) {
    let Ok(equipped) = slots.get(click.entity) else {
        return;
    };
    let now = time.elapsed();
    if is_slot_double_click(&last, equipped.0, now) {
        unequip.write(UnequipItemRequested { index: equipped.0 });
    }
    *last = LastSlotClick {
        index: equipped.0,
        at: now,
    };
}

/// Hovering a filled slot spawns a `name \u{00b7} slot type \u{00b7} refine` tooltip.
pub fn on_slot_hover_over(
    over: On<Pointer<Over>>,
    slots: Query<(&EquipSlotKind, &EquippedIndex)>,
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
        EquipSlotTooltip,
        theme::label(text, font, 11.0, theme::TEXT),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(72.0),
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

pub fn on_slot_hover_out(
    _: On<Pointer<Out>>,
    tooltips: Query<Entity, With<EquipSlotTooltip>>,
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
            (EquipSlotKind::HeadUpper, EQP_HEAD_TOP),
            (EquipSlotKind::HeadMid, EQP_HEAD_MID),
            (EquipSlotKind::HeadLower, EQP_HEAD_LOW),
            (EquipSlotKind::Body, EQP_ARMOR),
            (EquipSlotKind::Garment, EQP_GARMENT),
            (EquipSlotKind::RightHand, EQP_RIGHT_HAND),
            (EquipSlotKind::LeftHand, EQP_LEFT_HAND),
            (EquipSlotKind::AccessoryRight, EQP_RIGHT_ACCESSORY),
            (EquipSlotKind::AccessoryLeft, EQP_LEFT_ACCESSORY),
            (EquipSlotKind::Footgear, EQP_SHOES),
        ];
        for (kind, bit) in pairs {
            assert_eq!(kind.wear_bit(), bit, "{kind:?}");
        }
    }

    #[test]
    fn item_in_slot_returns_item_for_filled_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        let item = item_in_slot(&inv, EquipSlotKind::Body).expect("armor in body slot");

        assert_eq!(item.index, 2);
    }

    #[test]
    fn item_in_slot_returns_none_for_empty_slot() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(2, EQP_ARMOR));

        assert!(item_in_slot(&inv, EquipSlotKind::Footgear).is_none());
    }

    #[test]
    fn two_handed_weapon_resolves_into_both_hand_slots() {
        let mut inv = Inventory::default();
        inv.upsert(equipped(3, EQP_RIGHT_HAND | EQP_LEFT_HAND));

        let right = item_in_slot(&inv, EquipSlotKind::RightHand).expect("right hand");
        let left = item_in_slot(&inv, EquipSlotKind::LeftHand).expect("left hand");

        assert_eq!(right.index, 3);
        assert_eq!(left.index, 3);
    }

    #[test]
    fn non_v1_bit_resolves_to_no_slot() {
        const COSTUME_BIT: u32 = 0x000400;
        let mut inv = Inventory::default();
        inv.upsert(equipped(4, COSTUME_BIT));

        let kinds = [
            EquipSlotKind::HeadUpper,
            EquipSlotKind::HeadMid,
            EquipSlotKind::HeadLower,
            EquipSlotKind::Body,
            EquipSlotKind::Garment,
            EquipSlotKind::RightHand,
            EquipSlotKind::LeftHand,
            EquipSlotKind::AccessoryRight,
            EquipSlotKind::AccessoryLeft,
            EquipSlotKind::Footgear,
        ];
        for kind in kinds {
            assert!(item_in_slot(&inv, kind).is_none(), "{kind:?}");
        }
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
        let kinds = [
            EquipSlotKind::HeadUpper,
            EquipSlotKind::HeadMid,
            EquipSlotKind::HeadLower,
            EquipSlotKind::Body,
            EquipSlotKind::Garment,
            EquipSlotKind::RightHand,
            EquipSlotKind::LeftHand,
            EquipSlotKind::AccessoryRight,
            EquipSlotKind::AccessoryLeft,
            EquipSlotKind::Footgear,
        ];
        for kind in kinds {
            assert!(!kind.slot_label().is_empty(), "{kind:?}");
        }
    }

    #[test]
    fn slot_double_click_within_window_is_true() {
        let last = LastSlotClick {
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
                EquipSlotKind::Body,
                EquipSlotParts {
                    glyph,
                    icon,
                    refine,
                },
            ))
            .id();

        app.add_systems(Update, sync_equipment_slots);
        app.update();

        assert_eq!(app.world().get::<EquippedIndex>(slot).map(|e| e.0), Some(4));
        assert_eq!(app.world().get::<Text>(refine).unwrap().0, "+3");
        assert_eq!(
            app.world().get::<Visibility>(icon),
            Some(&Visibility::Inherited)
        );

        app.world_mut()
            .resource_mut::<Inventory>()
            .set_wear_state(4, 0);
        app.update();

        assert!(app.world().get::<EquippedIndex>(slot).is_none());
        assert_eq!(
            app.world().get::<Visibility>(icon),
            Some(&Visibility::Hidden)
        );
    }
}
