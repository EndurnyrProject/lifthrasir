pub mod dispatch;
pub mod model;
pub mod persistence;

pub use crate::domain::hotbar::dispatch::HotbarSlotActivated;
pub use crate::domain::hotbar::model::{Hotbar, HotbarSlot};
