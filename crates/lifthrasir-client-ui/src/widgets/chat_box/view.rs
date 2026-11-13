//! What the chat panel shows and where a typed line goes, worked out from plain
//! data so it is testable without a window.

use bevy::prelude::*;
use lifthrasir_ui::theme::ro_dark;
use net_contract::dto::{CHAT_LINK_MARK, CHAT_MAX_LINKS, ChatChannel};
use net_contract::events::ChatHeard;

use super::ChatUi;

pub(super) const WHISPER: Color = Color::srgb_u8(0xc9, 0x8a, 0x7a);
pub(super) const WHISPER_NAME: Color = Color::srgb_u8(0xe0, 0x7a, 0x6e);
/// Lines that report a failure.
pub(super) const ERROR: Color = WHISPER_NAME;
const PARTY_NAME: Color = Color::srgb_u8(0xa9, 0xd3, 0x94);
const TRADE_NAME: Color = Color::srgb_u8(0xd6, 0xf0, 0xff);

const WHISPER_USAGE: &str = "Usage: /w <name> <message>";
/// Rows the command palette shows at once.
const PALETTE_ROWS: usize = 7;

/// Who hears a line: a channel, or one hero for a whisper. It names both where a
/// line in the log belongs and where the input sends.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum Scope {
    #[default]
    Local,
    Party,
    Guild,
    Trade,
    /// A whisper with the hero of this name.
    Whisper(String),
}

impl Scope {
    pub(super) fn channel(&self) -> ChatChannel {
        match self {
            Scope::Local => ChatChannel::Local,
            Scope::Party => ChatChannel::Party,
            Scope::Guild => ChatChannel::Guild,
            Scope::Trade => ChatChannel::Trade,
            Scope::Whisper(_) => ChatChannel::Whisper,
        }
    }

    pub(super) fn label(&self) -> &str {
        match self {
            Scope::Local => "Local",
            Scope::Party => "Party",
            Scope::Guild => "Guild",
            Scope::Trade => "Trade",
            Scope::Whisper(name) => name,
        }
    }

    pub(super) fn color(&self) -> Color {
        match self {
            Scope::Local => ro_dark::BODY,
            Scope::Party => ro_dark::STATUS_ONLINE,
            Scope::Guild => ro_dark::STATUS_BUSY,
            Scope::Trade => ro_dark::CHECK_MARK,
            Scope::Whisper(_) => WHISPER,
        }
    }

    fn name_color(&self) -> Color {
        match self {
            Scope::Local => ro_dark::STRONG,
            Scope::Party => PARTY_NAME,
            Scope::Guild => ro_dark::BRONZE_LIGHT,
            Scope::Trade => TRADE_NAME,
            Scope::Whisper(_) => WHISPER_NAME,
        }
    }

    /// The channel tag in front of a line. Local and whisper lines have none.
    fn tag(&self) -> &'static str {
        match self {
            Scope::Party => "[Party] ",
            Scope::Guild => "[Guild] ",
            Scope::Trade => "[Trade] ",
            Scope::Local | Scope::Whisper(_) => "",
        }
    }
}

/// What a click on a piece of a line does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RunAction {
    /// Aims the input at a whisper to this hero.
    Whisper(String),
    /// Opens the n-th linked item of the line.
    Link(usize),
}

/// One coloured piece of a line.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Run {
    pub text: String,
    pub color: Color,
    pub action: Option<RunAction>,
    /// The channel tag, hidden on its own channel's tab.
    pub tag: bool,
}

impl Run {
    fn plain(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
            action: None,
            tag: false,
        }
    }
}

/// A heard line, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Speech {
    pub scope: Scope,
    pub runs: Vec<Run>,
    /// Someone else wrote the player's name.
    pub mention: bool,
    /// The player said it.
    pub mine: bool,
}

/// Lays out a heard line for the player named `me`. `links` holds the label and
/// rarity of each linked item, in order.
pub(super) fn speech(heard: &ChatHeard, me: &str, links: &[(String, bool)]) -> Speech {
    let mine = !me.is_empty() && heard.sender_name == me;
    let scope = match heard.channel {
        ChatChannel::Local => Scope::Local,
        ChatChannel::Party => Scope::Party,
        ChatChannel::Guild => Scope::Guild,
        ChatChannel::Trade => Scope::Trade,
        ChatChannel::Whisper if mine => Scope::Whisper(heard.whisper_to.clone()),
        ChatChannel::Whisper => Scope::Whisper(heard.sender_name.clone()),
    };
    let color = scope.color();
    let mut runs = Vec::new();
    if !scope.tag().is_empty() {
        runs.push(Run {
            tag: true,
            ..Run::plain(scope.tag(), color)
        });
    }
    if !heard.sender_name.is_empty() {
        runs.push(speaker(&scope, &heard.sender_name, mine));
        runs.push(Run::plain(" : ", color));
    }

    let mut mention = false;
    for (index, piece) in heard.text.split(CHAT_LINK_MARK).enumerate() {
        if let Some((label, rare)) = index.checked_sub(1).and_then(|link| links.get(link)) {
            let ink = if *rare {
                ro_dark::RARE_RING
            } else {
                ro_dark::BRONZE_LIGHT
            };
            runs.push(Run {
                action: Some(RunAction::Link(index - 1)),
                ..Run::plain(format!("[{label}]"), ink)
            });
        }
        for (text, named) in split_mentions(piece, if mine { "" } else { me }) {
            mention |= named;
            let ink = if named { ro_dark::FROST_GLOW } else { color };
            runs.push(Run::plain(text, ink));
        }
    }
    Speech {
        scope,
        runs,
        mention,
        mine,
    }
}

