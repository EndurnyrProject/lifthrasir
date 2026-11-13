//! NPC dialogue window, after the `NPC Dialog` artboard: the NPC's line typed onto a
//! parchment page, the reply the frame expects under it (menu options or a field), a
//! footer of keyed buttons and a log of what was said so far.
//!
//! [`ActiveNpcDialog`] is the conversation. Every [`NpcDialogReceived`] folds into it
//! and respawns the window from it; answers go out as [`RespondToNpc`] and the server
//! drives the next frame. All five `expect` frames are wired: `NEXT`, `CLOSE`, `MENU`
//! and the `INPUT_INT`/`INPUT_STR` fields.

use bevy::ecs::system::SystemParam;
use bevy::input_focus::{AutoFocus, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_ui::rich_text::parse_color_codes;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::primitives::HoverColors;
use net_contract::commands::RespondToNpc;
use net_contract::dto::{NpcDialogExpect, NpcResponse};
use net_contract::events::NpcDialogReceived;

use crate::screens::shared::shown_if;
use crate::worldspace::progress_bar::ActiveProgressBar;

pub mod scene;

/// Window-root marker: the full-width wrapper the log and the window are centred in.
/// One exists for the lifetime of a conversation.
#[derive(Component, Default, Clone)]
pub struct NpcDialogRoot;

/// The log panel above the window, laid out only while the log is open.
#[derive(Component, Default, Clone)]
pub struct NpcDialogLog;

/// The field of an `INPUT_INT`/`INPUT_STR` frame. It takes the keyboard as it appears.
#[derive(Component, Default, Clone)]
#[require(AutoFocus, TextCursorStyle)]
pub struct NpcInputField;

/// A `MENU` option row, by its position in the menu.
#[derive(Component, Default, Clone)]
pub struct MenuOption(pub usize);

/// Text that reads `typing` while the line is typed out and `done` once it is whole.
#[derive(Component, Default, Clone)]
pub struct TypingLabel {
    pub typing: &'static str,
    pub done: &'static str,
}

/// A node shown only while the line is typed out (`true`) or only after (`false`).
#[derive(Component, Default, Clone)]
pub struct ShownWhileTyping(pub bool);

/// Progressively reveals a dialogue line. `full` keeps the original `^RRGGBB`-coded
/// text so [`scene::slice_colored_runs`] can recompute colored runs for any prefix;
/// `total` is its count of visible characters and `revealed` how many of them are
/// shown so far (Unicode scalars, not bytes); `elapsed` accumulates delta time so the
/// reveal rate is frame-independent.
#[derive(Component, Default, Clone)]
pub struct Typewriter {
    pub revealed: usize,
    pub full: String,
    pub total: usize,
    pub elapsed: f32,
}

impl Typewriter {
    pub fn done(&self) -> bool {
        self.revealed >= self.total
    }
}

/// What a button or a menu option does when activated.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FooterButtonAction {
    /// Shows the rest of the line, then asks for the next page.
    #[default]
    Continue,
    CloseOrCancel,
    /// A MENU option; carries the already-1-based choice index.
    Choice(u32),
    /// Submits the `INPUT_INT`/`INPUT_STR` field's current value.
    Confirm,
}

/// Who said a line of the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    Npc,
    You,
}

/// The conversation with one NPC, present only while it is live: the frame on screen
/// and everything said before it.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ActiveNpcDialog {
    /// Echoed in every [`RespondToNpc`].
    pub npc_id: u32,
    pub expect: NpcDialogExpect,
    /// The NPC's display name, the window title.
    pub name: String,
    /// The line on screen, `^RRGGBB` codes included. Empty once it moved to the log.
    pub text: String,
    /// The options of a `MENU` frame, in server order.
    pub options: Vec<String>,
    pub log: Vec<(Speaker, String)>,
    pub show_log: bool,
    /// The menu option the arrow keys or the pointer last pointed at.
    pub highlight: usize,
    /// The frame on screen was answered; the server has yet to send the next one.
    pub answered: bool,
}

