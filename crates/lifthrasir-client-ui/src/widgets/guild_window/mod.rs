//! Guild UI: the guild window and the invitation dialog, after the `Guild` and
//! `Guild Invitation` artboards of the Lifthrasir Widgets design.
//!
//! Authority stays in [`GuildState`] and the server: this module projects that state
//! and turns its controls into outbound commands. Each [`GuildHost`] is a region
//! [`rebuild_guild_ui`] refills when its view-model changes. The page of the tab on
//! show is itself a host whose skeleton holds further hosts, so the fields a player
//! types in outlive the data changing around them.
//!
//! One request is in flight at a time ([`GuildUi::pending`]); its result, matched by
//! action name, releases the slot and becomes the toast over the panel.

pub(crate) mod emblem;
mod feedback;
pub mod invite_dialog;
mod relation_dialogs;
pub mod scene;
pub(crate) mod slash;
mod view;

pub use crate::widgets::guild_window::invite_dialog::PendingGuildInvite;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use leafwing_input_manager::prelude::ActionState;
use lifthrasir_assets::catalogs::job::registry::JobSpriteRegistry;
use lifthrasir_assets::catalogs::skill::registry::SkillCatalog;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::guild::{GuildState, GuildSystems};
use lifthrasir_client::domain::input::{PlayerAction, UiFocus};
use net_contract::commands::{
    GuildAllianceRequested, GuildAntagonistRemoveRequested, GuildCreateRequested,
    GuildExpelRequested, GuildInviteRequested, GuildInviteResponded, GuildLeaveRequested,
    GuildMemberPositionRequested, GuildNoticeEditRequested, GuildPositionEditRequested,
    GuildSkillUpRequested, PartyInviteRequested,
};
use net_contract::dto::{GuildPositionInfo, GuildRelationKind};
use net_contract::events::ZoneDisconnected;
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::screens::shared::{light_field_focus, light_search_focus, set_field_text};
use crate::widgets::chat_box::ChatUi;
use crate::widgets::info_modal::{InfoContent, ShowInfoModal, view as info_view};
use view::Guild;

/// Longest hero or position name a field submits.
const NAME_MAX_CHARS: usize = 24;
/// How long the toast stays up before it clears itself.
const FEEDBACK_DISMISS: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuildTab {
    #[default]
    Overview,
    Members,
    Positions,
    Skills,
    Relations,
}

/// What the roster is ordered by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemberSort {
    Name,
    Job,
    Level,
    Position,
    Exp,
    /// Online members first, then whoever was seen last.
    #[default]
    Seen,
}

/// One thing a position may be allowed to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Perm {
    #[default]
    Invite,
    Expel,
    Storage,
    Notice,
    Relations,
}

impl Perm {
    pub const ALL: [Perm; 5] = [
        Perm::Invite,
        Perm::Expel,
        Perm::Storage,
        Perm::Notice,
        Perm::Relations,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Perm::Invite => "INVITE",
            Perm::Expel => "EXPEL",
            Perm::Storage => "STORAGE",
            Perm::Notice => "NOTICE",
            Perm::Relations => "RELATIONS",
        }
    }

    /// Whether `position` has it.
    pub fn of(self, position: &GuildPositionInfo) -> bool {
        match self {
            Perm::Invite => position.can_invite,
            Perm::Expel => position.can_expel,
            Perm::Storage => position.can_storage,
            Perm::Notice => position.can_notice,
            Perm::Relations => position.can_relations,
        }
    }
}

/// The inline prompt that is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ask {
    Leave,
    Expel,
}

/// The request the server has not answered yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGuildMutation {
    pub action: &'static str,
    pub generation: ZoneSessionGeneration,
}

/// What the player picked in the guild window. None of it is sent to the server.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct GuildUi {
    pub selected_tab: GuildTab,
    /// The toast over the panel.
    pub feedback: Option<String>,
    pub pending: Option<PendingGuildMutation>,
    feedback_is_error: bool,
    selected_member: Option<u32>,
    online_only: bool,
    sort: MemberSort,
    sort_reversed: bool,
    ask: Option<Ask>,
    /// The selected member's position menu is open.
    position_menu: bool,
    /// The position whose edit bar is up.
    selected_position: Option<u32>,
    editing_notice: bool,
}

impl GuildUi {
    fn say(&mut self, text: impl Into<String>) {
        self.feedback = Some(text.into());
        self.feedback_is_error = false;
    }

    fn fail(&mut self, text: impl Into<String>) {
        self.feedback = Some(text.into());
        self.feedback_is_error = true;
    }

    /// Books `action` as the request in flight and shows `progress`. Refuses, saying
    /// so, while another request is still unanswered.
    fn begin(
        &mut self,
        action: &'static str,
        generation: ZoneSessionGeneration,
        progress: &str,
    ) -> bool {
        if self.pending.is_some() {
            self.fail("A guild action is already pending.");
            return false;
        }
        self.pending = Some(PendingGuildMutation { action, generation });
        self.say(progress);
        true
    }
}

/// The session the guild UI was last reset for.
#[derive(Resource, Default)]
pub(crate) struct GuildUiSession {
    generation: ZoneSessionGeneration,
    char_id: u32,
    guild_id: u32,
    /// The session ended; nothing from it may be shown or acted on.
    blocked: bool,
    /// The UI was reset this frame.
    reset: bool,
}

