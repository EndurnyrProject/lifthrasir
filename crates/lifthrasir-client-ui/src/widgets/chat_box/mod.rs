//! The chat panel, after the `Chat` artboard of the Lifthrasir Widgets design: a tab
//! strip (All, Party, Guild, Trade and one tab per whisper), a scrolling log and an
//! input row with a channel menu and a command palette.
//!
//! Every line of the log is an entity under [`ChatHistory`]. Speech arrives as
//! [`ChatHeard`] and lines nobody said as [`ChatNotice`]; other widgets may also
//! append their own with [`append_colored_line`]. A tab only hides the lines of the
//! other channels, so switching tabs never rebuilds the log. What the player picked
//! lives in [`ChatUi`]; none of it is sent to the server.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::{ComputedTextBlock, TextLayoutInfo};
use bevy::ui_widgets::Activate;
use chrono::Timelike;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::inventory::systems::to_item;
use lifthrasir_client::domain::inventory::{Inventory, Item};
use lifthrasir_ui::rich_text::{parse_color_codes, spawn_colored_text};
use lifthrasir_ui::theme::{self, ro_dark};
use net_contract::dto::CHAT_MAX_LINKS;
use net_contract::events::{ChatHeard, ChatNotice};

mod input;
mod scene;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use input::chat_input_control;
pub use view::Scope;

use crate::screens::shared::light_search_focus;
use crate::widgets::info_modal::view::inventory_item_view;
use crate::widgets::info_modal::{InfoContent, ShowInfoModal};
use crate::widgets::inventory_window::{InventoryCell, item_name};
use input::ChatDesk;
use view::{PopupView, RunAction, Speech, Unread};

/// Oldest lines past this are dropped so the log and its layout stay bounded.
const MAX_CHAT_LINES: usize = 100;
const CHAT_MAX_CHARS: usize = 255;
const CHAT_FONT_SIZE: f32 = 13.0;
/// A scroll offset past the end of any log; the layout clamps it to the last line.
const LOG_END: f32 = 1.0e6;
/// How close to its end, in pixels, the log still counts as following new lines.
const FOLLOW_SLACK: f32 = 8.0;

/// The panel root.
#[derive(Component, Default, Clone)]
pub struct ChatBoxRoot;

/// The scroll container that parents the log lines. Sibling widgets (feedback
/// lines, the announcement echo) find it with `With<ChatHistory>`.
#[derive(Component, Default, Clone)]
pub(crate) struct ChatHistory;

/// One line of the log, counted and dropped by the line cap.
#[derive(Component)]
struct ChatLine;

/// The chat input field.
#[derive(Component, Default, Clone)]
pub(crate) struct ChatInput;

/// The hint over the empty input.
#[derive(Component, Default, Clone)]
struct ChatHint;

/// The log with its overlays; it takes no room while the panel is collapsed.
#[derive(Component, Default, Clone)]
struct ChatLogFrame;

/// The note shown while the tab has no line.
#[derive(Component, Default, Clone)]
struct ChatEmpty;

/// A region [`rebuild_chat_ui`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum ChatHost {
    /// The tab strip and the toggles beside it.
    #[default]
    Head,
    /// The button naming where the input sends.
    Channel,
    Send,
    /// The channel menu or the command palette.
    Popup,
    /// The new-messages pill.
    Pill,
}

/// What a chat button does when activated.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub enum ChatAction {
    #[default]
    ToggleTime,
    ToggleCollapse,
    /// Shows a tab; `None` is All.
    ShowTab(Option<Scope>),
    CloseWhisper(String),
    ToggleMenu,
    /// Aims the input at a channel or a whisper.
    Aim(Scope),
    /// Completes the palette's n-th command.
    Complete(usize),
    Send,
    /// Scrolls the log back to its end.
    Jump,
}

/// What a line shows before its text, and which tabs show it.
#[derive(Component, Debug)]
struct LineMeta {
    /// `None` for a line nobody said: it shows on All only.
    scope: Option<Scope>,
    /// Shows on every tab: a refusal, which the player must not miss on the tab
    /// they typed in.
    everywhere: bool,
    /// When the line arrived, shown while timestamps are on.
    time: String,
    /// The text the line's root holds besides the time.
    head: String,
    /// The channel tag span and its text, blanked on its own channel's tab.
    tag: Option<(Entity, String)>,
}

/// What a click on a span of a line does.
#[derive(Component, Debug, Clone)]
enum SpanAction {
    /// Aims the input at a whisper to this hero.
    Whisper(String),
    /// Opens the linked item.
    Link(Box<Item>),
}

