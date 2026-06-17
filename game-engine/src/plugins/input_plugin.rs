use crate::app::InputPlugin as InputDomainPlugin;
use crate::domain::input::PlayerAction;
use bevy::prelude::*;
use leafwing_input_manager::prelude::InputManagerPlugin;

/// Wires action mapping (leafwing-input-manager) together with the input domain
/// systems. Keep raw key reading out of game systems: they read `ActionState`.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default())
            .add_plugins(InputDomainPlugin);
    }
}
