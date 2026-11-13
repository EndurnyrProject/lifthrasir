//! Party UI: the party window, the HUD frames and the invitation dialog, after the
//! `Party`, `Party Frames (HUD)` and `Party Invitation` artboards of the Lifthrasir
//! Widgets design.
//!
//! Authority stays in [`PartyState`] and the server: this module projects that state
//! and turns its controls into outbound commands. Each [`PartyHost`] is a region
//! [`rebuild_party_ui`] refills when its view-model changes. The views leave out what
//! changes all the time (HP, SP, distance); [`sync_readouts`] refreshes those in
//! place, so a member taking damage never respawns a button mid-click.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::input::targeting::TargetingMode;
use lifthrasir_client::domain::input::{PlayerAction, ui_unfocused};
use lifthrasir_client::domain::party::PartyState;
use lifthrasir_client::domain::skill::{CastTarget, SkillCastResolved};
use lifthrasir_client::domain::trade::TradeSession;
use lifthrasir_client::domain::vending::VendingState;
use lifthrasir_runtime::rendering::coordinates::RO_UNITS_PER_CELL;
use net_contract::commands::{
    PartyCreateRequested, PartyInviteRequested, PartyInviteResponded, PartyKickRequested,
    PartyLeaderRequested, PartyLeaveRequested, PartyOptionsRequested, RequestTrade,
};
use net_contract::state::ZoneSession;

pub mod feedback;
pub mod frames;
pub mod invite_dialog;
pub mod scene;
pub mod slash;
mod view;

pub use crate::widgets::party::invite_dialog::PendingPartyInvite;
pub use crate::widgets::party::slash::{PartySlash, PartySlashSubmitted};

use crate::screens::shared::{light_search_focus, set_field_text};
use view::{MemberReadout, ReadoutPart, ReadoutValue, Roster};

/// Aesir's hard cap on party size.
pub const PARTY_MAX: usize = 12;
/// Longest party or hero name the name field submits.
const NAME_MAX_CHARS: usize = 24;

/// Marks the window root so the hotkey, drag and close act on this window only.
#[derive(Component, Default, Clone)]
pub struct PartyWindowRoot;

/// The draggable title bar.
#[derive(Component, Default, Clone)]
pub struct PartyTitlebar;

/// A region [`rebuild_party_ui`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyHost {
    /// The window's heading: party name and head count.
    #[default]
    Head,
    /// The name field that founds a party or invites a hero.
    Form,
    /// The roster, the sharing options and the level track.
    Body,
    /// The party frames on the HUD.
    Frames,
    /// The invitation dialog.
    Invite,
}

/// The name typed to found a party or invite a hero; read on submit.
#[derive(Component, Default, Clone)]
pub struct PartyNameField;

/// What a party button does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyAction {
    #[default]
    CloseWindow,
    /// Founds a party or invites a hero with the typed name.
    Submit,
    Select(u32),
    Trade(u32),
    MakeLeader(u32),
    /// Raises the expel prompt for the selected member.
    AskExpel,
    KeepMember,
    Expel(u32),
    ExpShare(bool),
    PickupShare(bool),
    /// Raises the leave prompt.
    AskLeave,
    Stay,
    Leave,
    /// Collapses or expands the HUD frames.
    ToggleFrames,
    /// Casts the armed skill on a member, like a click on their character.
    CastOn(u32),
    AnswerInvite(bool),
}

/// What the player picked in the party UI. None of it is sent to the server.
#[derive(Resource, Default, Debug)]
pub struct PartyUi {
    selected: Option<u32>,
    ask_expel: bool,
    ask_leave: bool,
    frames_collapsed: bool,
}

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingPartyInvite>();
        app.init_resource::<PartyUi>();
        app.add_message::<PartySlashSubmitted>();
        app.add_observer(crate::widgets::player_context_menu::open_player_menu);
        app.add_systems(
            Update,
            toggle_party_window.run_if(in_state(GameState::InGame).and_then(ui_unfocused)),
        );
        app.add_systems(
            Update,
            (
                party_visibility,
                slash::dispatch_party_slash,
                invite_dialog::receive_invite,
                invite_dialog::expire_pending_invite,
                rebuild_party_ui,
                sync_readouts,
                invite_dialog::sync_invite_clock,
                light_search_focus::<PartyNameField>,
                feedback::ingest_party_feedback,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// Spawns the party window, the frames host and the invitation host under the HUD root.
pub fn spawn_party_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
    commands.spawn_scene(frames::host()).insert(ChildOf(parent));
    commands
        .spawn_scene(invite_dialog::host())
        .insert(ChildOf(parent));
}

/// Keep the window closed when joining a party and hide it when leaving one.
pub fn party_visibility(
    party: Res<PartyState>,
    mut prev_in_party: Local<bool>,
    mut root: Query<&mut Visibility, With<PartyWindowRoot>>,
) {
    let now = party.in_party();
    if now == *prev_in_party {
        return;
    }
    *prev_in_party = now;
    if now {
        return;
    }
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    *visibility = Visibility::Hidden;
}

/// `PlayerAction::Party` toggles the window. It works while partyless too, since the
/// window is where a party is founded.
pub fn toggle_party_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut window: Query<&mut Visibility, With<PartyWindowRoot>>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Party) {
        return;
    }
    let Ok(mut visibility) = window.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Visible,
        _ => Visibility::Hidden,
    };
}