/// The name to submit: trimmed, capped and non-empty, else `None`.
fn submit_name(raw: &str) -> Option<String> {
    let name: String = raw.trim().chars().take(NAME_MAX_CHARS).collect();
    (!name.is_empty()).then_some(name)
}

fn request_create(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    raw_name: &str,
) -> Option<GuildCreateRequested> {
    if ui.pending.is_some() {
        ui.fail("A guild action is already pending.");
        return None;
    }
    let Some(name) = submit_name(raw_name) else {
        ui.fail("Enter a guild name.");
        return None;
    };
    ui.begin("create", generation, "Creating guild\u{2026}");
    Some(GuildCreateRequested { name })
}

/// Invites a hero by id (from their context menu) or by name (from the window);
/// exactly one of the two must be given.
pub(crate) fn request_invite(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    target_char_id: u32,
    raw_name: &str,
) -> Option<GuildInviteRequested> {
    if ui.pending.is_some() {
        ui.fail("A guild action is already pending.");
        return None;
    }
    let target_name = raw_name.trim();
    if (target_char_id == 0) == target_name.is_empty() {
        ui.fail("Enter a character name.");
        return None;
    }
    ui.begin("invite", generation, "Sending guild invitation\u{2026}");
    Some(GuildInviteRequested {
        target_char_id,
        target_name: target_name.into(),
    })
}

/// A whole percentage from 0 to 100.
fn parse_tax(raw: &str) -> Option<u32> {
    raw.parse::<u32>().ok().filter(|tax| *tax <= 100)
}

/// Marks the window root so the hotkey, drag and close act on this window only.
#[derive(Component, Default, Clone)]
pub struct GuildWindowRoot;

/// The draggable title bar.
#[derive(Component, Default, Clone)]
pub struct GuildTitlebar;

/// A region [`rebuild_guild_ui`] refills when its view changes.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildHost {
    /// The guild and the player's position, in the title bar.
    #[default]
    Title,
    Tabs,
    /// The skeleton of the tab on show; it holds the hosts below.
    Page,
    Summary,
    Notice,
    Activity,
    /// The online filter and the head count over the roster.
    Tools,
    MemberHead,
    MemberRows,
    /// The bar acting on the selected member.
    Selection,
    Positions,
    PositionEdit,
    SkillHead,
    Skills,
    Relations,
    Footer,
    Toast,
    /// The invitation dialog.
    Invite,
}

/// How many [`GuildHost`] regions there are.
const HOSTS: usize = GuildHost::Invite as usize + 1;

impl GuildHost {
    /// Whether the host lives in the page's skeleton rather than in the frame.
    fn in_page(self) -> bool {
        (GuildHost::Page as usize) < self as usize && (self as usize) < GuildHost::Footer as usize
    }
}

/// The roster's search box.
#[derive(Component, Default, Clone)]
pub struct MemberSearchField;
/// The name of the hero to invite.
#[derive(Component, Default, Clone)]
pub struct GuildInviteNameField;
/// Why the selected member is expelled.
#[derive(Component, Default, Clone)]
pub struct ExpelReasonField;
#[derive(Component, Default, Clone)]
pub struct NoticeSubjectField;
#[derive(Component, Default, Clone)]
pub struct NoticeBodyField;
#[derive(Component, Default, Clone)]
pub struct PositionNameField;
#[derive(Component, Default, Clone)]
pub struct PositionTaxField;
/// The master of the guild to ally with.
#[derive(Component, Default, Clone)]
pub struct GuildAllianceNameField;
/// A hero of the guild to declare an antagonist.
#[derive(Component, Default, Clone)]
pub struct GuildAntagonistNameField;

/// An image that shows the emblem of a guild once it is cached.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmblemSlot {
    pub guild_id: u32,
    pub emblem_id: u32,
}

/// A guild skill card; a right click opens the skill's details.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct SkillCell(pub u32);

/// What a guild button does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildAction {
    #[default]
    CloseWindow,
    Tab(GuildTab),
    /// Opens the file picker for a new emblem.
    PickEmblem,
    EditNotice,
    CancelNotice,
    SaveNotice,
    ToggleOnline,
    Sort(MemberSort),
    Select(u32),
    /// Invites the hero typed in the name field.
    Invite,
    Whisper(u32),
    PartyInvite(u32),
    TogglePositionMenu,
    /// Moves a member to a position slot.
    Assign(u32, u32),
    /// Raises the expel prompt for the selected member.
    AskExpel,
    KeepMember,
    Expel(u32),
    /// Opens or closes a position's edit bar.
    SelectPosition(u32),
    TogglePerm(u32, Perm),
    SavePosition(u32),
    SkillUp(u32),
    ProposeAlliance,
    DeclareAntagonist,
    BreakAlliance(u32),
    RemoveAntagonist(u32),
    /// Raises the leave prompt.
    AskLeave,
    Stay,
    Leave,
    Storage,
    AnswerInvite(bool),
    /// Leaves the welcome for the guild window.
    OpenGuild,
    DismissInvite,
}

