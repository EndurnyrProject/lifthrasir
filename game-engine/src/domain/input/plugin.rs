use crate::domain::input::PlayerAction;
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use leafwing_input_manager::prelude::InputManagerPlugin;

/// Input Plugin
///
/// Wires action mapping (leafwing-input-manager); keep raw key reading out of
/// game systems: they read `ActionState`. Input domain systems register
/// themselves via `auto_*` attributes in `domain/input/*`.
#[derive(AutoPlugin)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    #[auto_plugin]
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default());
    }
}