/// The speaker's name. Clicking it aims a whisper at them, unless it is the
/// player's own name on a public channel.
fn speaker(scope: &Scope, sender: &str, mine: bool) -> Run {
    if let Scope::Whisper(peer) = scope {
        let way = if mine { "To" } else { "From" };
        return Run {
            action: Some(RunAction::Whisper(peer.clone())),
            ..Run::plain(format!("({way} {peer})"), scope.name_color())
        };
    }
    if mine {
        return Run::plain(sender, ro_dark::STRONG);
    }
    Run {
        action: Some(RunAction::Whisper(sender.to_string())),
        ..Run::plain(sender, scope.name_color())
    }
}

/// Splits `text` around whole-word uses of `name`, flagging those. An empty
/// `name` matches nothing.
fn split_mentions(text: &str, name: &str) -> Vec<(String, bool)> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut from = 0;
    while !name.is_empty() {
        let Some(at) = text[from..].find(name).map(|found| found + from) else {
            break;
        };
        let end = at + name.len();
        from = end;
        let inside_word = text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
            || text[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if inside_word {
            continue;
        }
        if at > start {
            pieces.push((text[start..at].to_string(), false));
        }
        pieces.push((name.to_string(), true));
        start = end;
    }
    if start < text.len() {
        pieces.push((text[start..].to_string(), false));
    }
    pieces
}

/// Unseen lines of a tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Unread {
    pub count: u32,
    /// One of them names the player.
    pub ping: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TabView {
    /// `None` is the All tab.
    pub scope: Option<Scope>,
    pub active: bool,
    pub unread: Unread,
}

impl TabView {
    pub(super) fn label(&self) -> &str {
        self.scope.as_ref().map_or("All", Scope::label)
    }

    /// The unread badge, capped at `9+`.
    pub(super) fn badge(&self) -> Option<String> {
        match self.unread.count {
            0 => None,
            count @ 1..=9 => Some(count.to_string()),
            _ => Some("9+".into()),
        }
    }
}

/// The tab strip and the two toggles beside it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HeadView {
    pub tabs: Vec<TabView>,
    pub show_time: bool,
    pub collapsed: bool,
}

pub(super) fn head(ui: &ChatUi) -> HeadView {
    let channels = [
        None,
        Some(Scope::Party),
        Some(Scope::Guild),
        Some(Scope::Trade),
    ];
    let whispers = ui.whispers.iter().cloned().map(Scope::Whisper).map(Some);
    let tabs = channels
        .into_iter()
        .chain(whispers)
        .map(|scope| TabView {
            active: ui.tab == scope,
            unread: scope
                .as_ref()
                .and_then(|scope| ui.unread.get(scope))
                .copied()
                .unwrap_or_default(),
            scope,
        })
        .collect();
    HeadView {
        tabs,
        show_time: ui.show_time,
        collapsed: ui.collapsed,
    }
}

/// One row of the channel menu.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MenuRow {
    pub scope: Scope,
    /// The prefix or command that reaches the channel without the menu.
    pub hint: &'static str,
    pub active: bool,
}

impl MenuRow {
    pub(super) fn label(&self) -> String {
        match &self.scope {
            Scope::Whisper(name) => format!("Whisper {name}"),
            scope => scope.label().to_string(),
        }
    }
}

/// A slash command the palette lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Command {
    pub name: &'static str,
    pub args: &'static str,
    pub about: &'static str,
}

const fn command(name: &'static str, args: &'static str, about: &'static str) -> Command {
    Command { name, args, about }
}

/// The commands the chat input runs. Emote aliases also work and are not listed.
pub(super) const COMMANDS: [Command; 11] = [
    command("/w", "<name> <message>", "Whisper a hero"),
    command("/pcreate", "<name>", "Create a party"),
    command("/pinvite", "<name>", "Invite a hero to your party"),
    command("/paccept", "", "Accept a party invitation"),
    command("/pdecline", "", "Decline a party invitation"),
    command("/guild", "\"<name>\"", "Found a guild"),
    command("/trade", "<name>", "Ask a hero to trade"),
    command("/memo", "", "Save this spot as a warp point"),
    command("/navi", "<map> [x y]", "Plot a route on the minimap"),
    command("/mount", "", "Mount your Peco Peco"),
    command("/unmount", "", "Dismount"),
];

