//! Chat rooms: the room window, the card a sign opens and the New Chat Room dialog,
//! after the `Chat Room Window` and `Chat Room Signs (World)` artboards of the
//! Lifthrasir Widgets design. The signs themselves are drawn by
//! [`crate::worldspace::chat_room_signs`].
//!
//! Authority stays in [`ChatRooms`] and the server: this module projects that state
//! and turns its controls into [`ChatRoomCommand`]s. Each [`ChatRoomHost`] is a region
//! [`rebuild_chat_room_ui`] refills when its view-model changes. The forms' hosts hold
//! a skeleton with the typed fields, and that skeleton holds further hosts, so a field
//! outlives the changes around it.

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::chat_room::ChatRooms;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};
use net_contract::commands::ChatRoomCommand;
use net_contract::dto::ChatRoomJoinError;
use net_contract::events::{ChatNotice, ChatRoomEvent};
use net_contract::state::ZoneSession;

pub(crate) mod scene;
pub(crate) mod view;

use crate::screens::shared::{light_field_focus, shown_if};
use crate::worldspace::WorldAnchor;
use view::{FormMode, RoomForm};

/// Marks the room window's root.
#[derive(Component, Default, Clone)]
pub struct ChatRoomWindow;

/// The window's draggable title bar.
#[derive(Component, Default, Clone)]
pub struct ChatRoomTitlebar;

/// A region [`rebuild_chat_room_ui`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRoomHost {
    /// The window's title, head count and settings button.
    #[default]
    Head,
    Log,
    /// The people in the room, or the owner's settings panel.
    Side,
    /// The New Chat Room dialog.
    Dialog,
    /// A form's seat stepper and who-can-join choice.
    FormOptions,
    /// A form's buttons and, for a new room, the preview of its sign.
    FormFoot,
    CardHead,
    CardFoot,
}

const HOSTS: usize = 8;

/// A text field of the chat room UI. Enter in one runs the action beside it.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomField {
    /// The line to say in the room.
    #[default]
    Message,
    Title,
    Password,
    /// The password asked by the card of a locked room.
    CardPassword,
}

/// The form's count of typed title characters.
#[derive(Component, Default, Clone)]
pub struct TitleCount;

/// The form's password field with its label, shown while the room asks for one.
#[derive(Component, Default, Clone)]
pub struct PasswordRow;

/// A row of the people list; its [`PersonActions`] show while the pointer is on it.
#[derive(Component, Default, Clone)]
pub struct PersonRow;

/// The owner's buttons on a [`PersonRow`].
#[derive(Component, Default, Clone)]
pub struct PersonActions;

/// The card open over a room's owner. A room that locks or changes hands gets a new
/// card, since both change what the card is built from.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomCard {
    room_id: u32,
    locked: bool,
    target: Entity,
}

/// What a chat room button does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRoomAction {
    #[default]
    Leave,
    /// Says the typed line in the room.
    Send,
    /// Opens or closes the owner's settings panel.
    ToggleSettings,
    MakeOwner(u32),
    Kick(u32),
    /// Adds this many seats to the form.
    Seats(i32),
    /// Sets whether the form's room asks for a password.
    Access(bool),
    /// Opens the new room or saves the edited one.
    SubmitForm,
    CancelForm,
    /// Joins the room of the open card.
    Join,
    CloseCard,
}

/// What the player picked in the chat room UI. None of it is sent to the server.
#[derive(Resource, Default, Debug)]
pub struct ChatRoomUi {
    /// The room whose card is open.
    card: Option<u32>,
    /// The room that last refused the player, and why.
    refusal: Option<(u32, ChatRoomJoinError)>,
    creating: bool,
    editing: bool,
    form: RoomForm,
    /// The password of the room this player opened or last saved. The server never
    /// sends one back, so it is empty for a room inherited from someone else.
    password: String,
}

impl ChatRoomUi {
    /// Opens the card of `room_id`, or closes it when it is the one on show.
    pub fn toggle_card(&mut self, room_id: u32) {
        self.card = (self.card != Some(room_id)).then_some(room_id);
        self.refusal = None;
    }

