use bevy::prelude::*;
use bevy_auto_plugin::modes::global::prelude::auto_add_system;
use game_engine::domain::character::catalog::HeadStyleCatalog;
use game_engine::domain::entities::character::components::Gender;

use crate::bridge::app_bridge::HairstyleInfo;
use crate::bridge::correlation::PendingHairstyleSenders;
use crate::bridge::events::GetHairstylesRequestedEvent;
use crate::plugin::TauriSystems;

#[auto_add_system(
    plugin = crate::plugin::TauriIntegrationAutoPlugin,
    schedule = Update,
    config(in_set = TauriSystems::Handlers)
)]
pub fn handle_get_hairstyles_request(
    mut events: MessageReader<GetHairstylesRequestedEvent>,
    mut hairstyle_senders: ResMut<PendingHairstyleSenders>,
    catalog: Option<Res<HeadStyleCatalog>>,
) {
    for event in events.read() {
        debug!(
            "Processing GetHairstylesRequestedEvent, gender: {}",
            event.gender
        );

        if let Some(sender) = hairstyle_senders.pop_oldest() {
            if let Some(catalog) = catalog.as_ref() {
                let gender = if event.gender == 0 {
                    Gender::Female
                } else {
                    Gender::Male
                };

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
                let error_msg = "HeadStyleCatalog not available".to_string();
                warn!("{}", error_msg);
                let _ = sender.send(Err(error_msg));
            }
        } else {
            error!("No pending sender found for hairstyles request");
        }
    }
}