/// What the player picked in the chat panel.
#[derive(Resource, Default, Debug)]
pub struct ChatUi {
    /// The tab on show; `None` is All.
    tab: Option<Scope>,
    /// Where the input sends.
    target: Scope,
    /// Heroes with a whisper tab, oldest first.
    whispers: Vec<String>,
    unread: HashMap<Scope, Unread>,
    show_time: bool,
    collapsed: bool,
    menu_open: bool,
    /// The highlighted row of the command palette.
    palette_row: usize,
    /// Lines that arrived on the shown tab while it was scrolled up.
    below: u32,
    /// Scroll the log to its end on the next frame.
    jump: bool,
    /// Items linked in the draft: the label typed into the input and the
    /// inventory index it stands for.
    links: Vec<(String, u32)>,
}

impl ChatUi {
    /// Whether the tab on show lists `line`.
    fn shows(&self, line: &LineMeta) -> bool {
        line.everywhere || self.tab.is_none() || self.tab == line.scope
    }

    /// Books a heard line: a whisper opens its tab, the player's own line brings
    /// the log back to its end, and anyone else's counts as unread on the tabs
    /// that are not on show. Local has no tab, so it never counts.
    fn hear(&mut self, speech: &Speech) {
        if let Scope::Whisper(peer) = &speech.scope
            && !self.whispers.contains(peer)
        {
            self.whispers.push(peer.clone());
        }
        if speech.mine {
            self.jump = true;
            return;
        }
        if speech.scope == Scope::Local || self.tab.as_ref() == Some(&speech.scope) {
            return;
        }
        let unread = self.unread.entry(speech.scope.clone()).or_default();
        unread.count += 1;
        unread.ping |= speech.mention;
    }

    /// Shows a tab. A channel or whisper tab also aims the input at it.
    fn show_tab(&mut self, tab: Option<Scope>) {
        if let Some(scope) = &tab {
            self.unread.remove(scope);
            self.target = scope.clone();
        }
        self.tab = tab;
        self.below = 0;
        self.jump = true;
    }

    fn close_whisper(&mut self, name: &str) {
        let scope = Scope::Whisper(name.to_string());
        self.whispers.retain(|whisper| whisper != name);
        self.unread.remove(&scope);
        if self.tab.as_ref() == Some(&scope) {
            self.tab = None;
            self.jump = true;
        }
        if self.target == scope {
            self.target = Scope::Local;
        }
    }
}

pub struct ChatBoxPlugin;

impl Plugin for ChatBoxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatUi>();
        app.add_systems(OnEnter(GameState::InGame), reset_chat);
        app.add_systems(
            Update,
            (
                ingest_chat,
                chat_input_control,
                rebuild_chat_ui,
                sync_lines,
                follow_log,
                sync_hint,
                light_search_focus::<ChatInput>,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            crate::widgets::mount::ingest_mount_feedback.run_if(in_state(GameState::InGame)),
        );
    }
}

/// Builds the chat panel under `parent`.
pub fn spawn_chat_box(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(scene::panel()).insert(ChildOf(parent));
}

/// A new session starts with an empty log, so it starts with fresh picks too.
fn reset_chat(mut ui: ResMut<ChatUi>) {
    *ui = ChatUi::default();
}

/// The local wall-clock time, as the log's timestamps show it.
fn clock() -> String {
    let now = chrono::Local::now();
    format!("{:02}:{:02}", now.hour(), now.minute())
}

fn line_font(font: &Handle<Font>) -> TextFont {
    TextFont {
        font: font.clone().into(),
        font_size: CHAT_FONT_SIZE.into(),
        ..default()
    }
}

/// Appends `text` as a line nobody said, then drops the oldest lines past
/// [`MAX_CHAT_LINES`]. Inline `^RRGGBB` codes in `text` become colored runs over
/// `default_color`. The line shows on the All tab only. Shared with the widgets
/// that report their own outcomes in the log, hence `pub(crate)`.
pub(crate) fn append_colored_line(
    commands: &mut Commands,
    container: Entity,
    text: &str,
    default_color: Color,
    font: Handle<Font>,
) {
    append_notice(commands, container, (text, default_color), font, false);
}

