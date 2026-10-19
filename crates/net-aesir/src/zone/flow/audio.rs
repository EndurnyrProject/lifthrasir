use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::client_connected;
use net_contract::events::PlaySoundEffect;

use super::super::mapping::audio::sound_effect;
use crate::dispatch::IncomingMessage;
use crate::envelope::Body;

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn zone_drain_audio(
    mut incoming: MessageReader<IncomingMessage>,
    mut sound_effects: MessageWriter<PlaySoundEffect>,
) {
    for msg in incoming.read() {
        let Body::SoundEffect(effect) = msg.body.clone() else {
            continue;
        };
        if let Some(event) = sound_effect(effect) {
            sound_effects.write(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::GAMEPLAY;
    use crate::proto::aesir::net;

    #[test]
    fn sound_effect_drains_to_event() {
        let mut app = App::new();
        app.add_message::<IncomingMessage>()
            .add_message::<PlaySoundEffect>()
            .add_systems(Update, zone_drain_audio);

        app.world_mut()
            .resource_mut::<Messages<IncomingMessage>>()
            .write(IncomingMessage {
                channel: GAMEPLAY,
                body: Body::SoundEffect(net::SoundEffect {
                    name: "effect\\door.wav".into(),
                    r#type: 0,
                }),
            });
        app.update();

        let events = app.world().resource::<Messages<PlaySoundEffect>>();
        let drained: Vec<_> = events.iter_current_update_messages().collect();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].name, "effect\\door.wav");
    }
}
