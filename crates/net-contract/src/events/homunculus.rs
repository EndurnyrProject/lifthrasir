//! Owner-private ingress, scoped to the current zone connection.

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

use crate::{
    dto::{HomunculusError, HomunculusSnapshot},
    state::ZoneSessionGeneration,
};

#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct HomunculusIngress {
    pub generation: ZoneSessionGeneration,
    pub sequence: u32,
    pub payload: HomunculusIngressPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomunculusIngressPayload {
    State(HomunculusSnapshot),
    Removed {
        durable_id: u64,
    },
    Result {
        request_id: u64,
        success: bool,
        error: HomunculusError,
        state: Option<HomunculusSnapshot>,
    },
    MapBarrier,
}

/// A local rejection, not an authoritative server result.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct HomunculusSendFailed {
    pub generation: ZoneSessionGeneration,
    pub request_id: u64,
    pub reason: String,
}

/// The connection can accept commands again, including after a same-zone warp.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct HomunculusReady {
    pub generation: ZoneSessionGeneration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::HomunculusCommand;

    #[test]
    fn homunculus_messages_are_registered() {
        let mut app = App::new();
        app.add_plugins(crate::NetContractPlugin);
        let world = app.world();
        assert!(world.contains_resource::<Messages<HomunculusCommand>>());
        assert!(world.contains_resource::<Messages<HomunculusIngress>>());
        assert!(world.contains_resource::<Messages<HomunculusSendFailed>>());
        assert!(world.contains_resource::<Messages<HomunculusReady>>());
    }
}