#[derive(SystemParam)]
pub(crate) struct GuildMutationContext<'w> {
    pub guild: Res<'w, GuildState>,
    pub session: Res<'w, ZoneSession>,
    pub generation: Res<'w, ZoneSessionGeneration>,
    pub ui: ResMut<'w, GuildUi>,
}

type GuildTextFieldFilter = Or<(
    With<MemberSearchField>,
    With<GuildInviteNameField>,
    With<ExpelReasonField>,
    With<NoticeSubjectField>,
    With<NoticeBodyField>,
    With<PositionNameField>,
    With<PositionTaxField>,
    With<GuildAllianceNameField>,
    With<GuildAntagonistNameField>,
)>;
type GuildTextFields<'w, 's> = Query<'w, 's, Entity, GuildTextFieldFilter>;

pub struct GuildWindowPlugin;

impl Plugin for GuildWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<slash::GuildSlashSubmitted>()
            .init_resource::<GuildUi>()
            .init_resource::<GuildUiSession>()
            .init_resource::<emblem::GuildEmblemPreview>()
            .init_resource::<PendingGuildInvite>()
            .init_resource::<relation_dialogs::PendingAllianceInvite>()
            .init_resource::<relation_dialogs::PendingRelationConfirmation>()
            .add_systems(
                Update,
                (reset_guild_ui_session, emblem::reset_emblem_preview)
                    .chain()
                    .in_set(GuildSystems::SessionReset),
            )
            .add_systems(
                Update,
                toggle_guild_window
                    .before(GuildSystems::UiSync)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    (
                        reset_guild_ui_guild,
                        relation_dialogs::reset_invalid_relation_dialogs,
                        slash::dispatch_guild_slash
                            .after(crate::widgets::chat_box::chat_input_control),
                        feedback::apply_guild_results,
                        relation_dialogs::expire_pending_alliance,
                        relation_dialogs::queue_incoming_alliance,
                        relation_dialogs::claim_alliance_choice,
                        relation_dialogs::claim_relation_confirmation,
                        feedback::ingest_guild_announcements,
                        invite_dialog::receive_invite,
                        invite_dialog::expire_pending_invite,
                        sync_membership_mode,
                        emblem::invalidate_picker_when_hidden,
                        emblem::poll_picker,
                        emblem::receive_emblem_changes,
                        emblem::queue_current_guild_emblem,
                    )
                        .chain(),
                    (
                        expire_feedback,
                        rebuild_guild_ui,
                        emblem::sync_emblems,
                        invite_dialog::sync_invite_clock,
                        (
                            light_search_focus::<MemberSearchField>,
                            light_search_focus::<GuildInviteNameField>,
                            light_search_focus::<ExpelReasonField>,
                            light_search_focus::<GuildAllianceNameField>,
                            light_search_focus::<GuildAntagonistNameField>,
                            light_field_focus::<NoticeSubjectField>,
                            light_field_focus::<NoticeBodyField>,
                            light_field_focus::<PositionNameField>,
                            light_field_focus::<PositionTaxField>,
                        ),
                        release_hidden_guild_focus,
                    )
                        .chain(),
                )
                    .chain()
                    .in_set(GuildSystems::UiSync)
                    .run_if(in_state(GameState::InGame)),
            );
        app.add_systems(
            PostUpdate,
            (
                relation_dialogs::show_pending_alliance,
                relation_dialogs::show_pending_relation_confirmation,
                relation_dialogs::close_finished_relation_dialogs,
            )
                .chain(),
        );
        app.add_systems(
            OnExit(GameState::InGame),
            (
                clear_guild_focus_on_exit,
                block_guild_ui_on_exit,
                invite_dialog::clear_pending_invite,
                relation_dialogs::clear_relation_dialogs,
            ),
        );
    }
}

/// Spawns the guild window and the invitation host under the HUD root.
pub fn spawn_guild_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(ChildOf(parent));
    commands
        .spawn_scene(invite_dialog::host())
        .insert(ChildOf(parent));
}

fn hide(roots: &mut Query<&mut Visibility, With<GuildWindowRoot>>) {
    for mut visibility in roots {
        *visibility = Visibility::Hidden;
    }
}

/// Starts the UI over when the zone session, the character or the connection changes.
fn reset_guild_ui_session(
    generation: Res<ZoneSessionGeneration>,
    zone_session: Option<Res<ZoneSession>>,
    mut disconnected: Option<MessageReader<ZoneDisconnected>>,
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut invite: ResMut<PendingGuildInvite>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
) {
    let disconnected = disconnected
        .as_mut()
        .is_some_and(|reader| reader.read().count() != 0);
    let char_id = zone_session.as_deref().map_or(0, |session| session.char_id);
    let generation_changed = session.generation != *generation;
    let character_changed = session.char_id != char_id;
    let reset = generation_changed || character_changed || disconnected;
    session.reset = reset;
    if !reset {
        return;
    }
    session.generation = *generation;
    session.char_id = char_id;
    session.blocked = !generation_changed && (disconnected || character_changed);
    *ui = GuildUi::default();
    invite.clear();
    hide(&mut roots);
}

/// Starts the UI over when the player's guild changes.
fn reset_guild_ui_guild(
    guild: Res<GuildState>,
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
) {
    let guild_id = guild.info().map_or(0, |info| info.guild_id);
    if session.guild_id == guild_id {
        return;
    }
    session.guild_id = guild_id;
    session.reset = true;
    *ui = GuildUi::default();
    hide(&mut roots);
}

