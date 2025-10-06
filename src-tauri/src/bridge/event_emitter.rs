use bevy::prelude::*;
use game_engine::domain::character::events::{
    ActorInitSent, MapLoadCompleted, MapLoadingFailed, MapLoadingStarted, ZoneAuthenticationFailed,
    ZoneAuthenticationSuccess, ZoneServerConnected, ZoneServerConnectionFailed,
    ZoneServerInfoReceivedEvent,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Resource that allows Bevy systems to emit events to Tauri/React
/// This is a NonSend resource because AppHandle is !Send
#[derive(Resource)]
pub struct TauriEventEmitter {
    app_handle: AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Emit zone-connecting event with map name
    pub fn emit_zone_connecting(&self, map_name: String) {
        let payload = ZoneConnectingPayload { map_name };
        if let Err(e) = self.app_handle.emit("zone-connecting", payload) {
            error!("Failed to emit zone-connecting event: {:?}", e);
        }
    }

    /// Emit zone-connected event
    pub fn emit_zone_connected(&self) {
        if let Err(e) = self.app_handle.emit("zone-connected", ()) {
            error!("Failed to emit zone-connected event: {:?}", e);
        }
    }

    /// Emit zone-authenticated event with spawn coordinates
    pub fn emit_zone_authenticated(&self, spawn_x: u16, spawn_y: u16) {
        let payload = ZoneAuthenticatedPayload { spawn_x, spawn_y };
        if let Err(e) = self.app_handle.emit("zone-authenticated", payload) {
            error!("Failed to emit zone-authenticated event: {:?}", e);
        }
    }

    /// Emit map-loading event
    pub fn emit_map_loading(&self, map_name: String) {
        let payload = MapLoadingPayload { map_name };
        if let Err(e) = self.app_handle.emit("map-loading", payload) {
            error!("Failed to emit map-loading event: {:?}", e);
        }
    }

    /// Emit map-loaded event
    pub fn emit_map_loaded(&self, map_name: String) {
        let payload = MapLoadedPayload { map_name };
        if let Err(e) = self.app_handle.emit("map-loaded", payload) {
            error!("Failed to emit map-loaded event: {:?}", e);
        }
    }

    /// Emit entering-world event (after CZ_NOTIFY_ACTORINIT sent)
    pub fn emit_entering_world(&self) {
        if let Err(e) = self.app_handle.emit("entering-world", ()) {
            error!("Failed to emit entering-world event: {:?}", e);
        }
    }

    /// Emit zone-error event with error message
    pub fn emit_zone_error(&self, error: String) {
        let payload = ZoneErrorPayload { error };
        if let Err(e) = self.app_handle.emit("zone-error", payload) {
            error!("Failed to emit zone-error event: {:?}", e);
        }
    }

    /// Emit map-loading-failed event with error message
    pub fn emit_map_loading_failed(&self, error: String) {
        let payload = MapLoadingFailedPayload { error };
        if let Err(e) = self.app_handle.emit("map-loading-failed", payload) {
            error!("Failed to emit map-loading-failed event: {:?}", e);
        }
    }
}

// Event payload types

#[derive(Debug, Clone, Serialize)]
struct ZoneConnectingPayload {
    map_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ZoneAuthenticatedPayload {
    spawn_x: u16,
    spawn_y: u16,
}

#[derive(Debug, Clone, Serialize)]
struct MapLoadingPayload {
    map_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct MapLoadedPayload {
    map_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ZoneErrorPayload {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
struct MapLoadingFailedPayload {
    error: String,
}

// ============================================================================
// System: zone_status_event_emitter
// ============================================================================

/// System that listens to zone-related events and emits them to Tauri/React
/// This is a NonSend system because it accesses TauriEventEmitter which contains AppHandle
pub fn zone_status_event_emitter(
    emitter: NonSend<TauriEventEmitter>,
    mut zone_info_events: EventReader<ZoneServerInfoReceivedEvent>,
    mut connected_events: EventReader<ZoneServerConnected>,
    mut connection_failed_events: EventReader<ZoneServerConnectionFailed>,
    mut auth_success_events: EventReader<ZoneAuthenticationSuccess>,
    mut auth_failed_events: EventReader<ZoneAuthenticationFailed>,
    mut map_loading_events: EventReader<MapLoadingStarted>,
    mut map_loaded_events: EventReader<MapLoadCompleted>,
    mut map_loading_failed_events: EventReader<MapLoadingFailed>,
    mut actor_init_events: EventReader<ActorInitSent>,
) {
    // Zone server info received → starting connection
    for event in zone_info_events.read() {
        emitter.emit_zone_connecting(event.map_name.clone());
    }

    // Zone server connected (TCP connection established)
    for _event in connected_events.read() {
        emitter.emit_zone_connected();
    }

    // Zone server connection failed
    for event in connection_failed_events.read() {
        emitter.emit_zone_error(event.reason.clone());
    }

    // Zone authentication successful (received ZC_ACCEPT_ENTER)
    for event in auth_success_events.read() {
        emitter.emit_zone_authenticated(event.spawn_x, event.spawn_y);
    }

    // Zone authentication failed (received ZC_REFUSE_ENTER)
    for event in auth_failed_events.read() {
        let error_msg = match event.error_code {
            0 => "Server is full",
            1 => "Server is closed",
            2 => "Already connected",
            3 => "Character not found",
            _ => "Authentication failed",
        };
        error!(
            "Emitting zone-error event: {} (code {})",
            error_msg, event.error_code
        );
        emitter.emit_zone_error(format!("{} (code: {})", error_msg, event.error_code));
    }

    // Map loading started
    for event in map_loading_events.read() {
        emitter.emit_map_loading(event.map_name.clone());
    }

    // Map loading completed
    for event in map_loaded_events.read() {
        emitter.emit_map_loaded(event.map_name.clone());
    }

    // Map loading failed (timeout or asset error)
    for event in map_loading_failed_events.read() {
        error!(
            "Emitting map-loading-failed event for map '{}': {}",
            event.map_name, event.reason
        );
        emitter.emit_map_loading_failed(event.reason.clone());
    }

    // Actor init sent (final step before entering game)
    for _event in actor_init_events.read() {
        emitter.emit_entering_world();
    }
}
