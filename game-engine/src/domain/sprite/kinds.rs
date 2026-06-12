use bevy::prelude::*;
use moonshine_kind::{CastInto, Kind};

use crate::domain::entities::character::components::core::CharacterAppearance;
use crate::domain::entities::markers::{
    Elemental, Homunculus, LocalPlayer, Mercenary, Mob, Npc, RemotePlayer,
};
use crate::infrastructure::assets::ro_animation_asset::RoSprite;

/// Kind for any entity with RoSprite (animated entity)
/// Note: Will return empty queries until Phase 3 adds RoSprite to entities
pub struct Animated;

impl Kind for Animated {
    type Filter = With<RoSprite>;
}

/// Kind for the local player's character
pub struct LocalPlayerKind;

impl Kind for LocalPlayerKind {
    type Filter = (With<LocalPlayer>, With<CharacterAppearance>);
}

/// Kind for remote player characters
pub struct PlayerKind;

impl Kind for PlayerKind {
    type Filter = (With<RemotePlayer>, With<CharacterAppearance>);
}

/// Kind for monster/mob entities
pub struct MonsterKind;

impl Kind for MonsterKind {
    type Filter = With<Mob>;
}

/// Kind for NPC entities
pub struct NpcKind;

impl Kind for NpcKind {
    type Filter = With<Npc>;
}

/// Kind for homunculus entities (player's summoned creature)
pub struct HomunculusKind;

impl Kind for HomunculusKind {
    type Filter = With<Homunculus>;
}

/// Kind for mercenary entities (hired fighter)
pub struct MercenaryKind;

impl Kind for MercenaryKind {
    type Filter = With<Mercenary>;
}

/// Kind for elemental entities (summoned element)
pub struct ElementalKind;

impl Kind for ElementalKind {
    type Filter = With<Elemental>;
}

// All entity kinds can be cast to Animated (once they have RoSprite)
impl CastInto<Animated> for LocalPlayerKind {}
impl CastInto<Animated> for PlayerKind {}
impl CastInto<Animated> for MonsterKind {}
impl CastInto<Animated> for NpcKind {}
impl CastInto<Animated> for HomunculusKind {}
impl CastInto<Animated> for MercenaryKind {}
impl CastInto<Animated> for ElementalKind {}
