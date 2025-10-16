pub mod components;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod systems;

pub use events::{MuteBgmEvent, PlayBgmEvent, SetBgmVolumeEvent, StopBgmEvent};
pub use plugin::AudioDomainPlugin;
pub use resources::{AudioSettings, BgmManager};