fn block_guild_ui_on_exit(
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
) {
    session.blocked = true;
    *ui = GuildUi::default();
    hide(&mut roots);
}

/// `PlayerAction::Guild` toggles the window. It stays closed while guildless: a guild
/// is founded with `/guild`, not here.
fn toggle_guild_window(
    guild: Res<GuildState>,
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    ui_focus: Res<UiFocus>,
    mut root: Query<&mut Visibility, With<GuildWindowRoot>>,
    owned_fields: GuildTextFields,
    mut input_focus: ResMut<InputFocus>,
) {
    let Ok(actions) = player.single() else {
        return;
    };
    if !actions.just_pressed(&PlayerAction::Guild) {
        return;
    }
    let Ok(mut visibility) = root.single_mut() else {
        return;
    };
    if *visibility == Visibility::Hidden {
        if !guild.in_guild() || ui_focus.text_input_active {
            return;
        }
        *visibility = Visibility::Visible;
        return;
    }
    *visibility = Visibility::Hidden;
    clear_guild_focus(&mut input_focus, &owned_fields);
}

/// Takes the `ResMut` wrapper (not `&mut InputFocus`) so the no-op path never
/// deref-muts the resource: this runs every frame via
/// `release_hidden_guild_focus`, and an unconditional deref-mut would flag
/// `InputFocus` as changed each frame for every `is_changed` consumer.
fn clear_guild_focus(input_focus: &mut ResMut<InputFocus>, fields: &GuildTextFields) {
    if input_focus
        .get()
        .is_some_and(|focused| fields.contains(focused))
    {
        input_focus.clear();
    }
}

fn release_hidden_guild_focus(
    root: Query<&Visibility, With<GuildWindowRoot>>,
    fields: GuildTextFields,
    mut input_focus: ResMut<InputFocus>,
) {
    if root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Hidden)
    {
        clear_guild_focus(&mut input_focus, &fields);
    }
}

fn clear_guild_focus_on_exit(fields: GuildTextFields, mut input_focus: ResMut<InputFocus>) {
    clear_guild_focus(&mut input_focus, &fields);
}

/// Closes the window when the player is no longer in a guild.
fn sync_membership_mode(
    guild: Res<GuildState>,
    mut root: Query<&mut Visibility, With<GuildWindowRoot>>,
) {
    if !guild.in_guild() {
        hide(&mut root);
    }
}

/// Clears the toast after [`FEEDBACK_DISMISS`]. A new message restarts the wait;
/// setting the same text again while it shows does not.
fn expire_feedback(
    mut ui: ResMut<GuildUi>,
    time: Res<Time>,
    mut shown: Local<Option<(String, Duration)>>,
) {
    let now = time.elapsed();
    let Some(message) = ui.feedback.clone() else {
        *shown = None;
        return;
    };
    match shown.as_ref() {
        Some((current, since)) if *current == message => {
            if now - *since > FEEDBACK_DISMISS {
                ui.feedback = None;
                *shown = None;
            }
        }
        _ => *shown = Some((message, now)),
    }
}

/// What a [`GuildHost`] shows.
#[derive(Clone, PartialEq)]
enum Content {
    Title(String),
    Tabs(Vec<view::TabView>),
    Page(view::PageView),
    Summary(view::SummaryView),
    Notice(view::NoticeView),
    Activity(Vec<view::ActivityLine>),
    Tools(view::ToolsView),
    MemberHead((MemberSort, bool)),
    MemberRows(Vec<view::MemberRowView>),
    Selection(Option<view::SelectionView>),
    Positions(view::PositionsView),
    PositionEdit(view::PositionEditView),
    SkillHead(u32),
    Skills(Vec<view::SkillCardView>),
    Relations(view::RelationsView),
    Footer(view::FooterView),
    Toast(view::ToastView),
    Invite(view::InviteView),
    Welcome(view::WelcomeView),
}

impl Content {
    fn spawn(self, commands: &mut Commands) -> Entity {
        match self {
            Content::Title(line) => commands.spawn_scene(scene::title(line)).id(),
            Content::Tabs(view) => commands.spawn_scene(scene::tabs(view)).id(),
            Content::Page(view) => commands.spawn_scene(scene::page(view)).id(),
            Content::Summary(view) => commands.spawn_scene(scene::summary(view)).id(),
            Content::Notice(view) => commands.spawn_scene(scene::notice(view)).id(),
            Content::Activity(view) => commands.spawn_scene(scene::activity(view)).id(),
            Content::Tools(view) => commands.spawn_scene(scene::tools(view)).id(),
            Content::MemberHead(view) => commands.spawn_scene(scene::member_head(view)).id(),
            Content::MemberRows(view) => commands.spawn_scene(scene::member_rows(view)).id(),
            Content::Selection(view) => commands.spawn_scene(scene::selection(view)).id(),
            Content::Positions(view) => commands.spawn_scene(scene::positions(view)).id(),
            Content::PositionEdit(view) => commands.spawn_scene(scene::position_edit(view)).id(),
            Content::SkillHead(points) => commands.spawn_scene(scene::skill_head(points)).id(),
            Content::Skills(view) => commands.spawn_scene(scene::skills(view)).id(),
            Content::Relations(view) => commands.spawn_scene(scene::relations(view)).id(),
            Content::Footer(view) => commands.spawn_scene(scene::footer(view)).id(),
            Content::Toast(view) => commands.spawn_scene(scene::toast(view)).id(),
            Content::Invite(view) => commands.spawn_scene(invite_dialog::dialog(view)).id(),
            Content::Welcome(view) => commands.spawn_scene(invite_dialog::welcome(view)).id(),
        }
    }
}