    fn refusal_of(&self, room_id: u32) -> Option<ChatRoomJoinError> {
        self.refusal
            .filter(|(refused, _)| *refused == room_id)
            .map(|(_, reason)| reason)
    }

    /// The form on show: the dialog while creating, the panel while the owner edits.
    fn form_mode(&self, rooms: &ChatRooms, me: u32) -> Option<FormMode> {
        if self.creating {
            return Some(FormMode::Create);
        }
        let room = rooms.joined().filter(|room| room.is_owner(me))?;
        self.editing.then_some(FormMode::Edit {
            seated: room.members.len() as u32,
        })
    }
}

pub struct ChatRoomWidgetPlugin;

impl Plugin for ChatRoomWidgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatRoomUi>();
        app.add_systems(
            Update,
            open_create_key.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                // Enter is the room's while one of its fields has the keyboard: it is
                // consumed here so the chat input does not take the focus.
                room_keys.before(crate::widgets::chat_box::chat_input_control),
                follow_rooms,
                sync_card,
                mirror_form,
                rebuild_chat_room_ui,
                show_window,
                show_password_row,
                reveal_person_actions,
                light_field_focus::<RoomField>,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), reset_ui);
    }
}

/// Spawns the room window and the dialog's host under the HUD root.
pub fn spawn_chat_room(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
    commands
        .spawn_scene(scene::dialog_host())
        .insert(ChildOf(parent));
}

fn reset_ui(mut ui: ResMut<ChatRoomUi>) {
    *ui = ChatRoomUi::default();
}

/// `PlayerAction::ChatRoom` (Alt+C) opens the New Chat Room dialog, or closes it. A
/// player in a room has no use for it: the server allows one room at a time.
fn open_create_key(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    rooms: Res<ChatRooms>,
    mut ui: ResMut<ChatRoomUi>,
) {
    let pressed = player
        .single()
        .is_ok_and(|actions| actions.just_pressed(&PlayerAction::ChatRoom));
    if !pressed || rooms.joined().is_some() {
        return;
    }
    ui.creating ^= true;
    if ui.creating {
        ui.form = RoomForm::default();
        ui.card = None;
    }
}

/// Enter in a chat room field runs the action beside it.
fn room_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    fields: Query<&RoomField>,
    mut desk: RoomDesk,
) {
    let Some(field) = focus.get().and_then(|field| fields.get(field).ok()) else {
        return;
    };
    if !keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        return;
    }
    keys.clear_just_pressed(KeyCode::Enter);
    keys.clear_just_pressed(KeyCode::NumpadEnter);
    desk.run(match field {
        RoomField::Message => ChatRoomAction::Send,
        RoomField::Title | RoomField::Password => ChatRoomAction::SubmitForm,
        RoomField::CardPassword => ChatRoomAction::Join,
    });
}

/// Keeps the picks in step with the rooms: entering a room closes what led to it,
/// refusals are kept for the card or told in the chat, and a pick whose room is gone
/// is dropped.
fn follow_rooms(
    mut events: MessageReader<ChatRoomEvent>,
    rooms: Res<ChatRooms>,
    session: Res<ZoneSession>,
    mut ui: ResMut<ChatRoomUi>,
    mut fields: Query<(&RoomField, &mut EditableText)>,
    mut notices: MessageWriter<ChatNotice>,
) {
    let mut refuse = |message: &str| {
        notices.write(ChatNotice {
            error: true,
            message: message.into(),
        });
    };
    for event in events.read() {
        match event {
            ChatRoomEvent::Created | ChatRoomEvent::Joined { .. } => {
                // A room entered by joining was set up by someone else.
                if matches!(event, ChatRoomEvent::Joined { .. }) {
                    ui.password.clear();
                }
                ui.card = None;
                ui.refusal = None;
                ui.creating = false;
                ui.editing = false;
                let drafts = fields
                    .iter_mut()
                    .filter(|(field, _)| **field == RoomField::Message);
                for (_, mut draft) in drafts {
                    draft.clear();
                }
            }
            ChatRoomEvent::JoinRefused { room_id, reason } => {
                ui.refusal = Some((*room_id, *reason));
            }
            ChatRoomEvent::CreateRefused => refuse("A chat room cannot be opened here."),
            ChatRoomEvent::MemberLeft {
                member,
                kicked: true,
                ..
            } if member.char_id == session.char_id => {
                refuse("You were removed from the chat room.");
            }
            _ => {}
        }
    }
    if ui.editing && rooms.joined().is_none() {
        ui.editing = false;
    }
    if ui
        .card
        .is_some_and(|room_id| !rooms.signs().contains_key(&room_id))
    {
        ui.card = None;
    }
}

