use bevy::prelude::*;

#[derive(Component)]
#[require(Transform, Visibility, crate::actors::ActorAnimationInput)]
pub struct SpriteRoot;

#[derive(Component)]
pub struct SpriteLayer;

#[derive(Component, Debug)]
pub struct EffectLayer;
