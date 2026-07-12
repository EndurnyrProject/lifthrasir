use bevy::prelude::*;
use net_contract::commands::EmoteSent;

use super::table::MAX_EMOTE_ID;

/// Internal intent to send an emote, written by the picker panel and the chat
/// slash parser. Consumed only by [`handle_emote_request`].
#[derive(Message, Debug, Clone)]
pub struct EmoteRequested {
    pub emote_type: u32,
}

/// Client mirror of the server's outbound emote flood window (~1s). Starts
/// finished so the first emote of the session is never gated (mirrors aesir:
/// `last_emote_at == nil` always allows the first emote).
#[derive(Resource)]
pub struct EmoteCooldown(pub Timer);

impl Default for EmoteCooldown {
    fn default() -> Self {
        let mut timer = Timer::from_seconds(1.0, TimerMode::Once);
        timer.finish();
        Self(timer)
    }
}

pub fn tick_emote_cooldown(mut cooldown: ResMut<EmoteCooldown>, time: Res<Time>) {
    cooldown.0.tick(time.delta());
}

/// Gates outbound emote sends: drops out-of-range ids, then converts a ready
/// intent into an `EmoteSent` command and re-arms the cooldown. A second
/// request in the same frame is dropped by the same `is_finished()` check
/// since the reset already happened for the first one.
pub fn handle_emote_request(
    mut requests: MessageReader<EmoteRequested>,
    mut cooldown: ResMut<EmoteCooldown>,
    mut sent: MessageWriter<EmoteSent>,
) {
    for request in requests.read() {
        if request.emote_type >= MAX_EMOTE_ID {
            continue;
        }
        if !cooldown.0.is_finished() {
            continue;
        }
        sent.write(EmoteSent {
            emote_type: request.emote_type,
        });
        cooldown.0.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<EmoteRequested>()
            .add_message::<EmoteSent>()
            .init_resource::<EmoteCooldown>()
            .add_systems(Update, handle_emote_request);
        app
    }

    fn sent(app: &App) -> Vec<EmoteSent> {
        app.world()
            .resource::<Messages<EmoteSent>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn request(app: &mut App, emote_type: u32) {
        app.world_mut()
            .resource_mut::<Messages<EmoteRequested>>()
            .write(EmoteRequested { emote_type });
    }

    #[test]
    fn default_cooldown_starts_finished() {
        assert!(EmoteCooldown::default().0.is_finished());
    }

    #[test]
    fn finished_cooldown_allows_one_emote_and_arms_timer() {
        let mut app = app();
        request(&mut app, 5);
        app.update();

        let msgs = sent(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].emote_type, 5);
        assert!(!app.world().resource::<EmoteCooldown>().0.is_finished());
    }

    #[test]
    fn second_request_same_frame_is_dropped() {
        let mut app = app();
        request(&mut app, 1);
        request(&mut app, 2);
        app.update();

        let msgs = sent(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].emote_type, 1);
    }

    #[test]
    fn out_of_range_emote_type_is_dropped() {
        let mut app = app();
        request(&mut app, MAX_EMOTE_ID);
        app.update();

        assert!(sent(&app).is_empty());
        assert!(app.world().resource::<EmoteCooldown>().0.is_finished());
    }

    #[test]
    fn cooldown_reallows_after_elapsing() {
        let mut app = app();
        request(&mut app, 1);
        app.update();
        assert_eq!(sent(&app).len(), 1);

        app.world_mut().resource_mut::<EmoteCooldown>().0.finish();
        request(&mut app, 2);
        app.update();

        let msgs = sent(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].emote_type, 2);
    }
}
