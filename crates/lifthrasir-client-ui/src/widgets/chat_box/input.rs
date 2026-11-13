//! The chat input: its keys, the command palette and where a submitted line goes.

use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use lifthrasir_client::domain::emote::EmoteRequested;
use net_contract::commands::{ChatSent, GmCommandSent, MemoRequested, MountPeco};
use net_contract::events::ChatNotice;

use super::view::{Command, Outgoing, Scope, bind_links, matching_commands, route};
use super::{ChatInput, ChatUi};
use crate::screens::shared::set_field_text;
use crate::widgets::emote::slash::parse_emote_slash;
use crate::widgets::guild_window::slash::{GuildSlashSubmitted, parse_guild_slash};
use crate::widgets::mount::parse_mount_slash;
use crate::widgets::navigation::slash::{NaviSlash, parse_navi_slash};
use crate::widgets::party::slash::{PartySlashSubmitted, parse_party_slash};
use crate::widgets::trade_window::slash::{TradeSlashSubmitted, parse_trade_slash};

#[derive(SystemParam)]
pub(crate) struct ChatInputWriters<'w> {
    chat: MessageWriter<'w, ChatSent>,
    gm: MessageWriter<'w, GmCommandSent>,
    party: MessageWriter<'w, PartySlashSubmitted>,
    trade: MessageWriter<'w, TradeSlashSubmitted>,
    emote: MessageWriter<'w, EmoteRequested>,
    mount: MessageWriter<'w, MountPeco>,
    navi: MessageWriter<'w, NaviSlash>,
    guild: MessageWriter<'w, GuildSlashSubmitted>,
    memo: MessageWriter<'w, MemoRequested>,
    feedback: MessageWriter<'w, ChatNotice>,
}

impl ChatInputWriters<'_> {
    fn refuse(&mut self, message: impl Into<String>) {
        self.feedback.write(ChatNotice {
            error: true,
            message: message.into(),
        });
    }

    /// Runs a client command. The first parser that knows the line takes it; a line
    /// nobody knows is refused, so a mistyped command is never said aloud.
    fn slash(&mut self, line: &str) {
        if let Some(emote_type) = parse_emote_slash(line) {
            self.emote.write(EmoteRequested { emote_type });
            return;
        }
        if let Some(mount) = parse_mount_slash(line) {
            self.mount.write(MountPeco { mount });
            return;
        }
        if let Some(slash) = parse_party_slash(line) {
            self.party.write(PartySlashSubmitted(slash));
            return;
        }
        if let Some(name) = parse_trade_slash(line) {
            self.trade.write(TradeSlashSubmitted(name));
            return;
        }
        if let Some(slash) = parse_navi_slash(line) {
            self.navi.write(slash);
            return;
        }
        if let Some(slash) = parse_guild_slash(line) {
            self.guild.write(slash);
            return;
        }
        let command = line.split_whitespace().next().unwrap_or(line);
        if line == "/memo" {
            self.memo.write(MemoRequested);
            return;
        }
        if command == "/memo" {
            self.refuse("Usage: /memo");
            return;
        }
        self.refuse(format!(
            "Unknown command {command}. Type / to see them all."
        ));
    }
}

/// The chat input with everything a submit touches. Shared by the keyboard and
/// the panel's buttons.
#[derive(SystemParam)]
pub(crate) struct ChatDesk<'w, 's> {
    field: Query<'w, 's, (Entity, &'static mut EditableText), With<ChatInput>>,
    pub(super) ui: ResMut<'w, ChatUi>,
    focus: ResMut<'w, InputFocus>,
    writers: ChatInputWriters<'w>,
}

impl ChatDesk<'_, '_> {
    pub(super) fn refocus(&mut self) {
        if let Ok((entity, _)) = self.field.single() {
            self.focus.set(entity, FocusCause::Navigated);
        }
    }

    /// Writes `command` into the input, ready for its arguments.
    fn complete(&mut self, command: Command) {
        let Ok((_, mut field)) = self.field.single_mut() else {
            return;
        };
        let gap = if command.args.is_empty() { "" } else { " " };
        set_field_text(&mut field, &format!("{}{gap}", command.name));
        self.ui.palette_row = 0;
        self.refocus();
    }