/// The world as the guild UI reads it.
#[derive(SystemParam)]
pub struct GuildData<'w, 's> {
    guild: Res<'w, GuildState>,
    session: Res<'w, ZoneSession>,
    jobs: Option<Res<'w, JobSpriteRegistry>>,
    skills: Option<Res<'w, SkillCatalog>>,
    ui: Res<'w, GuildUi>,
    pending: Res<'w, PendingGuildInvite>,
    window: Query<'w, 's, &'static Visibility, With<GuildWindowRoot>>,
    search: Query<'w, 's, &'static EditableText, With<MemberSearchField>>,
}

impl GuildData<'_, '_> {
    /// The player's guild at `now`, while they are in one.
    fn guild(&self, now: u64) -> Option<Guild<'_>> {
        Some(Guild {
            state: &self.guild,
            info: self.guild.info()?,
            me: self.session.char_id,
            jobs: self.jobs.as_deref(),
            skills: self.skills.as_deref(),
            now,
        })
    }

    /// What `host` shows. The window's hosts are empty while it is hidden, and the
    /// invitation's while none is pending and none was just accepted.
    fn content(&self, host: GuildHost, now: u64) -> Option<Content> {
        let ui = &*self.ui;
        if host == GuildHost::Invite {
            if let Some(view) = self.pending.view() {
                return Some(Content::Invite(view.clone()));
            }
            let guild = self.guild(now)?;
            return (self.pending.joined() == guild.info.guild_id)
                .then(|| Content::Welcome(view::welcome(guild)));
        }
        let open = self
            .window
            .single()
            .is_ok_and(|visibility| *visibility != Visibility::Hidden);
        let guild = self.guild(now).filter(|_| open)?;
        Some(match host {
            GuildHost::Title => Content::Title(view::title(guild)),
            GuildHost::Tabs => Content::Tabs(view::tabs(guild, ui)),
            GuildHost::Page => Content::Page(view::page(guild, ui)),
            GuildHost::Summary => Content::Summary(view::summary(guild)),
            GuildHost::Notice => Content::Notice(view::notice(guild, ui)),
            GuildHost::Activity => Content::Activity(view::activity(guild)),
            GuildHost::Tools => Content::Tools(view::tools(guild, ui)),
            GuildHost::MemberHead => Content::MemberHead((ui.sort, ui.sort_reversed)),
            GuildHost::MemberRows => {
                let query = self
                    .search
                    .single()
                    .map(|field| field.value().to_string())
                    .unwrap_or_default();
                Content::MemberRows(view::member_rows(guild, ui, &query))
            }
            GuildHost::Selection => Content::Selection(view::selection(guild, ui)),
            GuildHost::Positions => Content::Positions(view::positions(guild, ui)),
            GuildHost::PositionEdit => Content::PositionEdit(view::position_edit(guild, ui)?),
            GuildHost::SkillHead => Content::SkillHead(guild.info.skill_points),
            GuildHost::Skills => Content::Skills(view::skills(guild).cards),
            GuildHost::Relations => Content::Relations(view::relations(guild)),
            GuildHost::Footer => Content::Footer(view::footer(guild, ui)),
            GuildHost::Toast => Content::Toast(view::toast(ui)?),
            GuildHost::Invite => return None,
        })
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Refills every [`GuildHost`] whose view changed.
fn rebuild_guild_ui(
    mut commands: Commands,
    data: GuildData,
    hosts: Query<(Entity, Ref<GuildHost>, Option<&Children>)>,
    mut shown: Local<[Option<Content>; HOSTS]>,
) {
    let now = unix_now();
    // Once the page is replaced its old hosts are on their way out with it; the new
    // page's hosts are filled next frame.
    let mut page_replaced = false;
    for (host, kind, children) in &hosts {
        if page_replaced && kind.in_page() {
            continue;
        }
        let content = data.content(*kind, now);
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
        page_replaced |= *kind == GuildHost::Page;
    }
}

/// The commands a guild button can send.
#[derive(SystemParam)]
pub struct GuildCommands<'w> {
    invite: MessageWriter<'w, GuildInviteRequested>,
    respond: MessageWriter<'w, GuildInviteResponded>,
    leave: MessageWriter<'w, GuildLeaveRequested>,
    expel: MessageWriter<'w, GuildExpelRequested>,
    position: MessageWriter<'w, GuildPositionEditRequested>,
    assign: MessageWriter<'w, GuildMemberPositionRequested>,
    notice: MessageWriter<'w, GuildNoticeEditRequested>,
    skill: MessageWriter<'w, GuildSkillUpRequested>,
    alliance: MessageWriter<'w, GuildAllianceRequested>,
    antagonist_remove: MessageWriter<'w, GuildAntagonistRemoveRequested>,
    party: MessageWriter<'w, PartyInviteRequested>,
}

