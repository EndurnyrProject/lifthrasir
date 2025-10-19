use bevy::prelude::*;
use game_engine::domain::character::catalog::HeadStyleCatalog;
use game_engine::domain::entities::character::components::Gender;

use crate::bridge::app_bridge::HairstyleInfo;
use crate::bridge::events::GetHairstylesRequestedEvent;
use crate::bridge::pending_senders::PendingSenders;

/// System that handles GetHairstylesRequestedEvent
/// This is a synchronous query that responds immediately from the catalog
pub fn handle_get_hairstyles_request(
    mut events: MessageReader<GetHairstylesRequestedEvent>,
    mut pending: ResMut<PendingSenders>,
    catalog: Option<Res<HeadStyleCatalog>>,
) {
    for event in events.read() {
        debug!(
            "Processing GetHairstylesRequestedEvent, gender: {}, request_id: {}",
            event.gender, event.request_id
        );

        // Respond immediately since this is a synchronous query
        if let Some(sender) = pending.hairstyles.senders.remove(&event.request_id) {
            if let Some(catalog) = catalog.as_ref() {
                // Convert gender u8 to Gender enum
                let gender = if event.gender == 0 {
                    Gender::Female
                } else {
                    Gender::Male
                };

                // Get available hairstyles for this gender
                let hairstyles: Vec<HairstyleInfo> = catalog
                    .get_all(gender)
                    .iter()
                    .map(|entry| HairstyleInfo {
                        id: entry.id,
                        available_colors: entry.available_colors.clone(),
                    })
                    .collect();

                debug!(
                    "Found {} hairstyles for gender {}, sending response to UI",
                    hairstyles.len(),
                    event.gender
                );

                let _ = sender.send(Ok(hairstyles));
            } else {
                // No catalog available - send error
                let error_msg = "HeadStyleCatalog not available".to_string();
                warn!("{}", error_msg);
                let _ = sender.send(Err(error_msg));
            }
        } else {
            error!(
                "No pending sender found for hairstyles request_id: {}",
                event.request_id
            );
        }
    }
}