    /// Completes the palette's `row` for the current draft.
    pub(super) fn complete_row(&mut self, row: usize) {
        let Ok((_, field)) = self.field.single() else {
            return;
        };
        let commands = matching_commands(&field.value().to_string());
        if let Some(command) = commands.and_then(|commands| commands.get(row).copied()) {
            self.complete(command);
        }
    }

    /// Appends an item link to the draft and remembers which slot it stands for.
    pub(super) fn link(&mut self, label: String, index: u32) {
        let Ok((_, mut field)) = self.field.single_mut() else {
            return;
        };
        let draft = field.value().to_string();
        let gap = if draft.is_empty() || draft.ends_with(' ') {
            ""
        } else {
            " "
        };
        set_field_text(&mut field, &format!("{draft}{gap}{label} "));
        self.ui.links.push((label, index));
        self.refocus();
    }

    /// Sends the draft where [`route`] says, then clears the input and hands the
    /// keyboard back to the game. A blank draft does nothing and returns `false`.
    pub(super) fn submit(&mut self) -> bool {
        let Ok((_, mut field)) = self.field.single_mut() else {
            return false;
        };
        let Some(outgoing) = route(&field.value().to_string(), &self.ui.target) else {
            return false;
        };
        match outgoing {
            Outgoing::Say { scope, text } => {
                let (text, link_indices) = bind_links(&text, &self.ui.links);
                let whisper_to = match &scope {
                    Scope::Whisper(name) => name.clone(),
                    _ => String::new(),
                };
                self.writers.chat.write(ChatSent {
                    channel: scope.channel(),
                    text,
                    whisper_to,
                    link_indices,
                });
                // A `/w` keeps the input aimed at that hero for the reply.
                if matches!(scope, Scope::Whisper(_)) {
                    self.ui.target = scope;
                }
            }
            Outgoing::Gm(command) => {
                self.writers.gm.write(GmCommandSent { command });
            }
            Outgoing::Slash(line) => self.writers.slash(&line),
            Outgoing::Refused(why) => self.writers.refuse(why),
        }
        field.clear();
        self.ui.links.clear();
        self.ui.menu_open = false;
        self.ui.palette_row = 0;
        self.focus.clear();
        true
    }
}

/// Keyboard control of the chat input.
///
/// - Enter while the input is idle focuses it, which gates gameplay input.
/// - While a command is being typed the palette is open: the arrows move its
///   highlight, and Tab, or Enter on an unfinished name, completes the command.
/// - Enter submits; a blank draft keeps the focus.
/// - Escape closes the channel menu or clears a command draft and keeps typing.
///   Otherwise it releases the input, which `TextInput` does on its own.
///
/// `TextInput` blurs on Escape and tab navigation moves on at Tab, both before
/// this system runs, so the keys also count on the frame right after the input
/// lost the focus.
pub(crate) fn chat_input_control(
    keys: Res<ButtonInput<KeyCode>>,
    mut desk: ChatDesk,
    mut was_focused: Local<bool>,
) {
    let Ok((entity, mut field)) = desk.field.single_mut() else {
        return;
    };
    let focused = desk.focus.get() == Some(entity);
    let typing = focused || *was_focused;
    *was_focused = focused;
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    if !typing {
        if enter {
            desk.focus.set(entity, FocusCause::Navigated);
        }
        return;
    }

    let draft = field.value().to_string();
    if keys.just_pressed(KeyCode::Escape) {
        if desk.ui.menu_open {
            desk.ui.menu_open = false;
        } else if draft.starts_with('/') {
            field.clear();
        } else {
            return;
        }
        desk.focus.set(entity, FocusCause::Navigated);
        return;
    }

    let palette = matching_commands(&draft).filter(|rows| !rows.is_empty() && !desk.ui.menu_open);
    if let Some(rows) = palette {
        let lit = desk.ui.palette_row.min(rows.len() - 1);
        if keys.just_pressed(KeyCode::ArrowDown) {
            desk.ui.palette_row = (lit + 1) % rows.len();
            return;
        }
        if keys.just_pressed(KeyCode::ArrowUp) {
            desk.ui.palette_row = (lit + rows.len() - 1) % rows.len();
            return;
        }
        let unfinished = !draft.eq_ignore_ascii_case(rows[lit].name);
        if keys.just_pressed(KeyCode::Tab) || (enter && unfinished) {
            desk.complete(rows[lit]);
            return;
        }
    }

    if focused && enter {
        desk.submit();
    }
}