/// The fields a guild button reads on submit. Each is on show only on its own tab.
#[derive(SystemParam)]
pub struct GuildFields<'w, 's> {
    texts: Query<'w, 's, &'static mut EditableText>,
    invite: Query<'w, 's, Entity, With<GuildInviteNameField>>,
    reason: Query<'w, 's, Entity, With<ExpelReasonField>>,
    subject: Query<'w, 's, Entity, With<NoticeSubjectField>>,
    body: Query<'w, 's, Entity, With<NoticeBodyField>>,
    position_name: Query<'w, 's, Entity, With<PositionNameField>>,
    position_tax: Query<'w, 's, Entity, With<PositionTaxField>>,
    alliance: Query<'w, 's, Entity, With<GuildAllianceNameField>>,
    antagonist: Query<'w, 's, Entity, With<GuildAntagonistNameField>>,
}

impl GuildFields<'_, '_> {
    /// The trimmed text of `field`; empty while the field is not on show.
    fn read(&self, field: Option<Entity>) -> String {
        field
            .and_then(|field| self.texts.get(field).ok())
            .map(|text| text.value().to_string().trim().to_string())
            .unwrap_or_default()
    }

    fn clear(&mut self, field: Option<Entity>) {
        if let Some(mut text) = field.and_then(|field| self.texts.get_mut(field).ok()) {
            set_field_text(&mut text, "");
        }
    }
}

/// Everything a guild button reads or changes.
#[derive(SystemParam)]
pub struct GuildActions<'w, 's> {
    context: GuildMutationContext<'w>,
    pending: ResMut<'w, PendingGuildInvite>,
    confirmation: ResMut<'w, relation_dialogs::PendingRelationConfirmation>,
    emblem: ResMut<'w, emblem::GuildEmblemPreview>,
    chat: Option<ResMut<'w, ChatUi>>,
    window: Query<'w, 's, &'static mut Visibility, With<GuildWindowRoot>>,
    fields: GuildFields<'w, 's>,
    out: GuildCommands<'w>,
}

impl GuildActions<'_, '_> {
    fn run(&mut self, action: GuildAction) {
        match action {
            GuildAction::CloseWindow => self.show(Visibility::Hidden),
            GuildAction::Tab(tab) => {
                let ui = &mut self.context.ui;
                ui.selected_tab = tab;
                ui.ask = None;
                ui.position_menu = false;
                ui.editing_notice = false;
            }
            GuildAction::PickEmblem => emblem::pick_emblem(
                &self.context.guild,
                &self.context.session,
                *self.context.generation,
                &mut self.emblem,
            ),
            GuildAction::EditNotice => self.context.ui.editing_notice = true,
            GuildAction::CancelNotice => self.context.ui.editing_notice = false,
            GuildAction::SaveNotice => self.save_notice(),
            GuildAction::ToggleOnline => self.context.ui.online_only ^= true,
            GuildAction::Sort(by) => {
                let ui = &mut self.context.ui;
                ui.sort_reversed = ui.sort == by && !ui.sort_reversed;
                ui.sort = by;
            }
            GuildAction::Select(char_id) => {
                let ui = &mut self.context.ui;
                ui.selected_member = Some(char_id);
                ui.ask = None;
                ui.position_menu = false;
            }
            GuildAction::Invite => self.invite(),
            GuildAction::Whisper(char_id) => self.whisper(char_id),
            GuildAction::PartyInvite(char_id) => self.party_invite(char_id),
            GuildAction::TogglePositionMenu => self.context.ui.position_menu ^= true,
            GuildAction::Assign(char_id, index) => self.assign(char_id, index),
            GuildAction::AskExpel => {
                self.context.ui.ask = Some(Ask::Expel);
                self.context.ui.position_menu = false;
            }
            GuildAction::KeepMember | GuildAction::Stay => self.context.ui.ask = None,
            GuildAction::Expel(char_id) => self.expel(char_id),
            GuildAction::SelectPosition(index) => {
                let ui = &mut self.context.ui;
                ui.selected_position = (ui.selected_position != Some(index)).then_some(index);
            }
            GuildAction::TogglePerm(index, perm) => self.toggle_perm(index, perm),
            GuildAction::SavePosition(index) => self.save_position(index),
            GuildAction::SkillUp(skill_id) => self.skill_up(skill_id),
            GuildAction::ProposeAlliance => self.propose_alliance(),
            GuildAction::DeclareAntagonist => self.declare_antagonist(),
            GuildAction::BreakAlliance(guild_id) => self.break_alliance(guild_id),
            GuildAction::RemoveAntagonist(guild_id) => self.remove_antagonist(guild_id),
            GuildAction::AskLeave => self.context.ui.ask = Some(Ask::Leave),
            GuildAction::Leave => self.leave(),
            GuildAction::Storage => self.context.ui.say("Guild storage opens at a Kafra"),
            GuildAction::AnswerInvite(accept) => self.answer_invite(accept),
            GuildAction::OpenGuild => {
                self.pending.clear();
                self.show(Visibility::Visible);
            }
            GuildAction::DismissInvite => self.pending.clear(),
        }
    }

    fn show(&mut self, shown: Visibility) {
        if let Ok(mut visibility) = self.window.single_mut() {
            *visibility = shown;
        }
    }

