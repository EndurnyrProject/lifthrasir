use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use net_contract::commands::{LocalMapLoaded, LocalPlayerReady};

use crate::domain::entities::markers::LocalPlayer;
use crate::domain::world::map::MapData;

/// Signal the adapter that the local map asset finished loading.
///
/// The adapter latches this and drives its own map-load handshake; the domain no
/// longer touches the zone phase machine.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn notify_map_loaded(
    map_added: Query<(), Added<MapData>>,
    mut w: MessageWriter<LocalMapLoaded>,
) {
    if !map_added.is_empty() {
        w.write(LocalMapLoaded);
    }
}

/// Signal the adapter that the local-player entity exists.
///
/// Fires once on spawn via `Added<LocalPlayer>`; the adapter latches it, so its
/// order relative to the map-load signal does not matter.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn notify_player_ready(
    player_added: Query<(), Added<LocalPlayer>>,
    mut w: MessageWriter<LocalPlayerReady>,
) {
    if !player_added.is_empty() {
        w.write(LocalPlayerReady);
    }
}
