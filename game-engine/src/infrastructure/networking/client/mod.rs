// Core generic client
pub mod network_client;

// Protocol-specific wrapper clients
pub mod char_server_client;
pub mod zone_server_client;

// Re-export core client
pub use network_client::NetworkClient;

// Re-export wrapper clients, their update systems, and SystemParams
pub use char_server_client::{char_server_update_system, CharServerClient, CharServerEventWriters};
pub use zone_server_client::{
    time_sync_system, zone_server_update_system, ZoneServerClient, ZoneServerEventWriters,
};