    fn me(&self) -> u32 {
        self.context.session.char_id
    }

    /// Books `action` as the request in flight, or says another one still is.
    fn begin(&mut self, action: &'static str, progress: &str) -> bool {
        let generation = *self.context.generation;
        self.context.ui.begin(action, generation, progress)
    }

    /// The name of a member, for the toast that names them.
    fn member_name(&self, char_id: u32) -> Option<String> {
        Some(self.context.guild.member(char_id)?.name.clone())
    }

    /// A position the local player may edit: any but the master's own, as the master.
    fn editable_position(&self, index: u32) -> Option<GuildPositionInfo> {
        let guild = &self.context.guild;
        let master = guild.info()?.master_char_id;
        if !guild.is_master(self.me()) || guild.position_of(master)?.index == index {
            return None;
        }
        guild.position(index).cloned()
    }

    fn invite(&mut self) {
        let field = self.fields.invite.single().ok();
        let name = self.fields.read(field);
        let generation = *self.context.generation;
        let Some(command) = request_invite(&mut self.context.ui, generation, 0, &name) else {
            return;
        };
        self.out.invite.write(command);
        self.fields.clear(field);
    }

    /// Aims the chat input at a whisper to the member.
    fn whisper(&mut self, char_id: u32) {
        let Some(name) = self.member_name(char_id) else {
            return;
        };
        if let Some(chat) = self.chat.as_mut() {
            chat.aim_whisper(&name);
        }
        self.context.ui.say(format!("Chat set to whisper {name}"));
    }

    fn party_invite(&mut self, char_id: u32) {
        let Some(name) = self.member_name(char_id) else {
            return;
        };
        self.out.party.write(PartyInviteRequested {
            target_char_id: char_id,
            target_name: String::new(),
        });
        self.context.ui.say(format!("Party invite sent to {name}."));
    }

    fn assign(&mut self, target: u32, index: u32) {
        let guild = &self.context.guild;
        let valid = !guild.is_master(target)
            && guild.member(target).is_some()
            && self.editable_position(index).is_some();
        if !valid {
            self.context
                .ui
                .fail("This member cannot be assigned to that position.");
            return;
        }
        if !self.begin("member_position", "Assigning position\u{2026}") {
            return;
        }
        self.out.assign.write(GuildMemberPositionRequested {
            target_char_id: target,
            index,
        });
        self.context.ui.position_menu = false;
    }

    fn expel(&mut self, target: u32) {
        let guild = &self.context.guild;
        let me = self.me();
        if target == me
            || guild.is_master(target)
            || guild.member(target).is_none()
            || !guild.can_expel(me)
        {
            return;
        }
        let reason = self.fields.read(self.fields.reason.single().ok());
        if reason.is_empty() {
            self.context
                .ui
                .fail("Enter a reason before expelling a member.");
            return;
        }
        if !self.begin("expel", "Expelling guild member\u{2026}") {
            return;
        }
        self.out.expel.write(GuildExpelRequested {
            target_char_id: target,
            reason,
        });
        self.context.ui.ask = None;
        self.context.ui.selected_member = None;
    }

    fn leave(&mut self) {
        if !self.context.guild.in_guild() || !self.begin("leave", "Leaving guild\u{2026}") {
            return;
        }
        self.out.leave.write(GuildLeaveRequested);
        self.context.ui.ask = None;
    }

    fn save_notice(&mut self) {
        if !self.context.guild.can_notice(self.me()) {
            return;
        }
        let subject = self.fields.read(self.fields.subject.single().ok());
        let body = self.fields.read(self.fields.body.single().ok());
        if !self.begin("notice_edit", "Saving notice\u{2026}") {
            return;
        }
        self.out
            .notice
            .write(GuildNoticeEditRequested { subject, body });
        self.context.ui.editing_notice = false;
    }

    /// Flips one permission of a position, leaving the rest as the server has them.
    fn toggle_perm(&mut self, index: u32, perm: Perm) {
        let Some(position) = self.editable_position(index) else {
            return;
        };
        if position.name.trim().is_empty() {
            self.context
                .ui
                .fail("Name the position before changing what it can do.");
            return;
        }
        if !self.begin("position_edit", "Saving position\u{2026}") {
            return;
        }
        let flipped = !perm.of(&position);
        let mut command = GuildPositionEditRequested {
            index,
            name: position.name,
            can_invite: position.can_invite,
            can_expel: position.can_expel,
            tax: None,
            can_storage: None,
            can_notice: None,
            can_relations: None,
        };
        match perm {
            Perm::Invite => command.can_invite = flipped,
            Perm::Expel => command.can_expel = flipped,
            Perm::Storage => command.can_storage = Some(flipped),
            Perm::Notice => command.can_notice = Some(flipped),
            Perm::Relations => command.can_relations = Some(flipped),
        }
        self.out.position.write(command);
    }