/// Keeps one card over the owner of the picked room, while that owner is in view.
fn sync_card(
    mut commands: Commands,
    ui: Res<ChatRoomUi>,
    rooms: Res<ChatRooms>,
    registry: Res<EntityRegistry>,
    cards: Query<(Entity, &RoomCard)>,
) {
    let wanted = ui
        .card
        .and_then(|room_id| rooms.signs().get(&room_id))
        .and_then(|sign| {
            Some(RoomCard {
                room_id: sign.room_id,
                locked: !sign.public,
                target: registry.get_entity(sign.owner_gid)?,
            })
        });
    let mut kept = false;
    for (entity, card) in &cards {
        if Some(*card) == wanted {
            kept = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    let Some(card) = wanted.filter(|_| !kept) else {
        return;
    };
    let anchor = WorldAnchor {
        target: card.target,
        width: scene::CARD_WIDTH,
        drop: scene::CARD_DROP,
    };
    commands
        .spawn_scene(scene::card(card.locked))
        .insert((card, anchor));
}

/// Copies what is typed in a form's fields into [`ChatRoomUi::form`], which the
/// form's views are computed from, and counts the title's characters.
fn mirror_form(
    fields: Query<(&RoomField, &EditableText), Changed<EditableText>>,
    mut ui: ResMut<ChatRoomUi>,
    mut counts: Query<&mut Text, With<TitleCount>>,
) {
    for (field, text) in &fields {
        let typed = text.value().to_string();
        match field {
            RoomField::Title if ui.form.title != typed => ui.form.title = typed,
            RoomField::Password if ui.form.password != typed => ui.form.password = typed,
            _ => {}
        }
    }
    let count = format!("{} / {}", ui.form.title.chars().count(), view::TITLE_MAX);
    for mut text in &mut counts {
        if text.0 != count {
            text.0.clone_from(&count);
        }
    }
}

/// What a [`ChatRoomHost`] shows.
#[derive(Clone, PartialEq)]
enum Content {
    Head(view::HeadView),
    Log(Vec<view::LineView>),
    People(view::PeopleView),
    /// The owner's settings panel: the form's skeleton.
    Settings,
    /// The New Chat Room dialog: the form's skeleton in its own window.
    Dialog,
    Options(view::OptionsView),
    Foot(view::FootView),
    CardHead(view::CardHeadView),
    CardFoot(view::CardFootView),
}

impl Content {
    /// `form` fills the fields of a form's skeleton as it is spawned.
    fn spawn(self, commands: &mut Commands, form: &RoomForm) -> Entity {
        match self {
            Content::Head(view) => commands.spawn_scene(scene::head(view)).id(),
            Content::Log(lines) => commands.spawn_scene(scene::log(lines)).id(),
            Content::People(view) => commands.spawn_scene(scene::people(view)).id(),
            Content::Settings => commands.spawn_scene(scene::settings(form)).id(),
            Content::Dialog => commands.spawn_scene(scene::dialog(form)).id(),
            Content::Options(view) => commands.spawn_scene(scene::options(view)).id(),
            Content::Foot(view) => commands.spawn_scene(scene::foot(view)).id(),
            Content::CardHead(view) => commands.spawn_scene(scene::card_head(view)).id(),
            Content::CardFoot(view) => commands.spawn_scene(scene::card_foot(view)).id(),
        }
    }
}

/// The world as the chat room UI reads it.
#[derive(SystemParam)]
pub struct RoomData<'w, 's> {
    rooms: Res<'w, ChatRooms>,
    ui: Res<'w, ChatRoomUi>,
    session: Res<'w, ZoneSession>,
    registry: Res<'w, EntityRegistry>,
    names: Query<'w, 's, &'static EntityName>,
}

impl RoomData<'_, '_> {
    fn content(&self, host: ChatRoomHost) -> Option<Content> {
        let me = self.session.char_id;
        let room = self.rooms.joined();
        let mode = self.ui.form_mode(&self.rooms, me);
        let card = self
            .ui
            .card
            .and_then(|room_id| self.rooms.signs().get(&room_id));
        match host {
            ChatRoomHost::Head => {
                room.map(|room| Content::Head(view::head(room, me, self.ui.editing)))
            }
            ChatRoomHost::Log => room.map(|room| Content::Log(view::log(room, me))),
            ChatRoomHost::Side => room.map(|room| match mode {
                Some(FormMode::Edit { .. }) => Content::Settings,
                _ => Content::People(view::people(room, me)),
            }),
            ChatRoomHost::Dialog => (mode == Some(FormMode::Create)).then_some(Content::Dialog),
            ChatRoomHost::FormOptions => {
                mode.map(|mode| Content::Options(view::options(&self.ui.form, mode)))
            }
            ChatRoomHost::FormFoot => {
                mode.map(|mode| Content::Foot(view::foot(&self.ui.form, mode)))
            }
            ChatRoomHost::CardHead => card.map(|sign| {
                let owner = self
                    .registry
                    .get_entity(sign.owner_gid)
                    .and_then(|owner| self.names.get(owner).ok());
                Content::CardHead(view::card_head(sign, owner.map(|name| name.name.as_str())))
            }),
            ChatRoomHost::CardFoot => card.map(|sign| {
                let refusal = self.ui.refusal_of(sign.room_id);
                Content::CardFoot(view::card_foot(sign, refusal, room.is_some()))
            }),
        }
    }
}

type HostNode<'a> = (
    Entity,
    Ref<'a, ChatRoomHost>,
    Option<&'a Children>,
    Option<&'a mut ScrollPosition>,
);

/// Refills every [`ChatRoomHost`] whose view changed. A refilled log jumps to its
/// newest line.
fn rebuild_chat_room_ui(
    mut commands: Commands,
    data: RoomData,
    mut hosts: Query<HostNode>,
    mut shown: Local<[Option<Content>; HOSTS]>,
) {
    for (host, kind, children, scroll) in &mut hosts {
        let content = data.content(*kind);
        let slot = &mut shown[*kind as usize];
        if *slot == content && !kind.is_added() {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).despawn();
        }
        if let Some(content) = content.clone() {
            let spawned = content.spawn(&mut commands, &data.ui.form);
            commands.entity(spawned).insert(ChildOf(host));
        }
        if let Some(mut scroll) = scroll {
            scroll.0.y = scene::LOG_END;
        }
        *slot = content;
    }
}

