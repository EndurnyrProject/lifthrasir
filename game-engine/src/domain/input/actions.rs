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
}

impl PlayerAction {
    /// Default keybinds. Attached to the local player entity on spawn.
    ///
    /// `Insert` is the classic RO binding; `Help` occupies Insert's physical
    /// slot on full-size Apple keyboards (MacBooks lack an Insert key entirely).
    /// `Status` is the classic RO Alt+A chord. `Inventory` is the classic RO Alt+E chord.
    pub fn default_input_map() -> InputMap<Self> {
        InputMap::new([(Self::Sit, KeyCode::Insert), (Self::Sit, KeyCode::Help)])
            .with(
                Self::Status,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyA),
            )
            .with(
                Self::Inventory,
                ButtonlikeChord::modified(ModifierKey::Alt, KeyCode::KeyE),
            )
    }
}