/// [`append_colored_line`] for `(text, default color)`, shown on every tab when
/// `everywhere`.
fn append_notice(
    commands: &mut Commands,
    container: Entity,
    (text, default_color): (&str, Color),
    font: Handle<Font>,
    everywhere: bool,
) {
    let line = spawn_colored_text(
        commands,
        container,
        text,
        font,
        CHAT_FONT_SIZE,
        default_color,
    );
    let head = parse_color_codes(text, default_color)
        .into_iter()
        .next()
        .map(|(_, run)| run)
        .unwrap_or_default();
    commands.entity(line).insert((
        ChatLine,
        LineMeta {
            scope: None,
            everywhere,
            time: clock(),
            head,
            tag: None,
        },
    ));
    cap_lines(commands, container);
}

/// Appends a heard line: the root holds the timestamp and every piece of the line
/// is a span after it, so [`on_line_click`] can tell which piece was clicked.
fn append_speech(
    commands: &mut Commands,
    container: Entity,
    speech: Speech,
    links: &[Item],
    font: &Handle<Font>,
) {
    let fill = if speech.mention {
        ro_dark::FROST_GLOW.with_alpha(0.09)
    } else {
        Color::NONE
    };
    let line = commands
        .spawn((
            Text::default(),
            line_font(font),
            TextColor(ro_dark::MUTED),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK.with_alpha(0.6),
            },
            Node {
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(fill),
            ChatLine,
            ChildOf(container),
        ))
        .id();
    let mut tag = None;
    for run in speech.runs {
        let mut span = commands.spawn((
            TextSpan::new(run.text.clone()),
            line_font(font),
            TextColor(run.color),
            ChildOf(line),
        ));
        match run.action {
            Some(RunAction::Whisper(name)) => {
                span.insert(SpanAction::Whisper(name));
            }
            Some(RunAction::Link(link)) => {
                if let Some(item) = links.get(link) {
                    span.insert(SpanAction::Link(Box::new(item.clone())));
                }
            }
            None => {}
        }
        if run.tag {
            tag = Some((span.id(), run.text));
        }
    }
    commands.entity(line).insert(LineMeta {
        scope: Some(speech.scope),
        everywhere: false,
        time: clock(),
        head: String::new(),
        tag,
    });
    cap_lines(commands, container);
}

/// Despawns the oldest lines of `container` past [`MAX_CHAT_LINES`].
fn cap_lines(commands: &mut Commands, container: Entity) {
    commands.queue(move |world: &mut World| {
        let Some(children) = world.get::<Children>(container) else {
            return;
        };
        let lines: Vec<Entity> = children
            .iter()
            .filter(|entity| world.get::<ChatLine>(*entity).is_some())
            .collect();
        let extra = lines.len().saturating_sub(MAX_CHAT_LINES);
        for entity in lines.into_iter().take(extra) {
            world.entity_mut(entity).despawn();
        }
    });
}

/// A linked item is drawn rare when it is refined or carries a card.
fn is_rare(item: &Item) -> bool {
    item.refine > 0 || item.cards.iter().any(|card| *card != 0)
}

/// What a heard line is drawn from besides its own text.
#[derive(SystemParam)]
struct ChatSources<'w, 's> {
    container: Query<'w, 's, Entity, With<ChatHistory>>,
    player: Query<'w, 's, &'static EntityName, With<LocalPlayer>>,
    item_db: Option<Res<'w, ItemDb>>,
    asset_server: Res<'w, AssetServer>,
}

/// Turns heard speech and notices into log lines.
fn ingest_chat(
    mut heard: MessageReader<ChatHeard>,
    mut notices: MessageReader<ChatNotice>,
    sources: ChatSources,
    mut ui: ResMut<ChatUi>,
    mut commands: Commands,
) {
    if heard.is_empty() && notices.is_empty() {
        return;
    }
    let Ok(container) = sources.container.single() else {
        return;
    };
    let font = sources.asset_server.load(theme::FONT_BODY);
    let me = sources
        .player
        .single()
        .map_or("", |name| name.name.as_str());
    let item_db = sources.item_db.as_deref();
    for event in heard.read() {
        let items: Vec<Item> = event.links.iter().map(to_item).collect();
        let labels: Vec<(String, bool)> = items
            .iter()
            .map(|item| (item_name(item, item_db), is_rare(item)))
            .collect();
        let speech = view::speech(event, me, &labels);
        ui.hear(&speech);
        append_speech(&mut commands, container, speech, &items, &font);
    }
    for notice in notices.read() {
        let color = if notice.error {
            view::ERROR
        } else {
            ro_dark::MUTED
        };
        append_notice(
            &mut commands,
            container,
            (&notice.message, color),
            font.clone(),
            notice.error,
        );
    }
}

