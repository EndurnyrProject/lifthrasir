use crate::domain::entities::types::ObjectType;
use bevy::prelude::*;

/// Buffer for spawn events that arrive before GameState::InGame
///
/// Events are captured during Connecting/Loading states and replayed
/// when the game enters InGame state.
#[derive(Resource, Default)]
pub struct PendingSpawnBuffer {
    pub events: Vec<SpawnEntity>,
}

/// Event to spawn a network entity
#[derive(Message, Debug, Clone)]
pub struct SpawnEntity {
    // Identity
    pub aid: u32,
    pub gid: u32,
    pub object_type: ObjectType,
    pub name: String,

    // Position & Movement
    pub position: (u16, u16),
    pub direction: u8,
    pub destination: Option<(u16, u16)>,
    pub move_start_time: Option<u32>,
    pub current_server_tick: u32,

    // Appearance
    pub job: u16,
    pub head: u16,
    pub body: u16,
    pub gender: u8,
    pub head_palette: u16,
    pub body_palette: u16,

    // Equipment
    pub weapon: u32,
    pub shield: u32,
    pub head_bottom: u16,
    pub head_mid: u16,
    pub head_top: u16,
    pub robe: u16,

    // Stats
    pub hp: u32,
    pub max_hp: u32,
    pub speed: u16,
    pub level: u16,
}

/// Event to despawn a network entity
///
/// This event uses the observer pattern and is targeted at a specific entity.
/// When triggered, it will remove the entity and all its sprite hierarchy.
#[derive(EntityEvent, Debug, Clone)]
pub struct DespawnEntity {
    #[event_target]
    pub entity: Entity,
    pub aid: u32,
}

/// Network protocol event to request entity vanish (from server)
///
/// This event is emitted by VanishHandler when the server sends VANISH packet.
/// A bridge system converts this to EntityVanishRequested observer event.
#[derive(Message, Debug, Clone)]
pub struct RequestEntityVanish {
    pub aid: u32,
    pub vanish_type: u8,
}

/// Entity-targeted event for vanish requests
///
/// This observer event is triggered when an entity needs to vanish.
/// An observer will check if the entity is moving and either:
/// - Mark it with PendingDespawn if moving (defer despawn until movement completes)
/// - Trigger DespawnEntity immediately if idle
#[derive(EntityEvent, Debug, Clone)]
pub struct EntityVanishRequested {
    #[event_target]
    pub entity: Entity,
    pub aid: u32,
    pub vanish_type: u8,
}