/// The commands a draft could still become: it starts with `/` and has no
/// argument yet. `None` once the draft is anything else.
pub(super) fn matching_commands(draft: &str) -> Option<Vec<Command>> {
    if !draft.starts_with('/') || draft.contains(char::is_whitespace) {
        return None;
    }
    let typed = draft.to_lowercase();
    Some(
        COMMANDS
            .into_iter()
            .filter(|command| command.name.starts_with(&typed))
            .take(PALETTE_ROWS)
            .collect(),
    )
}

/// What floats over the input row.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PopupView {
    /// The channel menu.
    Menu(Vec<MenuRow>),
    /// The command palette for `draft`; the flag marks the highlighted row.
    Palette {
        rows: Vec<(Command, bool)>,
        draft: String,
    },
}

/// The channel menu while it is open, else the palette while a command is typed.
pub(super) fn popup(ui: &ChatUi, draft: &str) -> Option<PopupView> {
    if ui.menu_open {
        let row = |scope: Scope, hint| MenuRow {
            active: ui.target == scope,
            scope,
            hint,
        };
        let whispers = ui.whispers.iter().cloned().map(Scope::Whisper);
        return Some(PopupView::Menu(
            [
                row(Scope::Local, ""),
                row(Scope::Party, "%"),
                row(Scope::Guild, "$"),
                row(Scope::Trade, ""),
            ]
            .into_iter()
            .chain(whispers.map(|scope| row(scope, "/w")))
            .collect(),
        ));
    }
    let commands = matching_commands(draft)?;
    let lit = ui.palette_row.min(commands.len().saturating_sub(1));
    Some(PopupView::Palette {
        rows: commands
            .into_iter()
            .enumerate()
            .map(|(row, command)| (command, row == lit))
            .collect(),
        draft: draft.to_string(),
    })
}

/// The hint in the empty input.
pub(super) fn hint(target: &Scope) -> String {
    let verb = match target {
        Scope::Whisper(_) => "Whisper to",
        _ => "Say to",
    };
    format!("{verb} {} \u{b7} / for commands", target.label())
}

/// Where a submitted draft goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outgoing {
    Say {
        scope: Scope,
        text: String,
    },
    /// A GM command, without its `@`.
    Gm(String),
    /// A slash command for the client's own parsers.
    Slash(String),
    /// A malformed command; the text tells the player why.
    Refused(&'static str),
}

/// Reads a draft typed while the input aims at `target`: `@` runs a GM command,
/// `/w` whispers, any other `/` is a client command, and `%` or `$` speak to the
/// party or the guild for this one line. A blank draft goes nowhere.
pub(super) fn route(draft: &str, target: &Scope) -> Option<Outgoing> {
    let draft = draft.trim();
    if let Some(command) = draft.strip_prefix('@') {
        let command = command.trim();
        return (!command.is_empty()).then(|| Outgoing::Gm(command.to_string()));
    }
    if draft.starts_with('/') {
        let (command, rest) = draft.split_once(char::is_whitespace).unwrap_or((draft, ""));
        if command != "/w" {
            return Some(Outgoing::Slash(draft.to_string()));
        }
        return Some(whisper_slash(rest.trim()).unwrap_or(Outgoing::Refused(WHISPER_USAGE)));
    }
    let (scope, text) = match draft.chars().next() {
        Some('%') => (Scope::Party, draft[1..].trim()),
        Some('$') => (Scope::Guild, draft[1..].trim()),
        _ => (target.clone(), draft),
    };
    (!text.is_empty()).then(|| Outgoing::Say {
        scope,
        text: text.to_string(),
    })
}

/// `<name> <message>`; a name with spaces goes in double quotes.
fn whisper_slash(rest: &str) -> Option<Outgoing> {
    let (name, text) = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split_once('"')?,
        None => rest.split_once(char::is_whitespace)?,
    };
    let (name, text) = (name.trim(), text.trim());
    (!name.is_empty() && !text.is_empty()).then(|| Outgoing::Say {
        scope: Scope::Whisper(name.to_string()),
        text: text.to_string(),
    })
}

/// Swaps each pending link label still present in `text` for a link mark and
/// returns the inventory indices in the order they appear. A label the player
/// edited away stays plain text.
pub(super) fn bind_links(text: &str, pending: &[(String, u32)]) -> (String, Vec<u32>) {
    let text = text.replace(CHAT_LINK_MARK, "");
    let mut pending = pending.to_vec();
    let mut bound = String::new();
    let mut indices = Vec::new();
    let mut rest = text.as_str();
    while indices.len() < CHAT_MAX_LINKS {
        let first = pending
            .iter()
            .enumerate()
            .filter_map(|(slot, (label, _))| rest.find(label.as_str()).map(|at| (at, slot)))
            .min();
        let Some((at, slot)) = first else {
            break;
        };
        let (label, index) = pending.remove(slot);
        bound.push_str(&rest[..at]);
        bound.push(CHAT_LINK_MARK);
        indices.push(index);
        rest = &rest[at + label.len()..];
    }
    bound.push_str(rest);
    (bound, indices)
}