impl ActiveNpcDialog {
    /// Moves the line on screen into the log, followed by what the player `said`.
    fn record(&mut self, said: Option<String>) {
        let line = plain_text(&std::mem::take(&mut self.text));
        if !line.is_empty() {
            self.log.push((Speaker::Npc, line));
        }
        self.log.extend(said.map(|said| (Speaker::You, said)));
    }
}

const FALLBACK_TITLE: &str = "Conversation";

/// Typewriter reveal speed, in visible characters per second.
const CHARS_PER_SECOND: f32 = 40.0;

/// The keys that pick a menu option, in menu order.
const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

pub struct NpcDialogPlugin;

impl Plugin for NpcDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(clear_input_focus);
        app.add_systems(
            Update,
            on_dialog_received.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            (
                cancel_on_escape,
                // Enter is the dialogue's while it is open: it is consumed here so the
                // chat does not open on the same press.
                dialog_keys.before(crate::widgets::chat_box::chat_input_control),
            )
                .run_if(
                    in_state(GameState::InGame)
                        .and_then(resource_exists::<ActiveNpcDialog>)
                        // While a progressbar is live it owns cancel (ESC + move), so
                        // the dialogue's keys yield to avoid a double answer.
                        .and_then(not(resource_exists::<ActiveProgressBar>)),
                ),
        );
        app.add_systems(
            Update,
            (
                (typewriter_reveal, sync_typing_cues).chain(),
                paint_options,
                light_input_focus,
            )
                .run_if(in_state(GameState::InGame).and_then(resource_exists::<ActiveNpcDialog>)),
        );
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveNpcDialog>()
        });
    }
}

/// Folds the latest [`NpcDialogReceived`] into the conversation and respawns the
/// window from it.
fn on_dialog_received(
    mut events: MessageReader<NpcDialogReceived>,
    mut commands: Commands,
    active: Option<Res<ActiveNpcDialog>>,
    roots: Query<Entity, With<NpcDialogRoot>>,
    registry: Res<EntityRegistry>,
    names: Query<&EntityName>,
) {
    let Some(event) = events.read().last() else {
        return;
    };

    let name = registry
        .get_entity(event.npc_id)
        .and_then(|entity| names.get(entity).ok())
        .map(|entity_name| entity_name.name.clone());
    let dialog = next_frame(active.as_deref(), event, title_or_fallback(name));

    for root in &roots {
        commands.entity(root).despawn();
    }
    commands
        .spawn_scene(scene::window(&dialog))
        .insert(DespawnOnExit(GameState::InGame));
    commands.insert_resource(dialog);
}

/// The conversation after `event`. A frame from the NPC already talking keeps the log,
/// which gains the line it replaces, and the log's visibility; any other frame starts
/// a new conversation.
fn next_frame(
    previous: Option<&ActiveNpcDialog>,
    event: &NpcDialogReceived,
    name: String,
) -> ActiveNpcDialog {
    let mut carried = previous
        .filter(|previous| previous.npc_id == event.npc_id)
        .cloned();
    if let Some(carried) = &mut carried {
        carried.record(None);
    }
    let (log, show_log) = carried
        .map(|carried| (carried.log, carried.show_log))
        .unwrap_or_default();
    ActiveNpcDialog {
        npc_id: event.npc_id,
        expect: event.expect,
        name,
        text: event.text.clone(),
        options: event.options.clone(),
        log,
        show_log,
        highlight: 0,
        answered: false,
    }
}

/// The resolved NPC display name, falling back to `"Conversation"` when the entity
/// hasn't been named yet (e.g. a click without a prior hover).
fn title_or_fallback(name: Option<String>) -> String {
    name.unwrap_or_else(|| FALLBACK_TITLE.to_string())
}