/// The world as the party UI reads it.
#[derive(SystemParam)]
pub struct PartyData<'w, 's> {
    party: Res<'w, PartyState>,
    session: Res<'w, ZoneSession>,
    registry: Res<'w, EntityRegistry>,
    jobs: Option<Res<'w, JobSpriteRegistry>>,
    places: Query<'w, 's, &'static GlobalTransform>,
}

impl PartyData<'_, '_> {
    fn roster(&self) -> Roster<'_> {
        Roster {
            party: &self.party,
            me: self.session.char_id,
            jobs: self.jobs.as_deref(),
        }
    }

    /// Distance in cells from the local player to the character `char_id`, while that
    /// character is in view.
    fn cells(&self, char_id: u32) -> Option<u32> {
        let place = |entity| self.places.get(entity).map(GlobalTransform::translation);
        let own = place(self.registry.local_player_entity()?).ok()?;
        let other = place(self.registry.get_entity(char_id)?).ok()?;
        let gap = (other - own).abs() / RO_UNITS_PER_CELL;
        Some(gap.x.max(gap.z).round() as u32)
    }
}

/// What a [`PartyHost`] shows.
#[derive(Clone, PartialEq)]
enum Content {
    Head(view::HeadView),
    Form(view::FormView),
    Body(view::BodyView),
    Frames(view::FramesView),
    Invite(view::InviteView),
}

impl Content {
    fn spawn(self, commands: &mut Commands) -> Entity {
        match self {
            Content::Head(view) => commands.spawn_scene(scene::head(view)).id(),
            Content::Form(view) => commands.spawn_scene(scene::form(view)).id(),
            Content::Body(view) => commands.spawn_scene(scene::body(view)).id(),
            Content::Frames(view) => commands.spawn_scene(frames::panel(view)).id(),
            Content::Invite(view) => commands.spawn_scene(invite_dialog::dialog(view)).id(),
        }
    }
}

/// What the hosts show besides the roster.
#[derive(SystemParam)]
pub struct PartyShown<'w, 's> {
    ui: Res<'w, PartyUi>,
    pending: Res<'w, PendingPartyInvite>,
    trade: Res<'w, TradeSession>,
    vending: Res<'w, VendingState>,
    window: Query<'w, 's, &'static Visibility, With<PartyWindowRoot>>,
}

