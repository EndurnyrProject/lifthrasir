//! Speech bubbles: what a unit says on the local channel floats over its head on
//! a parchment bubble for a few seconds. A new line from the same unit replaces
//! its bubble.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::inventory::systems::to_item;
use lifthrasir_ui::theme::{self, ro_dark};
use lifthrasir_ui::widgets::chrome::ignore_picking;
use net_contract::dto::{CHAT_LINK_MARK, ChatChannel};
use net_contract::events::ChatHeard;

use crate::widgets::inventory_window::item_name;
use crate::worldspace::{WorldAnchor, WorldOverlaySystems, raised_root};

/// The widest a line of speech runs before it wraps.
const TEXT_WIDTH: f32 = 200.0;
const ROOT_WIDTH: f32 = 240.0;
const BUBBLE_SECS: f32 = 5.0;
const TAIL_SIZE: f32 = 10.0;
/// Pixels from the speaker's origin up to the tip of the bubble's tail.
const BUBBLE_HEAD_GAP: f32 = 78.0;
/// Over every other overlay; see [`super::WORLD_OVERLAY_Z`].
const BUBBLE_Z: i32 = super::WORLD_OVERLAY_Z + 170;
const PARCHMENT: Color = Color::srgba(0.941, 0.894, 0.769, 0.94);

pub struct ChatBubblePlugin;

impl Plugin for ChatBubblePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_bubbles, expire_bubbles)
                .chain()
                .in_set(WorldOverlaySystems::Sync),
        );
    }
}

/// A speaker's bubble, gone when `timer` ends; its [`WorldAnchor`] names the speaker.
#[derive(Component)]
struct ChatBubble {
    timer: Timer,
}

/// The spoken `text` with each link mark replaced by its item's name in brackets.
fn bubble_text(text: &str, links: &[String]) -> String {
    let mut pieces = text.split(CHAT_LINK_MARK);
    let mut spoken = pieces.next().unwrap_or_default().to_string();
    for (index, piece) in pieces.enumerate() {
        if let Some(link) = links.get(index) {
            spoken.push_str(&format!("[{link}]"));
        }
        spoken.push_str(piece);
    }
    spoken
}

fn bubble(text: String) -> impl Scene {
    // A square turned 45 degrees and tucked half under the bubble points down.
    let tilt = UiTransform::from_rotation(Rot2::degrees(45.0));
    let content = bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: {UiRect::bottom(px(TAIL_SIZE / 2.0))},
        }
        @ignore_picking()
        Children [
            Node {
                max_width: px(TEXT_WIDTH + 18.0),
                padding: {UiRect::axes(px(9), px(5))},
                border_radius: BorderRadius::all(px(8)),
            }
            BackgroundColor(PARCHMENT)
            BoxShadow::new(Color::BLACK.with_alpha(0.6), px(0), px(2), px(0), px(6))
            @ignore_picking()
            Children [
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor(ro_dark::INK)
                @ignore_picking()
            ]
            --
            Node {
                width: px(TAIL_SIZE),
                height: px(TAIL_SIZE),
                margin: {UiRect::top(px(-TAIL_SIZE / 2.0))},
            }
            tilt
            BackgroundColor(PARCHMENT)
            @ignore_picking()
        ]
    };
    raised_root(ROOT_WIDTH, BUBBLE_Z, content)
}

/// Raises a bubble over every unit in view that spoke on the local channel.
fn spawn_bubbles(
    mut heard: MessageReader<ChatHeard>,
    mut commands: Commands,
    registry: Res<EntityRegistry>,
    item_db: Option<Res<ItemDb>>,
    bubbles: Query<(Entity, &WorldAnchor), With<ChatBubble>>,
) {
    // The last line of each speaker this frame, so two lines never stack.
    let said: HashMap<Entity, String> = heard
        .read()
        .filter(|line| line.channel == ChatChannel::Local)
        .filter_map(|line| {
            let links: Vec<String> = line
                .links
                .iter()
                .map(|link| item_name(&to_item(link), item_db.as_deref()))
                .collect();
            let speaker = registry.get_entity(line.sender_gid)?;
            Some((speaker, bubble_text(&line.text, &links)))
        })
        .collect();

    for (entity, anchor) in &bubbles {
        if said.contains_key(&anchor.target) {
            commands.entity(entity).despawn();
        }
    }
    for (target, text) in said {
        commands.spawn_scene(bubble(text)).insert((
            WorldAnchor {
                target,
                width: ROOT_WIDTH,
                drop: -BUBBLE_HEAD_GAP,
            },
            ChatBubble {
                timer: Timer::from_seconds(BUBBLE_SECS, TimerMode::Once),
            },
        ));
    }
}

fn expire_bubbles(
    time: Res<Time>,
    mut bubbles: Query<(Entity, &mut ChatBubble)>,
    mut commands: Commands,
) {
    for (entity, mut bubble) in &mut bubbles {
        bubble.timer.tick(time.delta());
        if bubble.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use std::time::Duration;

    const SPEAKER: u32 = 7;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        app.add_message::<ChatHeard>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, (spawn_bubbles, expire_bubbles).chain());
        let speaker = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(SPEAKER, speaker);
        app
    }

    fn say(app: &mut App, channel: ChatChannel, sender_gid: u32, text: &str) {
        app.world_mut().write_message(ChatHeard {
            channel,
            sender_gid,
            text: text.into(),
            ..default()
        });
    }

    fn spoken(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn link_marks_become_bracketed_item_names() {
        let text = format!("S> {CHAT_LINK_MARK} and {CHAT_LINK_MARK}!");
        let links = ["Elunium".to_string(), "Oridecon".to_string()];
        assert_eq!(bubble_text(&text, &links), "S> [Elunium] and [Oridecon]!");
        assert_eq!(bubble_text(&text, &links[..1]), "S> [Elunium] and !");
        assert_eq!(bubble_text("plain", &[]), "plain");
    }

    #[test]
    fn local_speech_raises_one_bubble_that_the_next_line_replaces() {
        let mut app = app();
        say(&mut app, ChatChannel::Local, SPEAKER, "first");
        say(&mut app, ChatChannel::Local, SPEAKER, "second");
        app.update();
        assert_eq!(spoken(&mut app), ["second"]);

        say(&mut app, ChatChannel::Local, SPEAKER, "third");
        app.update();
        assert_eq!(spoken(&mut app), ["third"]);
    }

    #[test]
    fn other_channels_and_unseen_speakers_raise_nothing() {
        let mut app = app();
        say(&mut app, ChatChannel::Party, SPEAKER, "party only");
        say(&mut app, ChatChannel::Local, 99, "out of view");

        app.update();

        assert!(spoken(&mut app).is_empty());
    }

    #[test]
    fn a_bubble_leaves_when_its_time_is_up() {
        let mut app = app();
        say(&mut app, ChatChannel::Local, SPEAKER, "hello");
        app.update();

        let world = app.world_mut();
        for mut bubble in world.query::<&mut ChatBubble>().iter_mut(world) {
            bubble
                .timer
                .set_elapsed(Duration::from_secs_f32(BUBBLE_SECS));
        }
        app.update();

        assert!(spoken(&mut app).is_empty());
    }
}
