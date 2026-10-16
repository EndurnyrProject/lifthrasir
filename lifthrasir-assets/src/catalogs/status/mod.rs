pub mod asset;
pub mod catalog;
pub mod plugin;

pub use asset::StatusIconDataAsset;
pub use catalog::{StatusIconCatalog, process_loaded_status_icons, start_loading_status_icons};
pub use plugin::StatusIconPlugin;
