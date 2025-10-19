//! Character Registry for Multi-Character Support
//!
//! This module will provide entity lookup for multiple characters (players and NPCs)
//! when multi-character movement is implemented.
//!
//! # Status: NOT YET IMPLEMENTED
//!
//! The current codebase assumes a single local player character. This registry
//! design is documented here as a reference for future implementation when
//! the server starts sending movement packets for other players and NPCs.
//!
//! # Architecture
//!
//! The CharacterRegistry provides bidirectional mapping between server-side
//! Account IDs (used in network packets) and client-side Entity IDs (used by Bevy ECS).
//!
//! ## When to Implement
//!
//! Implement this when:
//! - Testing with multiple players on the same map
//! - Implementing NPC spawning and movement
//! - Server sends `ZC_NOTIFY_MOVE` (0x007B) or other multi-entity packets
//!
//! ## Usage Pattern
//!
//! ```rust,ignore
//! // When spawning a character from a network packet
//! let entity = commands.spawn(/* character components */).id();
//! character_registry.register_character(account_id, entity);
//!
//! // When the local player logs in
//! character_registry.set_local_player(player_entity, player_account_id);
//!
//! // When receiving movement packets with account_id
//! if let Some(entity) = character_registry.get_entity(packet.account_id) {
//!     // Apply movement to the entity
//! }
//!
//! // When a character despawns
//! character_registry.unregister_character(account_id);
//! ```

use bevy::prelude::*;
use std::collections::HashMap;

/// Maps server Account IDs to client Entity IDs for multi-character support
///
/// # Implementation Notes
///
/// - **Thread Safety**: This will be a Bevy `Resource`, so it's automatically
///   handled by Bevy's ECS scheduling
/// - **Local Player**: Tracked separately for quick access (most queries are for local player)
/// - **Cleanup**: When entities despawn, they must be unregistered to prevent stale references
/// - **Validation**: Consider adding debug assertions to catch double-registration bugs
#[derive(Resource, Default)]
pub struct CharacterRegistry {
    /// Maps server Account IDs to client Entity IDs
    account_to_entity: HashMap<u32, Entity>,

    /// Maps client Entity IDs back to Account IDs (for cleanup)
    entity_to_account: HashMap<Entity, u32>,

    /// The local player's entity (cached for fast access)
    local_player_entity: Option<Entity>,

    /// The local player's account ID
    local_player_account_id: Option<u32>,
}

impl CharacterRegistry {
    /// Register a character entity with its server-side account ID
    ///
    /// This should be called when spawning any character from a network packet.
    pub fn register_character(&mut self, account_id: u32, entity: Entity) {
        // Debug check for double-registration
        if let Some(old_entity) = self.account_to_entity.insert(account_id, entity) {
            if old_entity != entity {
                warn!(
                    "Account ID {} was already registered to entity {:?}, replacing with {:?}",
                    account_id, old_entity, entity
                );
            }
        }

        self.entity_to_account.insert(entity, account_id);

        debug!(
            "Registered character: account_id={}, entity={:?}",
            account_id, entity
        );
    }

    /// Unregister a character (called when entity despawns)
    pub fn unregister_character(&mut self, account_id: u32) {
        if let Some(entity) = self.account_to_entity.remove(&account_id) {
            self.entity_to_account.remove(&entity);

            // Clear local player cache if it was the local player
            if self.local_player_account_id == Some(account_id) {
                self.local_player_entity = None;
                self.local_player_account_id = None;
            }

            debug!(
                "Unregistered character: account_id={}, entity={:?}",
                account_id, entity
            );
        }
    }

    /// Unregister a character by entity (alternative for entity-based cleanup)
    pub fn unregister_entity(&mut self, entity: Entity) {
        if let Some(account_id) = self.entity_to_account.remove(&entity) {
            self.account_to_entity.remove(&account_id);

            // Clear local player cache if it was the local player
            if self.local_player_entity == Some(entity) {
                self.local_player_entity = None;
                self.local_player_account_id = None;
            }

            debug!(
                "Unregistered character: entity={:?}, account_id={}",
                entity, account_id
            );
        }
    }

