//! Guild creation through `/guild "guild name"`, never through public chat.

use bevy::prelude::*;
use net_contract::commands::GuildCreateRequested;

use super::{GuildMutationContext, GuildUiSession, request_create};
use crate::theme;
use crate::widgets::chat_box::{ChatHistory, append_colored_line};

const USAGE: &str = "Usage: /guild \"guild name\" (requires 1 Emperium).";

/// Recognized commands include syntax errors so they cannot leak into public chat.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuildSlashSubmitted(pub Result<String, &'static str>);

pub(crate) fn parse_guild_slash(input: &str) -> Option<GuildSlashSubmitted> {
    let input = input.trim();
    let (command, argument) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
    if command != "/guild" {
        return None;
    }
    let name = argument
        .trim()
        .strip_prefix('"')
        .and_then(|name| name.strip_suffix('"'))
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.contains('"'))
        .ok_or(USAGE)
        .and_then(|name| {
            if name.chars().count() > 24 {
                Err("Guild names must be at most 24 characters.")
            } else {
                Ok(name.to_string())
            }
        });
    Some(GuildSlashSubmitted(name))
}

pub(super) fn dispatch_guild_slash(
    mut submitted: MessageReader<GuildSlashSubmitted>,
    mut context: GuildMutationContext,
    session: Res<GuildUiSession>,
    mut create: MessageWriter<GuildCreateRequested>,
    container: Query<Entity, With<ChatHistory>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if session.blocked
        || session.generation != *context.generation
        || session.char_id != context.session.char_id
        || context.session.char_id == 0
    {
        submitted.clear();
        return;
    }
    for GuildSlashSubmitted(name) in submitted.read() {
        let error = match name {
            Err(error) => Some(*error),
            Ok(_) if context.guild.in_guild() => Some("Character already belongs to a guild"),
            Ok(name) => {
                if let Some(command) = request_create(&mut context.ui, *context.generation, name) {
                    create.write(command);
                }
                None
            }
        };
        if let Some(error) = error {
            context.ui.feedback = Some(error.to_string());
            context.ui.feedback_is_error = true;
        }
        if let Ok(container) = container.single()
            && let Some(text) = &context.ui.feedback
        {
            let color = if context.ui.feedback_is_error {
                theme::BAD
            } else {
                theme::GOLD
            };
            append_colored_line(
                &mut commands,
                container,
                text,
                color,
                asset_server.load(theme::FONT_BODY),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::GuildUi;
    use super::*;
    use game_engine::domain::guild::GuildState;
    use net_contract::state::{ZoneSession, ZoneSessionGeneration};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.add_message::<GuildSlashSubmitted>()
            .add_message::<GuildCreateRequested>()
            .init_resource::<GuildUi>()
            .init_resource::<GuildState>()
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(ZoneSessionGeneration(1))
            .insert_resource(GuildUiSession {
                generation: ZoneSessionGeneration(1),
                char_id: 42,
                ..default()
            })
            .add_systems(Update, dispatch_guild_slash);
        app.world_mut().spawn(ChatHistory);
        app
    }

    fn creates(app: &App) -> Vec<GuildCreateRequested> {
        let messages = app.world().resource::<Messages<GuildCreateRequested>>();
        messages.get_cursor().read(messages).cloned().collect()
    }

    #[test]
    fn malformed_commands_show_usage_without_sending_a_request() {
        for input in [
            "/guild",
            "/guild Vikings",
            "/guild \"\"",
            "/guild \"   \"",
            "/guild \"Vikings",
            "/guild \"Vikings\" extra",
            "/guild \"A\" \"B\"",
        ] {
            let mut app = app();
            app.world_mut()
                .write_message(parse_guild_slash(input).unwrap());
            app.update();

            assert!(creates(&app).is_empty(), "{input}");
            assert!(app.world().resource::<GuildUi>().pending.is_none());
            assert_eq!(
                app.world_mut()
                    .query::<&Text>()
                    .single(app.world())
                    .unwrap()
                    .0,
                USAGE
            );
        }
    }

    #[test]
    fn name_limit_counts_characters_and_other_commands_fall_through() {
        let name = "龍".repeat(24);
        assert_eq!(
            parse_guild_slash(&format!(" /guild\t\"{name}\" ")),
            Some(GuildSlashSubmitted(Ok(name.clone())))
        );
        assert!(
            parse_guild_slash(&format!("/guild \"{name}龍\""))
                .unwrap()
                .0
                .is_err()
        );
        for input in ["hello", "/guilds \"Vikings\"", "/guildchat hello"] {
            assert_eq!(parse_guild_slash(input), None);
        }
    }

    #[test]
    fn disconnected_or_replaced_sessions_discard_queued_commands() {
        for (blocked, generation, char_id) in [(true, 1, 42), (false, 2, 42), (false, 1, 43)] {
            let mut app = app();
            *app.world_mut().resource_mut::<GuildUiSession>() = GuildUiSession {
                blocked,
                generation: ZoneSessionGeneration(generation),
                char_id,
                ..default()
            };
            app.world_mut()
                .write_message(parse_guild_slash("/guild \"Vikings\"").unwrap());
            app.update();
            assert!(creates(&app).is_empty());
            assert!(app.world().resource::<GuildUi>().pending.is_none());
        }
    }

    #[test]
    fn quoted_command_creates_once_and_waits_for_server_membership() {
        let mut app = app();
        app.world_mut()
            .write_message(parse_guild_slash("/guild \"  Viking Guild  \"").unwrap());
        app.update();

        let written = creates(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].name, "Viking Guild");
        assert!(!app.world().resource::<GuildState>().in_guild());
        assert_eq!(
            app.world()
                .resource::<GuildUi>()
                .pending
                .as_ref()
                .unwrap()
                .action,
            "create"
        );

        app.world_mut()
            .write_message(parse_guild_slash("/guild \"Other\"").unwrap());
        app.update();
        assert_eq!(
            creates(&app).len(),
            1,
            "second command must not send another request"
        );
        assert_eq!(
            app.world().resource::<GuildUi>().feedback.as_deref(),
            Some("A guild action is already pending.")
        );
    }
}