/// Refills every [`PartyHost`] whose view changed. The window's hosts are empty while
/// it is hidden, the frames while partyless and the invitation while none is pending.
fn rebuild_party_ui(
    mut commands: Commands,
    data: PartyData,
    state: PartyShown,
    hosts: Query<(Entity, Ref<PartyHost>, Option<&Children>)>,
    mut shown: Local<[Option<Content>; 5]>,
) {
    let open = state
        .window
        .single()
        .is_ok_and(|visibility| *visibility != Visibility::Hidden);
    let roster = data.roster();
    let in_party = data.party.in_party();
    let ui = &*state.ui;
    // A vendor's shop locks trading out; the server enforces it too.
    let trade_free = !state.trade.is_open() && !state.vending.gameplay_locked();
    for (host, kind, children) in &hosts {
        let content = match *kind {
            PartyHost::Head => open.then(|| Content::Head(view::head(roster))),
            PartyHost::Form => open.then(|| Content::Form(view::form(roster))),
            PartyHost::Body => (open && in_party).then(|| {
                let in_view = |char_id| data.cells(char_id).is_some();
                Content::Body(view::body(roster, ui, in_view, trade_free))
            }),
            PartyHost::Frames => in_party.then(|| Content::Frames(view::frames(roster, ui))),
            PartyHost::Invite => state.pending.view().cloned().map(Content::Invite),
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

type ReadoutNode<'a> = (
    &'a MemberReadout,
    &'a mut Node,
    Option<&'a mut Text>,
    Option<&'a mut TextColor>,
);

/// Refreshes every [`MemberReadout`] from the roster, writing only what changed.
fn sync_readouts(data: PartyData, mut readouts: Query<ReadoutNode>) {
    let roster = data.roster();
    for (readout, mut node, text, color) in &mut readouts {
        let Some(member) = data
            .party
            .members
            .iter()
            .find(|member| member.char_id == readout.char_id)
        else {
            continue;
        };
        let placed = matches!(readout.part, ReadoutPart::Where | ReadoutPart::FrameWhere);
        let cells = placed.then(|| data.cells(member.char_id)).flatten();
        match view::readout(readout.part, member, roster, cells) {
            ReadoutValue::Fill(fill) => {
                let width = Val::Percent(fill);
                if node.width != width {
                    node.width = width;
                }
            }
            ReadoutValue::Text(value, tone) => {
                if let Some(mut text) = text
                    && text.0 != value
                {
                    text.0 = value;
                }
                if let Some(mut color) = color
                    && color.0 != tone
                {
                    color.0 = tone;
                }
            }
        }
    }
}

/// The name to submit: trimmed, capped and non-empty, else `None`.
pub fn submit_name(raw: &str) -> Option<String> {
    let name: String = raw.trim().chars().take(NAME_MAX_CHARS).collect();
    (!name.is_empty()).then_some(name)
}

/// The commands a party button can send.
#[derive(SystemParam)]
pub struct PartyCommands<'w> {
    create: MessageWriter<'w, PartyCreateRequested>,
    invite: MessageWriter<'w, PartyInviteRequested>,
    respond: MessageWriter<'w, PartyInviteResponded>,
    leave: MessageWriter<'w, PartyLeaveRequested>,
    kick: MessageWriter<'w, PartyKickRequested>,
    leader: MessageWriter<'w, PartyLeaderRequested>,
    options: MessageWriter<'w, PartyOptionsRequested>,
    trade: MessageWriter<'w, RequestTrade>,
    casts: MessageWriter<'w, SkillCastResolved>,
}

/// Everything a party button reads or changes.
#[derive(SystemParam)]
pub struct PartyActions<'w, 's> {
    ui: ResMut<'w, PartyUi>,
    party: Res<'w, PartyState>,
    targeting: ResMut<'w, TargetingMode>,
    pending: ResMut<'w, PendingPartyInvite>,
    window: Query<'w, 's, &'static mut Visibility, With<PartyWindowRoot>>,
    names: Query<'w, 's, &'static mut EditableText, With<PartyNameField>>,
    out: PartyCommands<'w>,
}

impl PartyActions<'_, '_> {
    fn run(&mut self, action: PartyAction) {
        match action {
            PartyAction::CloseWindow => {
                if let Ok(mut visibility) = self.window.single_mut() {
                    *visibility = Visibility::Hidden;
                }
            }
            PartyAction::Submit => self.submit(),
            PartyAction::Select(char_id) => {
                self.ui.selected = Some(char_id);
                self.ui.ask_expel = false;
            }
            PartyAction::Trade(target_char_id) => {
                self.out.trade.write(RequestTrade { target_char_id });
            }
            PartyAction::MakeLeader(target_char_id) => {
                self.out
                    .leader
                    .write(PartyLeaderRequested { target_char_id });
            }
            PartyAction::AskExpel => self.ui.ask_expel = true,
            PartyAction::KeepMember => self.ui.ask_expel = false,
            PartyAction::Expel(target_char_id) => {
                self.out.kick.write(PartyKickRequested { target_char_id });
                self.ui.ask_expel = false;
                self.ui.selected = None;
            }
            // The server takes both options as one desired state, so the untouched one
            // is sent as last confirmed.
            PartyAction::ExpShare(exp_share) => {
                self.out.options.write(PartyOptionsRequested {
                    exp_share,
                    item_pickup_share: self.party.item_pickup_share,
                });
            }
            PartyAction::PickupShare(item_pickup_share) => {
                self.out.options.write(PartyOptionsRequested {
                    exp_share: self.party.exp_share,
                    item_pickup_share,
                });
            }
            PartyAction::AskLeave => self.ui.ask_leave = true,
            PartyAction::Stay => self.ui.ask_leave = false,
            PartyAction::Leave => {
                self.out.leave.write(PartyLeaveRequested);
                self.ui.ask_leave = false;
            }
            PartyAction::ToggleFrames => self.ui.frames_collapsed ^= true,
            PartyAction::CastOn(char_id) => self.cast_on(char_id),
            PartyAction::AnswerInvite(accept) => {
                if !self.pending.is_pending() {
                    return;
                }
                self.out.respond.write(PartyInviteResponded {
                    party_id: self.pending.party_id,
                    accept,
                });
                self.pending.clear();
            }
        }
    }

    /// Sends the typed name and empties the field. An empty name sends nothing.
    fn submit(&mut self) {
        let Ok(mut field) = self.names.single_mut() else {
            return;
        };
        let Some(name) = submit_name(&field.value().to_string()) else {
            return;
        };
        if self.party.in_party() {
            self.out.invite.write(PartyInviteRequested {
                target_char_id: 0,
                target_name: name,
            });
        } else {
            self.out.create.write(PartyCreateRequested { name });
        }
        set_field_text(&mut field, "");
    }

    /// Resolves a skill armed for an entity target onto a member. Idle clicks do
    /// nothing: the client has no free targeting.
    fn cast_on(&mut self, char_id: u32) {
        let TargetingMode::AwaitingEntity { skill_id, level } = *self.targeting else {
            return;
        };
        self.out.casts.write(SkillCastResolved {
            skill_id,
            level,
            target: CastTarget::Entity(char_id),
        });
        *self.targeting = TargetingMode::Idle;
    }
}

/// Runs the [`PartyAction`] of an activated button. The headless button also activates
/// on a right click, which is ignored.
pub fn on_party_button(
    activate: On<Activate>,
    actions: Query<&PartyAction>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut party: PartyActions,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(action) = actions.get(activate.entity) {
        party.run(*action);
    }
}

#[cfg(test)]
mod tests;
