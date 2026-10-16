//! Opt-in content catalog resources, loaders, and lookups.

pub mod accessory;
pub mod effect;
pub mod item;
pub mod job;
mod plugin;
pub mod shader_fx;
pub mod skill;
pub mod status;
pub mod weapon;

pub use accessory::{AccessoryDataAsset, AccessoryDb};
pub use effect::{EffectCatalog, EffectDataAsset};
pub use item::{ItemDataAsset, ItemDb};
pub use job::{JobDataAsset, JobSpriteRegistry};
pub use plugin::ContentCatalogPlugin;
pub use shader_fx::ShaderFxCatalog;
pub use skill::{SkillCatalog, SkillDataAsset};
pub use status::{StatusIconCatalog, StatusIconDataAsset};
pub use weapon::{WeaponDataAsset, WeaponDb};
