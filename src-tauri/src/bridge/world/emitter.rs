use bevy::prelude::*;
use game_engine::domain::character::events::{
    ActorInitSent, MapLoadCompleted, MapLoadingFailed, MapLoadingStarted, ZoneAuthenticationFailed,
    ZoneAuthenticationSuccess, ZoneServerConnected, ZoneServerConnectionFailed,
    ZoneServerInfoReceivedEvent,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Resource that allows world/zone systems to emit events to Tauri/React
#[derive(Resource)]
pub struct WorldEmitter {
    app_handle: AppHandle,
}

impl WorldEmitter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// Emit zone-connecting event with map name
    pub fn emit_zone_connecting(&self, map_name: String) {
        #[derive(Serialize, Clone)]
        struct ZoneConnectingPayload {
            map_name: String,
        }

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
        #[derive(Serialize, Clone)]
        struct ZoneAuthenticatedPayload {
            spawn_x: u16,
            spawn_y: u16,
        }

        let payload = ZoneAuthenticatedPayload { spawn_x, spawn_y };
        if let Err(e) = self.app_handle.emit("zone-authenticated", payload) {
            error!("Failed to emit zone-authenticated event: {:?}", e);
        }
    }

    /// Emit map-loading event
    pub fn emit_map_loading(&self, map_name: String) {
        #[derive(Serialize, Clone)]
        struct MapLoadingPayload {
            map_name: String,
        }

        let payload = MapLoadingPayload { map_name };
        if let Err(e) = self.app_handle.emit("map-loading", payload) {
            error!("Failed to emit map-loading event: {:?}", e);
        }
    }

    /// Emit map-loaded event
    pub fn emit_map_loaded(&self, map_name: String) {
        #[derive(Serialize, Clone)]
        struct MapLoadedPayload {
            map_name: String,
        }

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
        #[derive(Serialize, Clone)]
        struct ZoneErrorPayload {
            error: String,
        }

        let payload = ZoneErrorPayload { error };
        if let Err(e) = self.app_handle.emit("zone-error", payload) {
            error!("Failed to emit zone-error event: {:?}", e);
        }
    }

    /// Emit map-loading-failed event with error message
    pub fn emit_map_loading_failed(&self, error: String) {
        #[derive(Serialize, Clone)]
        struct MapLoadingFailedPayload {
            error: String,
        }

        let payload = MapLoadingFailedPayload { error };
        if let Err(e) = self.app_handle.emit("map-loading-failed", payload) {
            error!("Failed to emit map-loading-failed event: {:?}", e);
        }
    }
}

/// System that listens to zone-related events and emits them to Tauri/React
#[allow(clippy::too_many_arguments)]
pub fn emit_world_events(
    emitter: Res<WorldEmitter>,
    mut zone_info_events: MessageReader<ZoneServerInfoReceivedEvent>,
    mut connected_events: MessageReader<ZoneServerConnected>,
    mut connection_failed_events: MessageReader<ZoneServerConnectionFailed>,
    mut auth_success_events: MessageReader<ZoneAuthenticationSuccess>,
    mut auth_failed_events: MessageReader<ZoneAuthenticationFailed>,
    mut map_loading_events: MessageReader<MapLoadingStarted>,
    mut map_loaded_events: MessageReader<MapLoadCompleted>,
    mut map_loading_failed_events: MessageReader<MapLoadingFailed>,
    mut actor_init_events: MessageReader<ActorInitSent>,
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