/// `text` without its `^RRGGBB` codes and on one line: what the log keeps of it.
fn plain_text(text: &str) -> String {
    let plain: String = parse_color_codes(text, Color::NONE)
        .into_iter()
        .map(|(_, run)| run)
        .collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

type TypewriterParts = (
    Entity,
    &'static mut Typewriter,
    &'static mut Text,
    &'static mut TextColor,
    Option<&'static Children>,
);

/// Advances every open dialogue's [`Typewriter`] by [`CHARS_PER_SECOND`] worth of
/// delta time and re-renders the revealed prefix. A no-op once the line is whole, so
/// a fully-revealed (or short/instant) line stops touching its spans every frame.
fn typewriter_reveal(time: Res<Time>, mut commands: Commands, mut texts: Query<TypewriterParts>) {
    for (entity, mut typewriter, mut text, mut color, children) in &mut texts {
        if typewriter.done() {
            continue;
        }
        typewriter.elapsed += time.delta_secs();
        let revealed = ((typewriter.elapsed * CHARS_PER_SECOND) as usize).min(typewriter.total);
        if revealed == typewriter.revealed {
            continue;
        }
        typewriter.revealed = revealed;
        let (runs, _) = parsed_total(&typewriter.full);
        render_revealed(
            &mut commands,
            entity,
            &mut text,
            &mut color,
            children,
            scene::slice_colored_runs(&runs, revealed),
        );
    }
}

/// Follows the typewriter: button labels and the nodes that only show while the line
/// is typed out, or only once it is whole.
fn sync_typing_cues(
    typewriters: Query<&Typewriter, Changed<Typewriter>>,
    mut labels: Query<(&TypingLabel, &mut Text)>,
    mut marks: Query<(&ShownWhileTyping, &mut Visibility)>,
) {
    let Some(typewriter) = typewriters.iter().next() else {
        return;
    };
    let typing = !typewriter.done();
    for (label, mut text) in &mut labels {
        let wanted = if typing { label.typing } else { label.done };
        if text.0 != wanted {
            text.0 = wanted.to_string();
        }
    }
    for (mark, mut visibility) in &mut marks {
        let wanted = shown_if(mark.0 == typing);
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// Clicking the page shows the rest of the line at once.
fn on_page_click(mut click: On<PointerClick>, mut answering: Answering) {
    click.propagate(false);
    answering.skip_typing();
}

/// Parses `full`'s `^RRGGBB` runs and the total visible (decoded) character count,
/// so the reveal cap stays consistent between the tick and the skip paths.
fn parsed_total(full: &str) -> (Vec<(Color, String)>, usize) {
    let runs = parse_color_codes(full, ro_dark::INK);
    let total = runs.iter().map(|(_, run)| run.chars().count()).sum();
    (runs, total)
}

/// Rewrites the dialogue text entity's root `Text`/`TextColor` from `visible`'s
/// first run and replaces its `TextSpan` children with the rest, so multi-color
/// dialogue keeps its per-run colors at every reveal length.
fn render_revealed(
    commands: &mut Commands,
    text_entity: Entity,
    text: &mut Text,
    color: &mut TextColor,
    children: Option<&Children>,
    visible: Vec<(Color, String)>,
) {
    if let Some(children) = children {
        for &child in children {
            commands.entity(child).despawn();
        }
    }
    let mut runs = visible.into_iter();
    let (first_color, first_text) = runs.next().unwrap_or((ro_dark::INK, String::new()));
    text.0 = first_text;
    color.0 = first_color;
    for (run_color, content) in runs {
        commands
            .spawn_scene(scene::text_span(content, run_color))
            .insert(ChildOf(text_entity));
    }
}

/// Everything a button or a key needs to answer the frame on screen.
#[derive(SystemParam)]
struct Answering<'w, 's> {
    active: ResMut<'w, ActiveNpcDialog>,
    roots: Query<'w, 's, Entity, With<NpcDialogRoot>>,
    fields: Query<'w, 's, &'static EditableText, With<NpcInputField>>,
    typewriters: Query<'w, 's, TypewriterParts>,
    commands: Commands<'w, 's>,
    respond: MessageWriter<'w, RespondToNpc>,
}

impl Answering<'_, '_> {
    /// `Continue`/`Choice` answer and leave the window open (the server drives the
    /// next frame); `Confirm` reads the input field and answers if it parses for the
    /// frame; `CloseOrCancel` ends the conversation.
    fn act(&mut self, action: FooterButtonAction) {
        match action {
            FooterButtonAction::Continue => self.advance(),
            FooterButtonAction::Choice(choice) => {
                let label = (choice as usize)
                    .checked_sub(1)
                    .and_then(|index| self.active.options.get(index))
                    .cloned();
                self.answer(NpcResponse::Choice(choice), label);
            }
            FooterButtonAction::Confirm => {
                let Ok(field) = self.fields.single() else {
                    return;
                };
                let text = field.value().to_string();
                if let Some(response) = confirm_response(self.active.expect, &text) {
                    self.answer(response, Some(text));
                }
            }
            FooterButtonAction::CloseOrCancel => self.close(),
        }
    }

    /// Space or Enter on a page: shows the rest of the line, then turns the page or,
    /// on the last one, closes the window.
    fn advance(&mut self) {
        if self.skip_typing() {
            return;
        }
        let expect = self.active.expect;
        match expect {
            NpcDialogExpect::Next => self.answer(NpcResponse::Continue, None),
            NpcDialogExpect::Close => self.close(),
            NpcDialogExpect::Menu | NpcDialogExpect::InputInt | NpcDialogExpect::InputStr => {}
        }
    }

    /// The keys of a `MENU` frame: arrows move the highlight, a digit or Enter answers.
    fn menu_keys(&mut self, keys: &ButtonInput<KeyCode>, enter: bool, space: bool) {
        if space {
            self.skip_typing();
        }
        let count = self.active.options.len();
        for (down, key) in [(true, KeyCode::ArrowDown), (false, KeyCode::ArrowUp)] {
            if keys.just_pressed(key) {
                self.active.highlight = step_highlight(self.active.highlight, down, count);
            }
        }
        let digit = DIGITS
            .iter()
            .take(count)
            .position(|key| keys.just_pressed(*key));
        let choice = digit.or(enter.then_some(self.active.highlight));
        if let Some(index) = choice.filter(|index| *index < count) {
            self.act(FooterButtonAction::Choice(index as u32 + 1));
        }
    }

    /// Sends `response` once per frame: a second press before the server's next frame
    /// would otherwise answer a page the player has not seen.
    fn answer(&mut self, response: NpcResponse, said: Option<String>) {
        if self.active.answered {
            return;
        }
        self.active.answered = true;
        self.active.record(said);
        self.respond.write(RespondToNpc {
            npc_id: self.active.npc_id,
            response,
        });
    }

    /// Shows the whole line at once. `false` when nothing was left to type.
    fn skip_typing(&mut self) -> bool {
        let mut skipped = false;
        for (entity, mut typewriter, mut text, mut color, children) in &mut self.typewriters {
            if typewriter.done() {
                continue;
            }
            typewriter.revealed = typewriter.total;
            let (runs, _) = parsed_total(&typewriter.full);
            render_revealed(
                &mut self.commands,
                entity,
                &mut text,
                &mut color,
                children,
                runs,
            );
            skipped = true;
        }
        skipped
    }

    /// Ends the conversation: `Cancel` unless the frame is already terminal (`CLOSE`),
    /// then the window and [`ActiveNpcDialog`] go.
    fn close(&mut self) {
        if self.active.expect != NpcDialogExpect::Close {
            self.respond.write(RespondToNpc {
                npc_id: self.active.npc_id,
                response: NpcResponse::Cancel,
            });
        }
        for root in &self.roots {
            self.commands.entity(root).despawn();
        }
        self.commands.remove_resource::<ActiveNpcDialog>();
    }
}

/// Shared handler of every button and menu option.
fn on_footer_button(
    activate: On<Activate>,
    actions: Query<&FooterButtonAction>,
    mut answering: Answering,
) {
    if let Ok(action) = actions.get(activate.entity) {
        answering.act(*action);
    }
}

/// Maps a `Confirm` submission to the response to send for the active frame's
/// `expect`: `INPUT_STR` always sends the raw text; `INPUT_INT` parses it as `i64`
/// and sends nothing (`None`) on empty/non-numeric input rather than a malformed
/// command.
fn confirm_response(expect: NpcDialogExpect, text: &str) -> Option<NpcResponse> {
    match expect {
        NpcDialogExpect::InputStr => Some(NpcResponse::Input(text.to_string())),
        NpcDialogExpect::InputInt => text.trim().parse::<i64>().ok().map(NpcResponse::Number),
        NpcDialogExpect::Next | NpcDialogExpect::Menu | NpcDialogExpect::Close => None,
    }
}

/// The option after or before `current` in a menu of `count`, wrapping at both ends.
fn step_highlight(current: usize, down: bool, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if down {
        (current + 1) % count
    } else {
        (current + count - 1) % count
    }
}

/// ESC ends the conversation exactly like the title bar's close button. Gated on
/// `ActiveNpcDialog` existing, so it only fires while a dialogue is open; the escape
/// menu is separately gated to skip while this resource is present, so the two never
/// both react to one press.
fn cancel_on_escape(keys: Res<ButtonInput<KeyCode>>, mut answering: Answering) {
    if keys.just_pressed(KeyCode::Escape) {
        answering.close();
    }
}

/// The footer's keys: Space or Enter turns the page, Enter submits the field, and a
/// menu takes the arrows, the digits and Enter. Typing in another field (the chat)
/// keeps every key.
fn dialog_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    fields: Query<Has<NpcInputField>, With<EditableText>>,
    mut answering: Answering,
) {
    let typing_elsewhere = focus
        .get()
        .and_then(|field| fields.get(field).ok())
        .is_some_and(|own| !own);
    if typing_elsewhere {
        return;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let space = keys.just_pressed(KeyCode::Space);
    if enter {
        keys.clear_just_pressed(KeyCode::Enter);
        keys.clear_just_pressed(KeyCode::NumpadEnter);
    }
    let expect = answering.active.expect;
    match expect {
        NpcDialogExpect::Next | NpcDialogExpect::Close if enter || space => answering.advance(),
        NpcDialogExpect::InputInt | NpcDialogExpect::InputStr if enter => {
            answering.act(FooterButtonAction::Confirm)
        }
        NpcDialogExpect::Menu => answering.menu_keys(&keys, enter, space),
        _ => {}
    }
}

/// The pointer lights the option it is over, like the arrow keys do.
fn on_option_over(
    over: On<PointerOver>,
    options: Query<&MenuOption>,
    mut active: ResMut<ActiveNpcDialog>,
) {
    if let Ok(option) = options.get(over.entity)
        && active.highlight != option.0
    {
        active.highlight = option.0;
    }
}

/// Lights the menu option the arrow keys or the pointer last pointed at.
fn paint_options(
    active: Res<ActiveNpcDialog>,
    mut options: Query<(&MenuOption, &mut BackgroundColor, &mut BorderColor)>,
) {
    if !active.is_changed() {
        return;
    }
    for (option, mut fill, mut stroke) in &mut options {
        let (background, border) = scene::option_colors(option.0 == active.highlight);
        fill.0 = background;
        *stroke = BorderColor::all(border);
    }
}

/// Opens or closes the log and lights its switch to match.
fn on_log_toggle(
    click: On<PointerClick>,
    mut active: ResMut<ActiveNpcDialog>,
    mut switches: Query<(&mut HoverColors, &mut BorderColor)>,
    mut panels: Query<&mut Node, With<NpcDialogLog>>,
) {
    active.show_log = !active.show_log;
    if let Ok((mut colors, mut stroke)) = switches.get_mut(click.entity) {
        *colors = scene::log_switch_colors(active.show_log);
        *stroke = BorderColor::all(colors.hover.1);
    }
    for mut panel in &mut panels {
        panel.display = scene::log_display(active.show_log);
    }
}

/// Rings the input field while it has the keyboard.
fn light_input_focus(
    focus: Res<InputFocus>,
    mut fields: Query<(Entity, &mut BorderColor), With<NpcInputField>>,
) {
    if !focus.is_changed() {
        return;
    }
    for (field, mut stroke) in &mut fields {
        *stroke = BorderColor::all(scene::field_stroke(focus.get() == Some(field)));
    }
}

/// A frame's field goes with its window; the keyboard must not stay on it.
fn clear_input_focus(removed: On<Remove<NpcInputField>>, mut focus: ResMut<InputFocus>) {
    if focus.get() == Some(removed.entity) {
        focus.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use bevy::state::app::StatesPlugin;

    fn frame(
        npc_id: u32,
        text: &str,
        expect: NpcDialogExpect,
        options: &[&str],
    ) -> NpcDialogReceived {
        NpcDialogReceived {
            npc_id,
            text: text.into(),
            expect,
            options: options.iter().map(|option| option.to_string()).collect(),
        }
    }

    #[test]
    fn title_or_fallback_uses_resolved_name() {
        assert_eq!(
            title_or_fallback(Some("Turban Thief".to_string())),
            "Turban Thief"
        );
    }

    #[test]
    fn title_or_fallback_defaults_when_unresolved() {
        assert_eq!(title_or_fallback(None), FALLBACK_TITLE);
    }

    #[test]
    fn confirm_response_input_str_sends_raw_text() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputStr, "hello"),
            Some(NpcResponse::Input("hello".to_string()))
        );
    }

    #[test]
    fn confirm_response_input_str_allows_empty_text() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputStr, ""),
            Some(NpcResponse::Input(String::new()))
        );
    }

    #[test]
    fn confirm_response_input_int_parses_digits() {
        assert_eq!(
            confirm_response(NpcDialogExpect::InputInt, "42"),
            Some(NpcResponse::Number(42))
        );
    }

    #[test]
    fn confirm_response_input_int_rejects_non_numeric() {
        assert_eq!(confirm_response(NpcDialogExpect::InputInt, "abc"), None);
    }

    #[test]
    fn confirm_response_input_int_rejects_empty() {
        assert_eq!(confirm_response(NpcDialogExpect::InputInt, ""), None);
    }

    #[test]
    fn step_highlight_wraps_at_both_ends() {
        assert_eq!(step_highlight(0, true, 3), 1);
        assert_eq!(step_highlight(2, true, 3), 0);
        assert_eq!(step_highlight(0, false, 3), 2);
        assert_eq!(step_highlight(0, true, 0), 0);
    }

    #[test]
    fn record_logs_the_line_without_codes_before_the_answer() {
        let mut dialog = next_frame(
            None,
            &frame(
                1,
                "Take the ^FF0000red\npotion^000000?",
                NpcDialogExpect::Menu,
                &["Yes"],
            ),
            "Mia".into(),
        );
        dialog.record(Some("Yes".into()));
        assert_eq!(
            dialog.log,
            vec![
                (Speaker::Npc, "Take the red potion?".to_string()),
                (Speaker::You, "Yes".to_string()),
            ]
        );

        // The line was moved, so the next frame does not log it a second time.
        let next = next_frame(
            Some(&dialog),
            &frame(1, "Here.", NpcDialogExpect::Close, &[]),
            "Mia".into(),
        );
        assert_eq!(next.log, dialog.log);
        assert!(!next.answered);
    }

    #[test]
    fn next_frame_keeps_the_log_only_for_the_same_npc() {
        let mut first = next_frame(
            None,
            &frame(1, "Hello.", NpcDialogExpect::Next, &[]),
            "Mia".into(),
        );
        first.show_log = true;

        let same = next_frame(
            Some(&first),
            &frame(1, "Bye.", NpcDialogExpect::Close, &[]),
            "Mia".into(),
        );
        assert_eq!(same.log, vec![(Speaker::Npc, "Hello.".to_string())]);
        assert!(same.show_log);

        let other = next_frame(
            Some(&first),
            &frame(2, "Yes?", NpcDialogExpect::Next, &[]),
            "Kafra".into(),
        );
        assert!(other.log.is_empty());
        assert!(!other.show_log);
    }

    /// An in-game app running the dialogue with a keyboard the test presses by hand.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            AssetPlugin::default(),
            ScenePlugin,
        ));
        app.init_state::<GameState>();
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<InputFocus>()
            .init_resource::<EntityRegistry>()
            .add_message::<NpcDialogReceived>()
            .add_message::<RespondToNpc>()
            .add_plugins(NpcDialogPlugin);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app
    }

    fn receive(app: &mut App, event: NpcDialogReceived) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<Messages<NpcDialogReceived>>()
            .write(event);
        app.update();
    }

    /// Presses `key` for one frame and returns the responses that frame sent.
    fn press(app: &mut App, key: KeyCode) -> Vec<NpcResponse> {
        app.world_mut()
            .resource_mut::<Messages<RespondToNpc>>()
            .clear();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
        keys.press(key);
        app.update();
        app.world()
            .resource::<Messages<RespondToNpc>>()
            .iter_current_update_messages()
            .map(|respond| respond.response.clone())
            .collect()
    }

    fn count<C: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query::<&C>().iter(world).count()
    }

    #[test]
    fn space_shows_the_line_then_turns_the_page_once() {
        let mut app = app();
        // Long enough that the typewriter cannot finish between two test frames.
        let line = "Back from the ruins already? ".repeat(8);
        receive(&mut app, frame(1, &line, NpcDialogExpect::Next, &[]));
        assert_eq!(count::<NpcDialogRoot>(&mut app), 1);

        assert_eq!(press(&mut app, KeyCode::Space), Vec::new());
        let world = app.world_mut();
        assert!(world.query::<&Typewriter>().single(world).unwrap().done());

        assert_eq!(press(&mut app, KeyCode::Space), vec![NpcResponse::Continue]);
        assert_eq!(press(&mut app, KeyCode::Space), Vec::new());
    }

    #[test]
    fn enter_is_consumed_so_the_chat_stays_closed() {
        let mut app = app();
        receive(&mut app, frame(1, "", NpcDialogExpect::Next, &[]));
        assert_eq!(press(&mut app, KeyCode::Enter), vec![NpcResponse::Continue]);
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Enter)
        );
    }

    #[test]
    fn menu_answers_by_digit_or_by_the_highlighted_option() {
        let mut app = app();
        let menu = || frame(1, "", NpcDialogExpect::Menu, &["Lore", "Donate", "Leave"]);
        receive(&mut app, menu());
        assert_eq!(count::<MenuOption>(&mut app), 3);
        assert_eq!(press(&mut app, KeyCode::Digit4), Vec::new());
        assert_eq!(
            press(&mut app, KeyCode::Digit2),
            vec![NpcResponse::Choice(2)]
        );

        receive(&mut app, menu());
        assert_eq!(press(&mut app, KeyCode::ArrowUp), Vec::new());
        assert_eq!(
            press(&mut app, KeyCode::Enter),
            vec![NpcResponse::Choice(3)]
        );
        assert_eq!(
            app.world().resource::<ActiveNpcDialog>().log,
            vec![
                (Speaker::You, "Donate".to_string()),
                (Speaker::You, "Leave".to_string())
            ]
        );
    }

    #[test]
    fn keys_stay_with_another_focused_field() {
        let mut app = app();
        receive(&mut app, frame(1, "", NpcDialogExpect::Next, &[]));
        let chat = app.world_mut().spawn(EditableText::new("")).id();
        app.insert_resource(InputFocus::from_entity(chat));
        assert_eq!(press(&mut app, KeyCode::Space), Vec::new());
    }

    #[test]
    fn input_frame_focuses_its_field_and_escape_cancels() {
        let mut app = app();
        receive(
            &mut app,
            frame(1, "How much?", NpcDialogExpect::InputInt, &[]),
        );
        let world = app.world_mut();
        let field = world
            .query_filtered::<Entity, With<NpcInputField>>()
            .single(world)
            .unwrap();
        assert!(world.get::<AutoFocus>(field).is_some());

        assert_eq!(press(&mut app, KeyCode::Escape), vec![NpcResponse::Cancel]);
        assert_eq!(count::<NpcDialogRoot>(&mut app), 0);
        assert!(!app.world().contains_resource::<ActiveNpcDialog>());
    }
}