    /// Renames a position and sets its tax from the edit bar.
    fn save_position(&mut self, index: u32) {
        let Some(position) = self.editable_position(index) else {
            return;
        };
        let name = self.fields.read(self.fields.position_name.single().ok());
        let Some(name) = submit_name(&name) else {
            self.context.ui.fail("Enter a position name.");
            return;
        };
        let tax = self.fields.read(self.fields.position_tax.single().ok());
        let Some(tax) = parse_tax(&tax) else {
            self.context
                .ui
                .fail("EXP tax must be a whole percentage from 0 to 100.");
            return;
        };
        if !self.begin("position_edit", "Saving position\u{2026}") {
            return;
        }
        self.out.position.write(GuildPositionEditRequested {
            index,
            name,
            can_invite: position.can_invite,
            can_expel: position.can_expel,
            tax: (tax != position.tax).then_some(tax),
            can_storage: None,
            can_notice: None,
            can_relations: None,
        });
    }

    fn skill_up(&mut self, skill_id: u32) {
        let guild = &self.context.guild;
        let spendable = guild.is_master(self.me())
            && guild.info().is_some_and(|info| {
                info.skill_points > 0
                    && info
                        .skills
                        .iter()
                        .any(|skill| skill.skill_id == skill_id && skill.level < skill.max_level)
            });
        if !spendable || !self.begin("skill_up", "Upgrading guild skill\u{2026}") {
            return;
        }
        self.out.skill.write(GuildSkillUpRequested { skill_id });
    }

    /// Whether the guild has a relation of `kind` with `guild_id`; its name if so.
    fn related(&self, guild_id: u32, kind: GuildRelationKind) -> Option<String> {
        if !self.context.guild.can_relations(self.me()) {
            return None;
        }
        self.context
            .guild
            .info()?
            .relations
            .iter()
            .find(|relation| relation.guild_id == guild_id && relation.kind == kind)
            .map(|relation| relation.name.clone())
    }

    fn propose_alliance(&mut self) {
        if !self.context.guild.can_relations(self.me()) {
            return;
        }
        let field = self.fields.alliance.single().ok();
        let Some(target_name) = submit_name(&self.fields.read(field)) else {
            self.context
                .ui
                .fail("Enter an online guild master\u{2019}s character name.");
            return;
        };
        if !self.begin("alliance_request", "Sending alliance request\u{2026}") {
            return;
        }
        self.out.alliance.write(GuildAllianceRequested {
            target_char_id: 0,
            target_name,
        });
        self.fields.clear(field);
    }

    /// Asks to confirm, since declaring an antagonist cannot be taken back quietly.
    fn declare_antagonist(&mut self) {
        let guild = &self.context.guild;
        let Some(own_guild_id) = guild.info().map(|info| info.guild_id) else {
            return;
        };
        if !guild.can_relations(self.me()) {
            return;
        }
        let field = self.fields.antagonist.single().ok();
        let Some(target_name) = submit_name(&self.fields.read(field)) else {
            self.context
                .ui
                .fail("Enter a character name from the target guild.");
            return;
        };
        if self.context.ui.pending.is_some() || self.confirmation.is_pending() {
            self.context.ui.fail("A guild action is already pending.");
            return;
        }
        self.confirmation
            .declare_antagonist(*self.context.generation, own_guild_id, target_name);
        self.fields.clear(field);
    }

    /// Asks to confirm before an alliance ends.
    fn break_alliance(&mut self, guild_id: u32) {
        let Some(name) = self.related(guild_id, GuildRelationKind::Ally) else {
            return;
        };
        let Some(own_guild_id) = self.context.guild.info().map(|info| info.guild_id) else {
            return;
        };
        if self.context.ui.pending.is_some() || self.confirmation.is_pending() {
            return;
        }
        self.confirmation
            .break_alliance(*self.context.generation, own_guild_id, guild_id, name);
    }

    fn remove_antagonist(&mut self, guild_id: u32) {
        if self
            .related(guild_id, GuildRelationKind::Antagonist)
            .is_none()
            || !self.begin("antagonist_remove", "Removing antagonist\u{2026}")
        {
            return;
        }
        self.out
            .antagonist_remove
            .write(GuildAntagonistRemoveRequested { guild_id });
    }

    fn answer_invite(&mut self, accept: bool) {
        if !self.pending.is_pending() {
            return;
        }
        self.out.respond.write(GuildInviteResponded {
            guild_id: self.pending.guild_id,
            accept,
        });
        if accept {
            self.pending.accept();
        } else {
            self.pending.clear();
        }
    }
}

/// Runs the [`GuildAction`] of an activated button. The headless button also activates
/// on a right click, which is ignored.
pub fn on_guild_button(
    activate: On<Activate>,
    actions: Query<&GuildAction>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut guild: GuildActions,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(action) = actions.get(activate.entity) {
        guild.run(*action);
    }
}

/// A right click on a skill card opens the info modal for that guild skill.
pub fn on_skill_cell_click(
    click: On<PointerClick>,
    cells: Query<&SkillCell>,
    guild: Res<GuildState>,
    catalog: Option<Res<SkillCatalog>>,
    mut writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Secondary {
        return;
    }
    let Some(skill) = guild
        .info()
        .and_then(|info| info.skills.iter().find(|skill| skill.skill_id == cell.0))
    else {
        warn!(
            "guild: skill #{} not in the guild snapshot, ignoring inspect",
            cell.0
        );
        return;
    };
    writer.write(ShowInfoModal {
        content: InfoContent::Skill {
            view: info_view::guild_skill_view(skill, catalog.as_deref()),
            raise: None,
        },
    });
}

#[cfg(test)]
mod tests;
