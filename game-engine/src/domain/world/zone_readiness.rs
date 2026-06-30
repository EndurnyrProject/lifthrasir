use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::QuinnetClient;

use crate::domain::entities::markers::LocalPlayer;
use crate::domain::world::map::MapData;
use crate::infrastructure::networking::quic::channels::CONTROL;
use crate::infrastructure::networking::quic::envelope::Body;
use crate::infrastructure::networking::quic::proto::aesir::net::MapLoaded;
use crate::infrastructure::networking::quic::zone::{QuicZoneState, ZonePhase};

/// Pure outcome of the map asset becoming ready: the next phase, or `None` when out of phase.
fn map_loaded_next(phase: ZonePhase) -> Option<ZonePhase> {
    (phase == ZonePhase::Entering).then_some(ZonePhase::MapReady)
}

/// Pure outcome of the local player entity becoming ready: the next phase, or
/// `None` when out of phase.
fn player_ready_next(phase: ZonePhase) -> Option<ZonePhase> {
    (phase == ZonePhase::MapReady).then_some(ZonePhase::Playing)
}

/// On the map asset becoming ready while `Entering`, send `MapLoaded` (legacy CZ_NOTIFY_ACTORINIT).
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn zone_send_map_loaded(
    map_added: Query<(), Added<MapData>>,
    mut client: ResMut<QuinnetClient>,
    mut state: ResMut<QuicZoneState>,
) {
    if map_added.is_empty() {
        return;
    }
    let Some(next) = map_loaded_next(state.phase) else {
        return;
    };
    if let Err(e) = state.send(&mut client, CONTROL, Body::MapLoaded(MapLoaded {})) {
        error!("failed to send MapLoaded: {e}");
        state.phase = ZonePhase::Failed;
        return;
    }
    state.phase = next;
}

/// Once the map is `MapReady` and the local-player entity exists, enter `Playing`.
/// This is the gate the gameplay senders (movement, chat, name lookups, input)
/// wait on, so without it those requests are silently dropped.
#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn zone_enter_playing(player: Query<(), With<LocalPlayer>>, mut state: ResMut<QuicZoneState>) {
    if player.is_empty() {
        return;
    }
    let Some(next) = player_ready_next(state.phase) else {
        return;
    };
    state.phase = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_loaded_in_entering_advances_to_map_ready() {
        assert_eq!(
            map_loaded_next(ZonePhase::Entering),
            Some(ZonePhase::MapReady)
        );
    }

    #[test]
    fn map_loaded_out_of_phase_is_ignored() {
        assert_eq!(map_loaded_next(ZonePhase::AuthSent), None);
        assert_eq!(map_loaded_next(ZonePhase::MapReady), None);
    }

    #[test]
    fn player_ready_in_map_ready_advances_to_playing() {
        assert_eq!(
            player_ready_next(ZonePhase::MapReady),
            Some(ZonePhase::Playing)
        );
    }

    #[test]
    fn player_ready_out_of_phase_is_ignored() {
        assert_eq!(player_ready_next(ZonePhase::Entering), None);
        assert_eq!(player_ready_next(ZonePhase::Playing), None);
    }
}
