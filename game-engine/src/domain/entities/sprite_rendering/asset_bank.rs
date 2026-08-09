//! Shared [`SystemParam`] bundling the SPR/ACT asset stores, the animation and
//! image outputs, and the graphics settings that every sprite-finalizing system
//! needs to turn a raw SPR/ACT pair into a renderable [`RoAnimationAsset`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::domain::settings::GraphicsSettings;
use crate::infrastructure::assets::loaders::{RoActAsset, RoSpriteAsset};
use crate::infrastructure::assets::ro_animation_asset::RoAnimationAsset;

/// The five parameters shared by the sprite-finalizing systems
/// (`spawn_effect_sprites`, `finalize_falcon_layer`, `finalize_frozen_ice_assets`).
/// Grouped so each of those systems takes one parameter instead of five.
#[derive(SystemParam)]
pub struct SpriteAssetBank<'w> {
    pub sprites: Res<'w, Assets<RoSpriteAsset>>,
    pub actions: Res<'w, Assets<RoActAsset>>,
    pub animations: ResMut<'w, Assets<RoAnimationAsset>>,
    pub images: ResMut<'w, Assets<Image>>,
    pub settings: Res<'w, GraphicsSettings>,
}
