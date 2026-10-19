use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;

use super::super::mapping::mount::mount_result;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use net_contract::events::PecoMountResult;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_mount(
    mut incoming: MessageReader<IncomingMessage>,
    mut results: MessageWriter<PecoMountResult>,
) {
    for msg in incoming.read() {
        if let Body::MountResult(r) = msg.body.clone() {
            results.write(mount_result(r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;
    use net_contract::events::PecoMountRejection;

    #[test]
    fn mount_result_produces_one_rejection_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<PecoMountResult>()
            .add_systems(Update, zone_drain_mount);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::MountResult(net::MountResult {
                    result: net::MountResultCode::MountSkillNotLearned as i32,
                }),
            });
        app.update();

        let results = app.world().resource::<Messages<PecoMountResult>>();
        let events: Vec<_> = results.iter_current_update_messages().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].outcome, Err(PecoMountRejection::SkillNotLearned));
    }
}