/// The window is on screen exactly while the player is in a room.
fn show_window(rooms: Res<ChatRooms>, mut window: Query<&mut Visibility, With<ChatRoomWindow>>) {
    let wanted = shown_if(rooms.joined().is_some());
    for mut visibility in &mut window {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

fn show_password_row(ui: Res<ChatRoomUi>, mut rows: Query<&mut Node, With<PasswordRow>>) {
    let wanted = scene::shown_row(ui.form.private);
    for mut node in &mut rows {
        if node.display != wanted {
            node.display = wanted;
        }
    }
}

type HoverChanged = (With<PersonRow>, Changed<Hovered>);

/// Shows a person's buttons while the pointer is on their row.
fn reveal_person_actions(
    rows: Query<(&Hovered, &Children), HoverChanged>,
    mut actions: Query<&mut Visibility, With<PersonActions>>,
) {
    for (hovered, children) in &rows {
        for child in children {
            if let Ok(mut visibility) = actions.get_mut(*child) {
                *visibility = shown_if(hovered.0);
            }
        }
    }
}

/// Everything a chat room button reads or changes.
#[derive(SystemParam)]
pub struct RoomDesk<'w, 's> {
    ui: ResMut<'w, ChatRoomUi>,
    rooms: Res<'w, ChatRooms>,
    session: Res<'w, ZoneSession>,
    fields: Query<'w, 's, (&'static RoomField, &'static mut EditableText)>,
    out: MessageWriter<'w, ChatRoomCommand>,
}

impl RoomDesk<'_, '_> {
    /// What is typed in `which`, or nothing while it is not on screen.
    fn typed(&self, which: RoomField) -> String {
        self.fields
            .iter()
            .find(|(field, _)| **field == which)
            .map_or_else(String::new, |(_, text)| text.value().to_string())
    }

    fn member_name(&self, char_id: u32) -> Option<String> {
        let members = &self.rooms.joined()?.members;
        let member = members.iter().find(|member| member.char_id == char_id)?;
        Some(member.name.clone())
    }

    fn run(&mut self, action: ChatRoomAction) {
        match action {
            ChatRoomAction::Leave => {
                self.out.write(ChatRoomCommand::Leave);
            }
            ChatRoomAction::Send => self.say(),
            ChatRoomAction::ToggleSettings => {
                let Some(room) = self.rooms.joined() else {
                    return;
                };
                self.ui.editing ^= true;
                if self.ui.editing {
                    self.ui.form = RoomForm::of(&room.sign, &self.ui.password);
                }
            }
            ChatRoomAction::MakeOwner(char_id) => {
                if let Some(name) = self.member_name(char_id) {
                    self.out.write(ChatRoomCommand::MakeOwner(name));
                }
            }
            ChatRoomAction::Kick(char_id) => {
                if let Some(name) = self.member_name(char_id) {
                    self.out.write(ChatRoomCommand::Kick(name));
                }
            }
            ChatRoomAction::Seats(delta) => {
                let mode = self.ui.form_mode(&self.rooms, self.session.char_id);
                let seated = mode.map_or(0, FormMode::seated);
                self.ui.form.step_seats(delta, seated);
            }
            ChatRoomAction::Access(private) => self.ui.form.private = private,
            ChatRoomAction::SubmitForm => self.submit(),
            ChatRoomAction::CancelForm => {
                self.ui.creating = false;
                self.ui.editing = false;
            }
            ChatRoomAction::Join => {
                let Some(room_id) = self.ui.card else {
                    return;
                };
                let password = self.typed(RoomField::CardPassword);
                self.out.write(ChatRoomCommand::Join { room_id, password });
                self.ui.refusal = None;
            }
            ChatRoomAction::CloseCard => self.ui.card = None,
        }
    }

    /// Says the typed line and empties the field. A blank line says nothing.
    fn say(&mut self) {
        let line = self.typed(RoomField::Message).trim().to_string();
        if line.is_empty() || self.rooms.joined().is_none() {
            return;
        }
        self.out.write(ChatRoomCommand::Say(line));
        for (field, mut draft) in &mut self.fields {
            if *field == RoomField::Message {
                draft.clear();
            }
        }
    }

    /// Opens the new room or saves the edited one, once the form is valid.
    fn submit(&mut self) {
        let Some(mode) = self.ui.form_mode(&self.rooms, self.session.char_id) else {
            return;
        };
        if !self.ui.form.is_valid() {
            return;
        }
        let settings = self.ui.form.settings();
        self.ui.password.clone_from(&settings.password);
        self.ui.creating = false;
        self.ui.editing = false;
        self.out.write(match mode {
            FormMode::Create => ChatRoomCommand::Create(settings),
            FormMode::Edit { .. } => ChatRoomCommand::ChangeSettings(settings),
        });
    }
}

/// Runs the [`ChatRoomAction`] of an activated button. The headless button also
/// activates on a right click, which is ignored.
pub fn on_room_button(
    activate: On<Activate>,
    actions: Query<&ChatRoomAction>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut desk: RoomDesk,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(action) = actions.get(activate.entity) {
        desk.run(*action);
    }
}

#[cfg(test)]
mod tests;