/// What a host shows. Compared against what it showed last, so a host is only
/// respawned when its view changed.
#[derive(Debug, Clone, PartialEq)]
enum Content {
    Head(view::HeadView),
    Channel(Scope),
    Send(bool),
    Popup(PopupView),
    Pill(u32),
}

impl Content {
    fn spawn(self, commands: &mut Commands) -> Entity {
        match self {
            Content::Head(view) => commands.spawn_scene(scene::head(view)).id(),
            Content::Channel(target) => commands.spawn_scene(scene::channel_button(target)).id(),
            Content::Send(enabled) => commands.spawn_scene(scene::send_button(enabled)).id(),
            Content::Popup(view) => commands.spawn_scene(scene::popup(view)).id(),
            Content::Pill(below) => commands.spawn_scene(scene::pill(below)).id(),
        }
    }
}

/// Refills every [`ChatHost`] whose view changed.
fn rebuild_chat_ui(
    mut commands: Commands,
    ui: Res<ChatUi>,
    field: Query<&bevy::text::EditableText, With<ChatInput>>,
    hosts: Query<(Entity, Ref<ChatHost>, Option<&Children>)>,
    mut shown: Local<[Option<Content>; 5]>,
) {
    let draft = field
        .single()
        .map(|field| field.value().to_string())
        .unwrap_or_default();
    for (host, kind, children) in &hosts {
        let content = match *kind {
            ChatHost::Head => Some(Content::Head(view::head(&ui))),
            ChatHost::Channel => Some(Content::Channel(ui.target.clone())),
            ChatHost::Send => Some(Content::Send(!draft.trim().is_empty())),
            ChatHost::Popup => view::popup(&ui, &draft).map(Content::Popup),
            ChatHost::Pill => (ui.below > 0 && !ui.collapsed).then_some(Content::Pill(ui.below)),
        };
        let slot = &mut shown[*kind as usize];
        if *slot == content && !kind.is_added() {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        if let Some(content) = content.clone() {
            let spawned = content.spawn(&mut commands);
            commands.entity(spawned).insert(ChildOf(host));
        }
        *slot = content;
    }
}

fn set_display(node: &mut Mut<Node>, shown: bool) {
    let display = if shown { Display::Flex } else { Display::None };
    if node.display != display {
        node.display = display;
    }
}

/// The log frame's node, kept apart from the lines and the empty note.
type OnlyLogFrame = (With<ChatLogFrame>, Without<ChatLine>, Without<ChatEmpty>);

/// Applies the tab, the timestamp toggle and the collapse toggle to the log:
/// which lines take room, what stands in front of each, and whether the log
/// itself is on show.
fn sync_lines(
    ui: Res<ChatUi>,
    added: Query<(), Added<LineMeta>>,
    mut lines: Query<(&LineMeta, &mut Node, &mut Text), With<ChatLine>>,
    mut spans: Query<&mut TextSpan>,
    mut notes: Query<&mut Node, (With<ChatEmpty>, Without<ChatLine>)>,
    mut frames: Query<&mut Node, OnlyLogFrame>,
) {
    if !ui.is_changed() && added.is_empty() {
        return;
    }
    let mut any = false;
    for (meta, mut node, mut text) in &mut lines {
        let shown = ui.shows(meta);
        any |= shown;
        set_display(&mut node, shown);
        let head = if ui.show_time {
            format!("{} {}", meta.time, meta.head)
        } else {
            meta.head.clone()
        };
        if text.0 != head {
            text.0 = head;
        }
        let Some((span, tag)) = &meta.tag else {
            continue;
        };
        let Ok(mut span) = spans.get_mut(*span) else {
            continue;
        };
        let tag = if ui.tab == meta.scope { "" } else { tag };
        if span.0 != tag {
            span.0 = tag.to_string();
        }
    }
    for mut note in &mut notes {
        set_display(&mut note, !any);
    }
    for mut frame in &mut frames {
        set_display(&mut frame, !ui.collapsed);
    }
}

/// Keeps the log on its newest line while the player is reading the end of it.
/// Lines that arrive while it is scrolled up are counted for the pill instead.
fn follow_log(
    mut ui: ResMut<ChatUi>,
    mut log: Query<(&ComputedNode, &mut ScrollPosition), With<ChatHistory>>,
    added: Query<&LineMeta, Added<LineMeta>>,
) {
    let Ok((node, mut scroll)) = log.single_mut() else {
        return;
    };
    let end = (node.content_size().y - node.size().y).max(0.0);
    let at_end = node.scroll_position.y + FOLLOW_SLACK / node.inverse_scale_factor >= end;
    let fresh = added.iter().filter(|meta| ui.shows(meta)).count() as u32;
    if ui.jump || (fresh > 0 && at_end) {
        scroll.y = LOG_END;
        if ui.jump {
            ui.jump = false;
        }
        if ui.below != 0 {
            ui.below = 0;
        }
        return;
    }
    if fresh > 0 {
        ui.below += fresh;
        return;
    }
    if at_end && ui.below != 0 {
        ui.below = 0;
    }
}

/// Rewrites the hint in the empty input when the input aims somewhere else.
fn sync_hint(ui: Res<ChatUi>, mut hints: Query<&mut Text, With<ChatHint>>) {
    let hint = view::hint(&ui.target);
    for mut text in &mut hints {
        if text.0 != hint {
            text.0.clone_from(&hint);
        }
    }
}

/// Runs the [`ChatAction`] of an activated button.
fn on_chat_button(activate: On<Activate>, actions: Query<&ChatAction>, mut desk: ChatDesk) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    match action.clone() {
        ChatAction::ToggleTime => desk.ui.show_time = !desk.ui.show_time,
        ChatAction::ToggleCollapse => {
            desk.ui.collapsed = !desk.ui.collapsed;
            desk.ui.jump = true;
        }
        ChatAction::ShowTab(tab) => desk.ui.show_tab(tab),
        ChatAction::CloseWhisper(name) => desk.ui.close_whisper(&name),
        ChatAction::ToggleMenu => desk.ui.menu_open = !desk.ui.menu_open,
        ChatAction::Aim(scope) => {
            desk.ui.target = scope;
            desk.ui.menu_open = false;
            desk.refocus();
        }
        ChatAction::Complete(row) => desk.complete_row(row),
        ChatAction::Send => {
            desk.submit();
        }
        ChatAction::Jump => desk.ui.jump = true,
    }
}

