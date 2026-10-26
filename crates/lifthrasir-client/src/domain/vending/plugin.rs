use bevy::prelude::*;

use super::state::{RevealVendingSeller, VendingIntent, VendingNotice, VendingState};
use super::systems;

/// Ordered vending stages: authority ingress, local intents, UI projection.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VendingSystems {
    Ingress,
    Intents,
    Projection,
}

pub struct VendingPlugin;

impl Plugin for VendingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VendingState>()
            .add_message::<VendingIntent>()
            .add_message::<VendingNotice>()
            .add_message::<RevealVendingSeller>()
            .configure_sets(
                Update,
                (
                    VendingSystems::Ingress,
                    VendingSystems::Intents,
                    VendingSystems::Projection,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    systems::reset_vending_session,
                    systems::apply_vending_ingress,
                    systems::mark_unconfirmed_mutations,
                )
                    .chain()
                    .in_set(VendingSystems::Ingress),
            )
            .add_systems(
                Update,
                (
                    systems::handle_vending_intents,
                    systems::send_queued_refresh,
                )
                    .chain()
                    .in_set(VendingSystems::Intents),
            )
            .add_systems(
                Update,
                systems::project_vending_notices.in_set(VendingSystems::Projection),
            );
    }
}
