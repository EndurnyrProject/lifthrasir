pub mod actions;
pub mod cursor;
pub mod events;
pub mod resources;
pub mod systems;
pub mod targeting;
pub mod terrain_raycast;
pub mod ui_focus;

pub use crate::domain::input::actions::{HOTBAR_ACTIONS, PlayerAction};
pub use crate::domain::input::cursor::{CurrentCursorType, CursorType};
pub use crate::domain::input::events::CursorChangeRequest;
pub use crate::domain::input::resources::{
    ForwardedCursorPosition, ForwardedMouseClick, LockedTarget,
};
pub use crate::domain::input::targeting::TargetingMode;
pub use crate::domain::input::terrain_raycast::TerrainRaycastCache;
pub use crate::domain::input::ui_focus::{UiFocus, ui_unfocused};
pub mod native;
pub mod plugin;
pub use crate::domain::input::native::NativeInputPlugin;
pub use crate::domain::input::plugin::InputPlugin;
