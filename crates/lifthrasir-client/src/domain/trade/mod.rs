pub mod plugin;
pub mod resource;
pub mod systems;

pub use crate::domain::trade::plugin::TradePlugin;
pub use crate::domain::trade::resource::{MAX_OFFER_SLOTS, OpenTrade, TradeSession};
