//! Equipment paperdoll window: the reference pilot for Bevy 0.19 BSN + Feathers.
//!
//! Task 4 builds the chrome only — a draggable, Alt+Q-toggled glass window with a
//! titlebar, two columns of empty `slot_well`s (one per [`EquipSlotKind`]), a center
//! preview-frame placeholder + Rotate row, and a footer note. The node trees are
//! authored with `bsn!` in [`scene`]; markers, picking, observers and `ChildOf`
//! wiring are inserted imperatively (the hybrid the design calls for). Live slot
//! data is Task 5; the real preview is Task 7.

use bevy::prelude::*;
use bevy_feathers::FeathersCorePlugin;
use bevy_feathers::FeathersPlugins;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::events::forward_character_sprite_events;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::input::{ui_unfocused, PlayerAction};
use leafwing_input_manager::prelude::ActionState;

use crate::theme::feathers_theme::install_norse_theme;

pub mod preview;
pub mod scene;
pub mod slots;

pub use slots::EquipSlotKind;

/// Window-root marker so the toggle/close systems can flip its visibility and the
/// drag helper can move it.
#[derive(Component, Default, Clone)]
pub struct EquipmentWindowRoot;

/// The drag-handle titlebar; the drag observer only moves the window when the drag's
/// target is the titlebar itself, so dragging from the close/minimize buttons is inert.
#[derive(Component, Default, Clone)]
pub struct EquipmentTitlebar;

/// Center preview-frame placeholder; Task 7 renders the live character into it.
#[derive(Component, Default, Clone)]
pub struct EquipmentPreviewFrame;

/// Rotate-left button; Task 7 steps the preview character's facing.
#[derive(Component, Default, Clone)]
pub struct RotateLeftButton;

/// Rotate-right button; Task 7 steps the preview character's facing.
#[derive(Component, Default, Clone)]
pub struct RotateRightButton;

/// The bordered well inside a slot; Task 5 places the item icon here and tints it.
#[derive(Component, Default, Clone)]
pub struct EquipSlotWell;

/// Hidden refine-badge text inside a slot; Task 5 fills it with `+N`.
#[derive(Component, Default, Clone)]
pub struct EquipSlotRefine;

/// Child entities of a slot's well that [`slots::sync_equipment_slots`] patches each
/// frame the inventory changes: the empty-state glyph, the item-icon `ImageNode`, and
/// the refine badge text. Derives `FromTemplate` so the fields can be populated from
/// `#Name` references inside the slot's `bsn!` scene. Item names are shown on hover
/// (see [`slots::on_slot_hover_over`]), not as an always-on caption.
#[derive(Component, FromTemplate, Clone)]
pub struct EquipSlotParts {
    pub glyph: Entity,
    pub icon: Entity,
    pub refine: Entity,
}

/// The inventory index of the item currently shown in a slot. Inserted when the slot
/// fills, removed when it empties; the double-click handler reads it to unequip.
#[derive(Component, Clone, Copy)]
pub struct EquippedIndex(pub u16);

/// A slot's place in a column: which kind it is, its caption label, and the
/// empty-state glyph icon name (under `assets/ui/icons/`).
#[derive(Clone, Copy)]
pub struct SlotSpec {
    pub kind: EquipSlotKind,
    pub label: &'static str,
    pub glyph: &'static str,
}

const fn slot(kind: EquipSlotKind, label: &'static str, glyph: &'static str) -> SlotSpec {
    SlotSpec { kind, label, glyph }
}

/// Left column, top-to-bottom: the five armor slots.
pub const LEFT_SLOTS: [SlotSpec; 5] = [
    slot(EquipSlotKind::HeadUpper, "Upper", "head"),
    slot(EquipSlotKind::HeadMid, "Mid", "headm"),
    slot(EquipSlotKind::HeadLower, "Lower", "headl"),
    slot(EquipSlotKind::Body, "Body", "armor"),
    slot(EquipSlotKind::Garment, "Garment", "garment"),
];

/// Right column, top-to-bottom: hands, accessories, footgear.
pub const RIGHT_SLOTS: [SlotSpec; 5] = [
    slot(EquipSlotKind::RightHand, "Right Hand", "sword"),
    slot(EquipSlotKind::LeftHand, "Left Hand", "shield"),
    slot(EquipSlotKind::AccessoryRight, "Accessory", "ring"),
    slot(EquipSlotKind::AccessoryLeft, "Accessory", "ring"),
    slot(EquipSlotKind::Footgear, "Footgear", "boot"),
];

pub struct EquipmentWindowPlugin;

impl Plugin for EquipmentWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<slots::LastSlotClick>();
        app.init_resource::<preview::PreviewState>();
        app.init_resource::<preview::LocalHeadgear>();
        app.add_systems(
            Update,
            toggle_equipment_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                slots::sync_equipment_slots,
                preview::cache_local_headgear,
                preview::spawn_preview.after(forward_character_sprite_events),
                preview::forward_preview_headgear,
                preview::tag_preview_billboards,
            )
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), preview::cleanup_preview);
    }
}

/// Spawn the hidden equipment window under `parent`. Delegates the BSN chrome to
/// [`scene::build`]; asset paths resolve inside the scene, so no `AssetServer` is needed.
pub fn spawn_equipment_window(commands: &mut Commands, parent: Entity) {
    scene::build(commands, parent);
}

/// Alt+Q toggles the equipment window between hidden and visible.
fn toggle_equipment_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<EquipmentWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Equipment) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_table_covers_every_equip_slot_kind() {
        let kinds: Vec<EquipSlotKind> = LEFT_SLOTS
            .iter()
            .chain(RIGHT_SLOTS.iter())
            .map(|spec| spec.kind)
            .collect();

        assert_eq!(kinds.len(), 10);

        let expected = [
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
        for kind in expected {
            assert!(kinds.contains(&kind), "missing {kind:?}");
        }
    }
}
