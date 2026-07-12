use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

/// In-world player actions, mapped from raw input by leafwing-input-manager.
///
/// Add variants here as keybinds grow; the `InputMap` (see `default_input_map`)
/// is where the concrete bindings live, so remapping and chords are a matter of
/// editing the map at runtime, not this enum.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum PlayerAction {
    /// Toggle sit/stand.
    Sit,
    /// Toggle the status window.
    Status,
    /// Toggle the inventory window.
    Inventory,
    /// Toggle the skills window.
    Skills,
    /// Toggle the equipment window.
    Equipment,
    /// Toggle the pushcart window.
    Cart,
    /// Toggle the party roster window.
    Party,
    /// Toggle the emote picker window.
    Emote,
    /// Activate hotbar slot 1 (default F1).
    Slot1,
    /// Activate hotbar slot 2 (default F2).
    Slot2,
    /// Activate hotbar slot 3 (default F3).
    Slot3,
    /// Activate hotbar slot 4 (default F4).
    Slot4,
    /// Activate hotbar slot 5 (default F5).
    Slot5,
    /// Activate hotbar slot 6 (default F6).
    Slot6,
    /// Activate hotbar slot 7 (default F7).
    Slot7,
    /// Activate hotbar slot 8 (default F8).
    Slot8,
    /// Activate hotbar slot 9 (default F9).
    Slot9,
    /// Activate hotbar slot 10 (default F10).
    Slot10,
    /// Activate hotbar slot 11 (default F11).
    Slot11,
    /// Activate hotbar slot 12 (default F12).
    Slot12,
}

/// The twelve hotbar actions in slot order; index `i` is slot `i + 1`.
/// The activation dispatch and the Settings rebind tab both index this.
pub const HOTBAR_ACTIONS: [PlayerAction; 12] = [
    PlayerAction::Slot1,
    PlayerAction::Slot2,
    PlayerAction::Slot3,
    PlayerAction::Slot4,
    PlayerAction::Slot5,
    PlayerAction::Slot6,
    PlayerAction::Slot7,
    PlayerAction::Slot8,
    PlayerAction::Slot9,
    PlayerAction::Slot10,
    PlayerAction::Slot11,
    PlayerAction::Slot12,
];

/// Default keys for the hotbar slots, aligned with `HOTBAR_ACTIONS` (F1..F12).
const HOTBAR_KEYS: [KeyCode; 12] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];

impl PlayerAction {
    /// Default keybinds. Attached to the local player entity on spawn.
    ///
    /// `Insert` is the classic RO binding; `Help` occupies Insert's physical
    /// slot on full-size Apple keyboards (MacBooks lack an Insert key entirely).
    /// `Status` is the classic RO Alt+A chord. `Inventory` is the classic RO Alt+E chord.
    /// `Skills` is the classic RO Alt+S chord. `Equipment` is the classic RO Alt+Q chord.
    /// `Cart` uses Alt+W. `Party` uses the unmodified P key. `Emote` uses Alt+M.
    pub fn default_input_map() -> InputMap<Self> {
        let mut map = InputMap::new([(Self::Sit, KeyCode::Insert), (Self::Sit, KeyCode::Help)])
            .with(
                Self::Status,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyA),
            )
            .with(
                Self::Inventory,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyE),
            )
            .with(
                Self::Skills,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyS),
            )
            .with(
                Self::Equipment,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyQ),
            )
            .with(
                Self::Cart,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyW),
            )
            .with(Self::Party, KeyCode::KeyP)
            .with(
                Self::Emote,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyM),
            );
        for (action, key) in HOTBAR_ACTIONS.into_iter().zip(HOTBAR_KEYS) {
            map.insert(action, key);
        }
        map
    }

    /// Slot index (0-based) of a hotbar action, or `None` for non-hotbar actions.
    pub fn hotbar_index(self) -> Option<usize> {
        HOTBAR_ACTIONS.iter().position(|&action| action == self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotbar_actions_map_to_sequential_slots() {
        use PlayerAction::*;
        let expected = [
            Slot1, Slot2, Slot3, Slot4, Slot5, Slot6, Slot7, Slot8, Slot9, Slot10, Slot11, Slot12,
        ];
        assert_eq!(HOTBAR_ACTIONS, expected);
        for (i, action) in HOTBAR_ACTIONS.into_iter().enumerate() {
            assert_eq!(action.hotbar_index(), Some(i));
        }
        assert_eq!(PlayerAction::Sit.hotbar_index(), None);
    }

    #[test]
    fn default_input_map_binds_equipment_to_alt_q() {
        let map = PlayerAction::default_input_map();
        let mut expected = InputMap::default();
        expected.insert(
            PlayerAction::Equipment,
            ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyQ),
        );
        assert_eq!(
            map.get(&PlayerAction::Equipment),
            expected.get(&PlayerAction::Equipment)
        );
    }

    #[test]
    fn default_input_map_binds_cart_to_alt_w() {
        let map = PlayerAction::default_input_map();
        let mut expected = InputMap::default();
        expected.insert(
            PlayerAction::Cart,
            ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyW),
        );
        assert_eq!(
            map.get(&PlayerAction::Cart),
            expected.get(&PlayerAction::Cart)
        );
    }

    #[test]
    fn default_input_map_binds_emote_to_alt_m() {
        let map = PlayerAction::default_input_map();
        let mut expected = InputMap::default();
        expected.insert(
            PlayerAction::Emote,
            ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyM),
        );
        assert_eq!(
            map.get(&PlayerAction::Emote),
            expected.get(&PlayerAction::Emote)
        );
    }

    #[test]
    fn default_input_map_binds_each_slot_to_its_f_key() {
        let map = PlayerAction::default_input_map();
        for (action, key) in HOTBAR_ACTIONS.into_iter().zip(HOTBAR_KEYS) {
            let mut expected = InputMap::default();
            expected.insert(action, key);
            assert_eq!(map.get(&action), expected.get(&action));
        }
    }
}