/// The span of a line under a click. Runs of the text layout are in the node's
/// physical pixels, measured from its top left corner.
fn clicked_span(
    click: &PointerClick,
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    layout: &TextLayoutInfo,
    block: &ComputedTextBlock,
) -> Option<Entity> {
    let pointer = click.pointer.position / node.inverse_scale_factor;
    let spot = (node.normalize_point(*transform, pointer)? + 0.5) * node.size();
    let run = layout
        .run_geometry
        .iter()
        .find(|run| run.bounds.contains(spot))?;
    Some(block.entities().get(run.section_index as usize)?.entity)
}

/// A click on a speaker's name aims a whisper at them; a click on an item link
/// opens the item. Bound to the log, so it sees the click on any of its lines.
fn on_line_click(
    click: On<PointerClick>,
    lines: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &TextLayoutInfo,
            &ComputedTextBlock,
        ),
        With<ChatLine>,
    >,
    actions: Query<&SpanAction>,
    item_db: Option<Res<ItemDb>>,
    option_db: Option<Res<RandomOptionDb>>,
    mut info: MessageWriter<ShowInfoModal>,
    mut desk: ChatDesk,
) {
    let Ok((node, transform, layout, block)) = lines.get(click.original_event_target()) else {
        return;
    };
    let action = clicked_span(&click, node, transform, layout, block)
        .and_then(|span| actions.get(span).ok());
    match action {
        Some(SpanAction::Whisper(name)) => {
            desk.ui.target = Scope::Whisper(name.clone());
            desk.ui.menu_open = false;
            desk.refocus();
        }
        Some(SpanAction::Link(item)) => {
            let Some(item_db) = item_db.as_deref() else {
                warn!("chat: ItemDb not loaded yet, ignoring item link");
                return;
            };
            info.write(ShowInfoModal {
                content: InfoContent::Item {
                    view: inventory_item_view(item, item_db, option_db.as_deref()),
                    action: None,
                },
            });
        }
        None => {}
    }
}

/// A shift-click on an inventory slot links its item in the chat draft. Bound to
/// the inventory's slots.
pub(crate) fn link_item_on_shift_click(
    click: On<PointerClick>,
    keys: Res<ButtonInput<KeyCode>>,
    cells: Query<&InventoryCell>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    mut desk: ChatDesk,
) {
    if click.button != PointerButton::Primary
        || !keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
    {
        return;
    }
    let Some(item) = cells
        .get(click.entity)
        .ok()
        .and_then(|cell| inventory.get(cell.index))
    else {
        return;
    };
    if desk.ui.links.len() >= CHAT_MAX_LINKS {
        return;
    }
    let label = format!("[{}]", item_name(item, item_db.as_deref()));
    desk.link(label, item.index.into());
}