    /// Look up the entity for a given account ID
    ///
    /// Returns `None` if the account is not registered (character not spawned yet).
    pub fn get_entity(&self, account_id: u32) -> Option<Entity> {
        self.account_to_entity.get(&account_id).copied()
    }

    /// Look up the account ID for a given entity
    ///
    /// Useful for debugging or when sending packets that need the account ID.
    pub fn get_account_id(&self, entity: Entity) -> Option<u32> {
        self.entity_to_account.get(&entity).copied()
    }

    /// Mark an entity as the local player character
    ///
    /// This should be called once during character selection/spawn.
    /// The local player is cached separately for fast access since most
    /// queries will be for the local player.
    pub fn set_local_player(&mut self, entity: Entity, account_id: u32) {
        self.register_character(account_id, entity);
        self.local_player_entity = Some(entity);
        self.local_player_account_id = Some(account_id);

        info!(
            "Set local player: entity={:?}, account_id={}",
            entity, account_id
        );
    }

    /// Get the local player's entity
    ///
    /// Fast path for the common case of querying the local player.
    pub fn local_player_entity(&self) -> Option<Entity> {
        self.local_player_entity
    }

    /// Get the local player's account ID
    pub fn local_player_account_id(&self) -> Option<u32> {
        self.local_player_account_id
    }

    /// Check if an entity is the local player
    pub fn is_local_player(&self, entity: Entity) -> bool {
        self.local_player_entity == Some(entity)
    }

    /// Get the total number of registered characters
    pub fn character_count(&self) -> usize {
        self.account_to_entity.len()
    }

    /// Clear all registrations (useful for map changes or disconnection)
    pub fn clear(&mut self) {
        self.account_to_entity.clear();
        self.entity_to_account.clear();
        self.local_player_entity = None;
        self.local_player_account_id = None;

        info!("Cleared all character registrations");
    }
}

// TODO: Implement cleanup system for despawned entities
//
// Add a system that runs when CharacterData entities despawn:
//
// ```rust,ignore
// fn cleanup_despawned_characters(
//     mut registry: ResMut<CharacterRegistry>,
//     mut removed: RemovedComponents<CharacterData>,
// ) {
//     for entity in removed.read() {
//         registry.unregister_entity(entity);
//     }
// }
// ```

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_registration() {
        let mut registry = CharacterRegistry::default();
        let entity = Entity::from_raw(42);
        let account_id = 12345;

        registry.register_character(account_id, entity);

        assert_eq!(registry.get_entity(account_id), Some(entity));
        assert_eq!(registry.get_account_id(entity), Some(account_id));
        assert_eq!(registry.character_count(), 1);
    }

    #[test]
    fn test_local_player() {
        let mut registry = CharacterRegistry::default();
        let player_entity = Entity::from_raw(1);
        let player_account_id = 99999;

        registry.set_local_player(player_entity, player_account_id);

        assert_eq!(registry.local_player_entity(), Some(player_entity));
        assert_eq!(registry.local_player_account_id(), Some(player_account_id));
        assert!(registry.is_local_player(player_entity));
    }

    #[test]
    fn test_unregister_character() {
        let mut registry = CharacterRegistry::default();
        let entity = Entity::from_raw(42);
        let account_id = 12345;

        registry.register_character(account_id, entity);
        registry.unregister_character(account_id);

        assert_eq!(registry.get_entity(account_id), None);
        assert_eq!(registry.get_account_id(entity), None);
        assert_eq!(registry.character_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut registry = CharacterRegistry::default();
        let entity1 = Entity::from_raw(1);
        let entity2 = Entity::from_raw(2);

        registry.register_character(100, entity1);
        registry.register_character(200, entity2);
        registry.set_local_player(entity1, 100);

        assert_eq!(registry.character_count(), 2);

        registry.clear();

        assert_eq!(registry.character_count(), 0);
        assert_eq!(registry.local_player_entity(), None);
    }
}
