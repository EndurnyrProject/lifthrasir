use bevy::prelude::*;

use super::types::ObjectType;

// =============================================================================
// PHASE 0.2: RoAnimationController REMOVED
// =============================================================================
// RoAnimationController has been replaced by:
// - PlayerSprite/MobSprite (sprite_rendering/components/ro_sprite.rs) - lightweight animation state
// - RoAnimationAsset - pre-computed textures and frame data
//
// The new system uses O(1) frame lookup instead of runtime texture creation.
// =============================================================================

/// Network entity identifier component
#[derive(Component, Debug, Clone, Copy)]
pub struct NetworkEntity {
    /// Account id. Informational only; aesir no longer keys in-game packets on it.
    pub aid: u32,
    /// char_id, the unit id aesir uses in every in-game packet (combat, move, name,
    /// despawn). This is the key the `EntityRegistry` and combat lookups resolve on.
    pub gid: u32,
    pub object_type: ObjectType,
}

impl NetworkEntity {
    pub fn new(aid: u32, gid: u32, object_type: ObjectType) -> Self {
        Self {
            aid,
            gid,
            object_type,
        }
    }
}

/// Entity name component
#[derive(Component, Debug, Clone)]
pub struct EntityName {
    pub name: String,
    pub party_name: Option<String>,
    pub guild_name: Option<String>,
    pub position_name: Option<String>,
}

impl EntityName {
    pub fn new(name: String) -> Self {
        Self {
            name,
            party_name: None,
            guild_name: None,
            position_name: None,
        }
    }

    pub fn with_full_details(
        name: String,
        party_name: String,
        guild_name: String,
        position_name: String,
    ) -> Self {
        Self {
            name,
            party_name: Some(party_name).filter(|s| !s.is_empty()),
            guild_name: Some(guild_name).filter(|s| !s.is_empty()),
            position_name: Some(position_name).filter(|s| !s.is_empty()),
        }
    }
}

/// Guild identity supplied with a remote player's visibility spawn.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct GuildIdentity {
    pub guild_id: u32,
    pub guild_name: String,
    pub emblem_id: u32,
}

/// Marks a remote PC whose current guild membership is authoritative from its spawn.
#[derive(Component, Debug, Clone, Copy)]
pub struct SpawnGuildIdentityKnown;

/// Pending despawn component for deferred entity removal
#[derive(Component, Debug, Clone, Copy)]
pub struct PendingDespawn {
    pub vanish_type: u8,
    pub marked_at: std::time::Instant,
}

impl PendingDespawn {
    pub fn new(vanish_type: u8) -> Self {
        Self {
            vanish_type,
            marked_at: std::time::Instant::now(),
        }
    }

    pub fn has_timed_out(&self) -> bool {
        self.marked_at.elapsed().as_secs() >= 5
    }
}
