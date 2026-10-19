pub mod assets;
pub mod plugin;
pub mod render;
pub mod send;
pub mod table;

pub use crate::domain::emote::plugin::EmotePlugin;
pub use crate::domain::emote::send::{EmoteCooldown, EmoteRequested};
