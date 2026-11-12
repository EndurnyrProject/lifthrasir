use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::{HomunculusResultReceived, HomunculusState, HomunculusStateReceived};

use crate::dispatch::IncomingMessage;
use crate::envelope::Body;
use crate::proto::aesir::net::HomunculusPrivateState;

fn map_state(state: &HomunculusPrivateState) -> HomunculusState {
    HomunculusState {
        world_id: state.world_gid,
        name: state.name.clone(),
        level: state.level,
        hp: state.hp,
        max_hp: state.max_hp,
        sp: state.sp,
        max_sp: state.max_sp,
        exp: state.exp,
        next_exp: state.next_exp,
        hunger: state.hunger,
        intimacy_hundredths: state.intimacy_hundredths,
        skill_points: state.skill_points,
    }
}

#[auto_add_system(plugin = crate::AesirNetPlugin, schedule = Update, config(run_if = client_connected))]
fn receive_homunculus(
    mut incoming: MessageReader<IncomingMessage>,
    mut states: MessageWriter<HomunculusStateReceived>,
    mut results: MessageWriter<HomunculusResultReceived>,
) {
    for message in incoming.read() {
        match &message.body {
            Body::HomunculusPrivateState(state) => {
                states.write(HomunculusStateReceived(map_state(state)));
            }
            Body::HomunculusResult(result) => {
                results.write(HomunculusResultReceived {
                    request_id: result.request_id,
                    success: result.success,
                    error: result.error,
                    state: result.state.as_ref().map(map_state),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::{BULK, GAMEPLAY};
    use crate::proto::aesir::net::HomunculusResult;

    #[test]
    fn forwards_private_state_and_correlated_result() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<HomunculusStateReceived>()
            .add_message::<HomunculusResultReceived>()
            .add_systems(Update, receive_homunculus);
        let state = HomunculusPrivateState {
            name: "Lif".into(),
            hp: 17,
            max_hp: 30,
            ..Default::default()
        };
        let mut messages = app.world_mut().resource_mut::<Messages<IncomingMessage>>();
        messages.write(IncomingMessage {
            channel: BULK,
            body: Body::HomunculusPrivateState(state.clone()),
        });
        messages.write(IncomingMessage {
            channel: GAMEPLAY,
            body: Body::HomunculusResult(HomunculusResult {
                request_id: 9,
                success: true,
                state: Some(state),
                ..Default::default()
            }),
        });
        app.update();
        let world = app.world();
        let state = &world
            .resource::<Messages<HomunculusStateReceived>>()
            .iter_current_update_messages()
            .next()
            .unwrap()
            .0;
        assert_eq!((&*state.name, state.hp, state.max_hp), ("Lif", 17, 30));
        let result = world
            .resource::<Messages<HomunculusResultReceived>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(result.request_id, 9);
        assert_eq!(result.state.as_ref(), Some(state));
    }
}
