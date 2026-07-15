mod dialogs;
pub(crate) mod emblem;
mod members;
mod notice;
mod positions;
pub mod scene;

pub(crate) use members::request_invite;

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::guild::{GuildState, GuildSystems};
use game_engine::domain::input::{PlayerAction, UiFocus};
use game_engine::infrastructure::job::JobSpriteRegistry;
use leafwing_input_manager::prelude::ActionState;
use net_contract::commands::GuildCreateRequested;
use net_contract::dto::GuildErrorKind;
use net_contract::events::{GuildIngress, GuildIngressPayload, ZoneDisconnected};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::theme::feathers_theme::install_norse_theme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuildTab {
    #[default]
    Members,
    Positions,
    Notice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGuildMutation {
    pub action: &'static str,
    pub generation: ZoneSessionGeneration,
}

#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct GuildUi {
    pub selected_tab: GuildTab,
    pub create_name: String,
    pub feedback: Option<String>,
    pub pending: Option<PendingGuildMutation>,
    feedback_is_error: bool,
}

#[derive(Resource, Default)]
pub(crate) struct GuildUiSession {
    generation: ZoneSessionGeneration,
    char_id: u32,
    blocked: bool,
    reset: bool,
}

fn request_create(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    raw_name: &str,
) -> Option<GuildCreateRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    let name = raw_name.trim();
    if name.is_empty() {
        ui.feedback = Some("Enter a guild name.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    ui.create_name = name.to_string();
    ui.feedback = Some("Creating guild…".to_string());
    ui.feedback_is_error = false;
    ui.pending = Some(PendingGuildMutation {
        action: "create",
        generation,
    });
    Some(GuildCreateRequested {
        name: name.to_string(),
    })
}

#[derive(Component, Default, Clone)]
pub struct GuildWindowRoot;
#[derive(Component, Default, Clone)]
pub struct GuildTitlebar;
#[derive(Component, Default, Clone)]
pub struct GuildUnguildedPanel;
#[derive(Component, Default, Clone)]
pub struct GuildGuildedPanel;
#[derive(Component, Default, Clone)]
pub struct GuildMembersPanel;
#[derive(Component, Default, Clone)]
pub struct GuildPositionsPanel;
#[derive(Component, Default, Clone)]
pub struct GuildNoticePanel;
#[derive(Component, Default, Clone)]
pub struct GuildPositionsList;
#[derive(Component, Default, Clone)]
pub struct GuildNoticeContent;
#[derive(Component, Default, Clone)]
pub struct GuildMembersList;
#[derive(Component, Default, Clone)]
pub struct GuildInviteControls;
#[derive(Component, Default, Clone)]
pub struct GuildInviteNameField;
#[derive(Component, Default, Clone)]
pub struct GuildInviteButton;
#[derive(Component, Default, Clone)]
pub struct GuildCreateNameField;
#[derive(Component, Default, Clone)]
pub struct GuildCreateButton;
#[derive(Component, Default, Clone)]
pub struct GuildFeedbackText;
#[derive(Component, Default, Clone)]
pub struct GuildNameText;
#[derive(Component, Default, Clone)]
pub struct GuildMasterText;
#[derive(Component, Default, Clone)]
pub struct GuildNoticeText;
#[derive(Component, Default, Clone)]
pub struct GuildMemberCountText;
#[derive(Component, Default, Clone)]
pub struct GuildOnlineCountText;
#[derive(Component, Default, Clone)]
pub struct GuildHeaderEmblemImage;
#[derive(Component, Default, Clone)]
pub struct GuildHeaderEmblemFallback;
#[derive(Component, Default, Clone)]
pub struct GuildEmblemUploadButton;
#[derive(Component, Default, Clone)]
pub struct MembersTabButton;
#[derive(Component, Default, Clone)]
pub struct PositionsTabButton;
#[derive(Component, Default, Clone)]
pub struct NoticeTabButton;
#[derive(Component, Default, Clone)]
pub struct GuildMutationControl;
#[derive(Component, Default, Clone)]
pub struct GuildLeaveButton;

#[derive(SystemParam)]
pub(crate) struct GuildMutationContext<'w> {
    pub guild: Res<'w, GuildState>,
    pub session: Res<'w, ZoneSession>,
    pub generation: Res<'w, ZoneSessionGeneration>,
    pub ui: ResMut<'w, GuildUi>,
}

type GuildTextFieldFilter = Or<(
    With<GuildCreateNameField>,
    With<GuildInviteNameField>,
    With<positions::PositionNameField>,
    With<notice::GuildNoticeSubjectField>,
    With<notice::GuildNoticeBodyField>,
    With<members::GuildExpelReasonField>,
)>;
type GuildTextFields<'w, 's> = Query<'w, 's, Entity, GuildTextFieldFilter>;
type GuildEditableTextFields<'w, 's> =
    Query<'w, 's, &'static mut EditableText, GuildTextFieldFilter>;
type GuildModePanel<'w, 's, F> = Query<'w, 's, (&'static mut Visibility, &'static mut Node), F>;

pub struct GuildWindowPlugin;

impl Plugin for GuildWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<GuildUi>()
            .init_resource::<GuildUiSession>()
            .init_resource::<emblem::GuildEmblemImages>()
            .init_resource::<dialogs::PendingGuildInvite>()
            .init_resource::<dialogs::PendingGuildConfirmation>()
            .add_systems(
                Update,
                (
                    reset_guild_ui_session,
                    dialogs::reset_stale_invite,
                    dialogs::reset_stale_confirmation,
                    emblem::reset_emblems,
                )
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
                        sync_create_draft,
                        apply_guild_results,
                        sync_membership_mode,
                        sync_header,
                        emblem::invalidate_picker_when_hidden,
                        emblem::poll_picker,
                        emblem::receive_emblem_data,
                        emblem::queue_current_guild_emblem,
                        emblem::send_next_fetch,
                        emblem::sync_header_emblem,
                        sync_emblem_upload_control,
                    )
                        .chain(),
                    (
                        sync_tabs,
                        sync_feedback,
                        sync_invite_controls,
                        sync_expel_controls,
                        refresh_members,
                        positions::refresh_positions,
                        positions::sync_invite_labels,
                        positions::sync_expel_labels,
                        notice::refresh_notice,
                        sync_management_controls,
                        release_hidden_guild_focus,
                    )
                        .chain(),
                )
                    .chain()
                    .in_set(GuildSystems::UiSync)
                    .run_if(in_state(GameState::InGame)),
            );
        app.add_systems(
            Update,
            (
                dialogs::queue_incoming_invite,
                dialogs::claim_invite_choice,
                dialogs::claim_confirmation_choice,
            )
                .in_set(GuildSystems::UiSync)
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            PostUpdate,
            (
                dialogs::expire_pending_invite,
                dialogs::show_pending_invite,
                dialogs::close_finished_invite_dialog,
                dialogs::show_pending_confirmation,
            )
                .chain(),
        );
        app.add_systems(
            OnExit(GameState::InGame),
            (
                clear_guild_focus_on_exit,
                block_guild_ui_on_exit,
                dialogs::clear_pending_invite,
                dialogs::clear_pending_confirmation,
                emblem::clear_emblems_on_exit,
            ),
        );
    }
}

fn reset_guild_ui_session(
    generation: Res<ZoneSessionGeneration>,
    zone_session: Option<Res<ZoneSession>>,
    mut disconnected: Option<MessageReader<ZoneDisconnected>>,
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
    mut fields: GuildEditableTextFields,
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
    for mut visibility in &mut roots {
        *visibility = Visibility::Hidden;
    }
    for mut field in &mut fields {
        field.clear();
    }
}

fn block_guild_ui_on_exit(
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
    mut fields: GuildEditableTextFields,
) {
    session.blocked = true;
    *ui = GuildUi::default();
    for mut visibility in &mut roots {
        *visibility = Visibility::Hidden;
    }
    for mut field in &mut fields {
        field.clear();
    }
}

fn sync_emblem_upload_control(
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    mut controls: Query<&mut Visibility, With<GuildEmblemUploadButton>>,
) {
    let visible = guild.is_master(session.char_id);
    for mut visibility in &mut controls {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn sync_management_controls(
    ui: Res<GuildUi>,
    confirmation: Option<Res<dialogs::PendingGuildConfirmation>>,
    controls: Query<Entity, With<GuildMutationControl>>,
    mut commands: Commands,
) {
    for control in &controls {
        if ui.pending.is_some()
            || confirmation
                .as_deref()
                .is_some_and(|pending| pending.is_pending())
        {
            commands.entity(control).insert(InteractionDisabled);
        } else {
            commands.entity(control).remove::<InteractionDisabled>();
        }
    }
}

fn toggle_guild_window(
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
        if ui_focus.text_input_active {
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

pub(crate) fn on_create(
    _: On<Activate>,
    field: Query<&EditableText, With<GuildCreateNameField>>,
    generation: Res<ZoneSessionGeneration>,
    mut ui: ResMut<GuildUi>,
    mut writer: MessageWriter<GuildCreateRequested>,
) {
    let Ok(field) = field.single() else {
        return;
    };
    if let Some(command) = request_create(&mut ui, *generation, &field.value().to_string()) {
        writer.write(command);
    }
}

fn sync_create_draft(
    field: Query<&EditableText, (With<GuildCreateNameField>, Changed<EditableText>)>,
    mut ui: ResMut<GuildUi>,
) {
    let Ok(field) = field.single() else {
        return;
    };
    let value = field.value().to_string();
    if ui.create_name != value {
        ui.create_name = value;
    }
}

pub(crate) fn select_members(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Members;
}

pub(crate) fn select_positions(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Positions;
}

pub(crate) fn select_notice(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Notice;
}

fn apply_guild_results(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    session: Option<Res<GuildUiSession>>,
    mut ui: ResMut<GuildUi>,
    mut images: ResMut<emblem::GuildEmblemImages>,
    mut assets: ResMut<Assets<Image>>,
) {
    if session.as_deref().is_some_and(|session| session.blocked) {
        ingress.clear();
        return;
    }
    for event in ingress.read() {
        let GuildIngressPayload::ActionResult(result) = &event.payload else {
            continue;
        };
        let matches = ui.pending.as_ref().is_some_and(|pending| {
            pending.action == result.action
                && pending.generation == event.generation
                && event.generation == *generation
        });
        if !matches {
            if let Some(pending) = ui.pending.as_ref() {
                if pending.action != result.action {
                    warn!(
                        expected = pending.action,
                        received = %result.action,
                        "ignoring mismatched guild action result"
                    );
                }
            }
            continue;
        }
        ui.pending = None;
        if result.success {
            ui.feedback = Some(match result.action.as_str() {
                "create" => "Guild created. Waiting for guild information…".to_string(),
                "invite" => "Guild invitation sent.".to_string(),
                "position_edit" => "Position saved. Waiting for guild information…".to_string(),
                "member_position" => {
                    "Position assignment sent. Waiting for guild information…".to_string()
                }
                "notice_edit" => "Notice saved. Waiting for guild information…".to_string(),
                "emblem_upload" => {
                    "Emblem uploaded. Waiting for the authoritative emblem update…".to_string()
                }
                "leave" => "Guild leave requested. Waiting for authoritative state…".to_string(),
                "expel" => "Guild member expelled. Waiting for roster refresh…".to_string(),
                _ => "Guild action completed.".to_string(),
            });
            ui.feedback_is_error = false;
        } else {
            if result.action == "emblem_upload" {
                images.discard_preview(&mut assets);
            }
            ui.feedback = Some(guild_error_text(result.error).to_string());
            ui.feedback_is_error = true;
        }
    }
}

fn guild_error_text(error: GuildErrorKind) -> &'static str {
    match error {
        GuildErrorKind::None => "Success",
        GuildErrorKind::NameTaken => "Guild name is already taken",
        GuildErrorKind::AlreadyInGuild => "Character already belongs to a guild",
        GuildErrorKind::GuildFull => "Guild is full",
        GuildErrorKind::NoPermission => "Current position lacks permission",
        GuildErrorKind::NotMember => "Character is not a guild member",
        GuildErrorKind::TargetOffline => "Target is offline",
        GuildErrorKind::NoEmperium => "Creation requires an Emperium",
        GuildErrorKind::InvalidEmblem => "Emblem is invalid",
        GuildErrorKind::CannotTargetMaster => "Guild master cannot be expelled",
        GuildErrorKind::InvalidPosition => "Position is invalid",
        GuildErrorKind::Unknown(value) => {
            warn!(value, "unknown guild operation error");
            "Guild operation failed"
        }
    }
}

fn sync_membership_mode(
    guild: Res<GuildState>,
    mut create: GuildModePanel<(
        With<GuildUnguildedPanel>,
        Without<GuildGuildedPanel>,
        Without<GuildWindowRoot>,
    )>,
    mut guilded: GuildModePanel<(
        With<GuildGuildedPanel>,
        Without<GuildUnguildedPanel>,
        Without<GuildWindowRoot>,
    )>,
    mut root: Query<&mut Node, With<GuildWindowRoot>>,
) {
    let Ok((mut create_visibility, mut create_node)) = create.single_mut() else {
        return;
    };
    let Ok((mut guilded_visibility, mut guilded_node)) = guilded.single_mut() else {
        return;
    };
    let Ok(mut root) = root.single_mut() else {
        return;
    };
    let in_guild = guild.in_guild();
    if in_guild {
        *create_visibility = Visibility::Hidden;
        *guilded_visibility = Visibility::Inherited;
        create_node.display = Display::None;
        guilded_node.display = Display::Flex;
    } else {
        *create_visibility = Visibility::Inherited;
        *guilded_visibility = Visibility::Hidden;
        create_node.display = Display::Flex;
        guilded_node.display = Display::None;
    }
    root.width = px(if in_guild {
        scene::GUILD_WINDOW_WIDTH
    } else {
        scene::CREATE_MODAL_WIDTH
    });
}

#[allow(clippy::type_complexity)]
fn sync_header(
    guild: Res<GuildState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<GuildNameText>>,
        Query<&mut Text, With<GuildMasterText>>,
        Query<&mut Text, With<GuildNoticeText>>,
        Query<&mut Text, With<GuildMemberCountText>>,
        Query<&mut Text, With<GuildOnlineCountText>>,
    )>,
) {
    let Some(info) = guild.info() else {
        return;
    };
    let master = info
        .members
        .iter()
        .find(|member| member.char_id == info.master_char_id)
        .map(|member| member.name.as_str())
        .unwrap_or("Unknown");
    set_single_text(&mut texts.p0(), info.name.clone());
    set_single_text(&mut texts.p1(), format!("Guild Master {master}"));
    let notice = if info.notice_subject.is_empty() {
        "No guild notice".to_string()
    } else {
        info.notice_subject.clone()
    };
    set_single_text(&mut texts.p2(), notice);
    let member_label = if info.members.len() == 1 {
        "1 member".to_string()
    } else {
        format!("{} members", info.members.len())
    };
    set_single_text(&mut texts.p3(), member_label);
    let online = info.members.iter().filter(|member| member.online).count();
    set_single_text(&mut texts.p4(), format!("{online} online"));
}

fn set_single_text<F: bevy::ecs::query::QueryFilter>(
    query: &mut Query<&mut Text, F>,
    value: String,
) {
    if let Ok(mut text) = query.single_mut() {
        if text.0 != value {
            text.0 = value;
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_tabs(
    ui: Res<GuildUi>,
    mut members: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
            Without<GuildNoticePanel>,
        ),
    >,
    mut positions: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildPositionsPanel>,
            Without<GuildMembersPanel>,
            Without<GuildNoticePanel>,
        ),
    >,
    mut notice: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildNoticePanel>,
            Without<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
        ),
    >,
) {
    let Ok(mut members) = members.single_mut() else {
        return;
    };
    let Ok(mut positions) = positions.single_mut() else {
        return;
    };
    let Ok(mut notice) = notice.single_mut() else {
        return;
    };
    let members_active = ui.selected_tab == GuildTab::Members;
    *members.0 = if members_active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    members.1.display = if members_active {
        Display::Flex
    } else {
        Display::None
    };
    let positions_active = ui.selected_tab == GuildTab::Positions;
    *positions.0 = if positions_active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    positions.1.display = if positions_active {
        Display::Flex
    } else {
        Display::None
    };
    let notice_active = ui.selected_tab == GuildTab::Notice;
    *notice.0 = if notice_active {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    notice.1.display = if notice_active {
        Display::Flex
    } else {
        Display::None
    };
}

fn sync_feedback(
    ui: Res<GuildUi>,
    mut feedback: Query<(&mut Text, &mut TextColor, &mut Visibility), With<GuildFeedbackText>>,
    button: Query<Entity, With<GuildCreateButton>>,
    mut commands: Commands,
) {
    for (mut text, mut color, mut visibility) in &mut feedback {
        if let Some(message) = &ui.feedback {
            text.0.clone_from(message);
            color.0 = if ui.feedback_is_error {
                theme::BAD
            } else {
                theme::EMERALD_BRI
            };
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
    let Ok(button) = button.single() else {
        return;
    };
    if ui.pending.is_some() {
        commands.entity(button).insert(InteractionDisabled);
    } else {
        commands.entity(button).remove::<InteractionDisabled>();
    }
}

fn sync_invite_controls(
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    ui: Res<GuildUi>,
    mut controls: Query<&mut Visibility, With<GuildInviteControls>>,
    button: Query<Entity, With<GuildInviteButton>>,
    mut commands: Commands,
) {
    if let Ok(mut visibility) = controls.single_mut() {
        *visibility = if guild.can_invite(session.char_id) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let Ok(button) = button.single() else {
        return;
    };
    if ui.pending.is_some() {
        commands.entity(button).insert(InteractionDisabled);
    } else {
        commands.entity(button).remove::<InteractionDisabled>();
    }
}

fn sync_expel_controls(
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    mut controls: Query<(&members::GuildExpelControl, &mut Visibility)>,
) {
    let allowed = guild.can_expel(session.char_id);
    for (control, mut visibility) in &mut controls {
        *visibility = if allowed && control.0 != session.char_id && !guild.is_master(control.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn refresh_members(
    mut commands: Commands,
    guild: Res<GuildState>,
    jobs: Option<Res<JobSpriteRegistry>>,
    container: Query<(Entity, Option<&Children>), With<GuildMembersList>>,
) {
    let Ok((container, children)) = container.single() else {
        return;
    };
    let empty = children.is_none_or(|children| children.is_empty());
    let jobs_changed = jobs.as_ref().is_some_and(|jobs| jobs.is_changed());
    if !empty && !guild.is_changed() && !jobs_changed {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let rows = guild
        .info()
        .map(|info| members::project_rows(info, jobs.as_deref()))
        .unwrap_or_default();
    commands
        .spawn_scene(scene::member_rows(rows))
        .insert(ChildOf(container));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::window::PrimaryWindow;
    use net_contract::dto::{
        GuildActionResult, GuildErrorKind, GuildInfo, GuildMemberInfo, GuildPositionInfo,
    };
    use net_contract::events::ZoneDisconnected;
    fn toggle_app(visibility: Visibility) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.init_resource::<GuildState>();
        app.init_resource::<InputFocus>();
        app.init_resource::<UiFocus>();
        app.add_plugins(crate::focus::UiFocusMirrorPlugin);
        let field = app
            .world_mut()
            .spawn((GuildCreateNameField, EditableText::new("")))
            .id();
        app.world_mut().spawn((GuildWindowRoot, visibility));
        let player = app
            .world_mut()
            .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
            .id();
        app.add_systems(
            Update,
            toggle_guild_window.run_if(in_state(GameState::InGame)),
        );
        (app, field, player)
    }

    fn press_guild(app: &mut App, player: Entity) {
        app.world_mut()
            .entity_mut(player)
            .get_mut::<ActionState<PlayerAction>>()
            .unwrap()
            .press(&PlayerAction::Guild);
    }

    #[test]
    fn opening_unguilded_window_does_not_claim_text_focus() {
        let (mut app, _, player) = toggle_app(Visibility::Hidden);
        let primary_window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.insert_resource(InputFocus::from_entity(primary_window));
        app.update();
        assert!(!app.world().resource::<UiFocus>().text_input_active);

        press_guild(&mut app, player);
        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(primary_window)
        );
        let visibility = app
            .world_mut()
            .query_filtered::<&Visibility, With<GuildWindowRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*visibility, Visibility::Visible);
    }

    #[test]
    fn unrelated_active_text_input_focus_stays_closed_and_preserves_focus() {
        let (mut app, _, player) = toggle_app(Visibility::Hidden);
        let unrelated = app.world_mut().spawn(EditableText::new("other")).id();
        app.insert_resource(InputFocus::from_entity(unrelated));
        app.update();
        assert!(app.world().resource::<UiFocus>().text_input_active);

        press_guild(&mut app, player);
        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), Some(unrelated));
        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Hidden);
    }

    #[test]
    fn only_the_active_guild_tab_participates_in_layout() {
        let mut app = App::new();
        app.init_resource::<GuildUi>();
        app.world_mut()
            .spawn((GuildMembersPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildPositionsPanel, Node::default(), Visibility::Hidden));
        app.world_mut()
            .spawn((GuildNoticePanel, Node::default(), Visibility::Hidden));
        app.add_systems(Update, sync_tabs);

        app.update();
        assert_eq!(node::<GuildMembersPanel>(&mut app).display, Display::Flex);
        assert_eq!(node::<GuildPositionsPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildNoticePanel>(&mut app).display, Display::None);

        app.world_mut().resource_mut::<GuildUi>().selected_tab = GuildTab::Notice;
        app.update();
        assert_eq!(node::<GuildMembersPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildPositionsPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildNoticePanel>(&mut app).display, Display::Flex);
        assert_eq!(
            visibility::<GuildNoticePanel>(&mut app),
            Visibility::Inherited
        );
    }

    #[test]
    fn visible_with_guild_focus_closes_and_releases_focus() {
        let (mut app, field, player) = toggle_app(Visibility::Visible);
        app.insert_resource(InputFocus::from_entity(field));
        app.update();
        assert!(app.world().resource::<UiFocus>().text_input_active);

        press_guild(&mut app, player);
        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Hidden);
    }

    #[test]
    fn guilded_window_close_releases_invite_field_focus() {
        let (mut app, _, player) = toggle_app(Visibility::Visible);
        let invite = app
            .world_mut()
            .spawn((GuildInviteNameField, EditableText::new("Thor")))
            .id();
        app.insert_resource(InputFocus::from_entity(invite));
        app.update();

        press_guild(&mut app, player);
        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Hidden);
    }

    #[test]
    fn titlebar_close_releases_guild_owned_focus() {
        let mut app = App::new();
        app.init_resource::<InputFocus>();
        let field = app.world_mut().spawn(GuildCreateNameField).id();
        app.world_mut()
            .spawn((GuildWindowRoot, Visibility::Visible));
        let close = app
            .world_mut()
            .spawn_empty()
            .observe(crate::widgets::chrome::close_window::<GuildWindowRoot>)
            .id();
        app.insert_resource(InputFocus::from_entity(field));
        app.add_systems(Update, release_hidden_guild_focus);

        app.world_mut().trigger(Activate { entity: close });
        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn hidden_root_releases_only_guild_owned_focus() {
        let mut app = App::new();
        app.init_resource::<InputFocus>();
        let field = app.world_mut().spawn(GuildCreateNameField).id();
        let invite = app.world_mut().spawn(GuildInviteNameField).id();
        let other = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((GuildWindowRoot, Visibility::Hidden));
        app.add_systems(Update, release_hidden_guild_focus);

        app.insert_resource(InputFocus::from_entity(field));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);

        app.insert_resource(InputFocus::from_entity(invite));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);

        app.insert_resource(InputFocus::from_entity(other));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    }

    #[test]
    fn gameplay_teardown_releases_invite_field_focus() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_systems(OnExit(GameState::InGame), clear_guild_focus_on_exit);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        let invite = app.world_mut().spawn(GuildInviteNameField).id();
        app.insert_resource(InputFocus::from_entity(invite));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    fn authoritative_ui_app() -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(9))
            .add_plugins(game_engine::domain::guild::GuildPlugin);
        app.world_mut()
            .spawn((GuildUnguildedPanel, Visibility::Hidden, Node::default()));
        app.world_mut()
            .spawn((GuildGuildedPanel, Visibility::Inherited, Node::default()));
        app.world_mut().spawn((GuildWindowRoot, Node::default()));
        app.world_mut().spawn((GuildNameText, Text::default()));
        app.world_mut().spawn((GuildMasterText, Text::default()));
        app.world_mut().spawn((GuildNoticeText, Text::default()));
        app.world_mut()
            .spawn((GuildMemberCountText, Text::default()));
        app.world_mut()
            .spawn((GuildOnlineCountText, Text::default()));
        app.add_systems(
            Update,
            (sync_membership_mode, sync_header).in_set(GuildSystems::UiSync),
        );
        app
    }

    fn visibility<M: Component>(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<M>>()
            .single(app.world())
            .unwrap()
    }

    fn marked_text<M: Component>(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<M>>()
            .single(app.world())
            .unwrap()
            .0
            .clone()
    }

    #[test]
    fn authoritative_snapshot_switches_mode_and_projects_header() {
        let mut app = authoritative_ui_app();
        app.update();
        assert_eq!(
            visibility::<GuildUnguildedPanel>(&mut app),
            Visibility::Inherited
        );
        assert_eq!(
            visibility::<GuildGuildedPanel>(&mut app),
            Visibility::Hidden
        );
        assert_eq!(node::<GuildUnguildedPanel>(&mut app).display, Display::Flex);
        assert_eq!(node::<GuildGuildedPanel>(&mut app).display, Display::None);
        assert_eq!(
            node::<GuildWindowRoot>(&mut app).width,
            px(scene::CREATE_MODAL_WIDTH)
        );

        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(GuildInfo {
                guild_id: 7,
                name: "Vikings".into(),
                master_char_id: 42,
                emblem_id: 3,
                notice_subject: "Welcome".into(),
                notice_body: "Be kind".into(),
                positions: vec![GuildPositionInfo {
                    index: 0,
                    name: "Master".into(),
                    can_invite: true,
                    can_expel: true,
                    can_storage: false,
                    tax: 0,
                }],
                members: vec![GuildMemberInfo {
                    char_id: 42,
                    name: "Odin".into(),
                    job_id: 1,
                    base_level: 99,
                    online: false,
                    map: "prontera".into(),
                    position_index: 0,
                    hp: 90,
                    max_hp: 100,
                    sp: 40,
                    max_sp: 50,
                    ap: 8,
                    max_ap: 10,
                }],
            }),
        });
        app.update();

        assert_eq!(
            visibility::<GuildUnguildedPanel>(&mut app),
            Visibility::Hidden
        );
        assert_eq!(
            visibility::<GuildGuildedPanel>(&mut app),
            Visibility::Inherited
        );
        assert_eq!(node::<GuildUnguildedPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildGuildedPanel>(&mut app).display, Display::Flex);
        assert_eq!(
            node::<GuildWindowRoot>(&mut app).width,
            px(scene::GUILD_WINDOW_WIDTH)
        );
        assert_eq!(marked_text::<GuildNameText>(&mut app), "Vikings");
        assert_eq!(
            marked_text::<GuildMasterText>(&mut app),
            "Guild Master Odin"
        );
        assert_eq!(marked_text::<GuildNoticeText>(&mut app), "Welcome");
        assert_eq!(marked_text::<GuildMemberCountText>(&mut app), "1 member");
        assert_eq!(marked_text::<GuildOnlineCountText>(&mut app), "0 online");
    }

    fn node<M: Component>(app: &mut App) -> Node {
        app.world_mut()
            .query_filtered::<&Node, With<M>>()
            .single(app.world())
            .unwrap()
            .clone()
    }

    #[test]
    fn create_is_trimmed_and_serialized_until_matching_result() {
        let mut ui = GuildUi::default();
        let generation = ZoneSessionGeneration(3);

        let first = request_create(&mut ui, generation, "  Vikings  ").unwrap();
        assert_eq!(first.name, "Vikings");
        assert_eq!(
            ui.pending,
            Some(PendingGuildMutation {
                action: "create",
                generation,
            })
        );

        assert!(request_create(&mut ui, generation, "Other").is_none());
        assert_eq!(
            ui.feedback.as_deref(),
            Some("A guild action is already pending.")
        );
    }

    #[test]
    fn empty_create_is_rejected_inline() {
        let mut ui = GuildUi::default();

        assert!(request_create(&mut ui, ZoneSessionGeneration(1), "   ").is_none());
        assert!(ui.pending.is_none());
        assert_eq!(ui.feedback.as_deref(), Some("Enter a guild name."));
    }

    #[test]
    fn successful_create_result_clears_pending_without_fabricating_membership() {
        let generation = ZoneSessionGeneration(4);
        let mut app = App::new();
        app.add_message::<GuildIngress>();
        app.insert_resource(generation);
        app.insert_resource(GuildUi {
            pending: Some(PendingGuildMutation {
                action: "create",
                generation,
            }),
            ..default()
        });
        app.init_resource::<GuildState>();
        app.init_resource::<emblem::GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        app.add_systems(Update, apply_guild_results);
        app.world_mut()
            .resource_mut::<Messages<GuildIngress>>()
            .write(GuildIngress {
                generation,
                payload: GuildIngressPayload::ActionResult(GuildActionResult {
                    action: "create".into(),
                    success: true,
                    error: GuildErrorKind::None,
                }),
            });

        app.update();

        let ui = app.world().resource::<GuildUi>();
        assert!(ui.pending.is_none());
        assert_eq!(
            ui.feedback.as_deref(),
            Some("Guild created. Waiting for guild information…")
        );
        assert!(!app.world().resource::<GuildState>().in_guild());
    }

    #[test]
    fn mismatched_result_does_not_claim_pending_create() {
        let generation = ZoneSessionGeneration(4);
        let mut app = App::new();
        app.add_message::<GuildIngress>();
        app.insert_resource(generation);
        app.insert_resource(GuildUi {
            pending: Some(PendingGuildMutation {
                action: "create",
                generation,
            }),
            ..default()
        });
        app.init_resource::<emblem::GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        app.add_systems(Update, apply_guild_results);
        app.world_mut()
            .resource_mut::<Messages<GuildIngress>>()
            .write(GuildIngress {
                generation,
                payload: GuildIngressPayload::ActionResult(GuildActionResult {
                    action: "invite".into(),
                    success: false,
                    error: GuildErrorKind::NoPermission,
                }),
            });

        app.update();

        assert_eq!(
            app.world().resource::<GuildUi>().pending,
            Some(PendingGuildMutation {
                action: "create",
                generation,
            })
        );
    }

    #[test]
    fn guild_error_copy_maps_every_known_error_and_unknown_is_generic() {
        let expected = [
            (GuildErrorKind::None, "Success"),
            (GuildErrorKind::NameTaken, "Guild name is already taken"),
            (
                GuildErrorKind::AlreadyInGuild,
                "Character already belongs to a guild",
            ),
            (GuildErrorKind::GuildFull, "Guild is full"),
            (
                GuildErrorKind::NoPermission,
                "Current position lacks permission",
            ),
            (GuildErrorKind::NotMember, "Character is not a guild member"),
            (GuildErrorKind::TargetOffline, "Target is offline"),
            (GuildErrorKind::NoEmperium, "Creation requires an Emperium"),
            (GuildErrorKind::InvalidEmblem, "Emblem is invalid"),
            (
                GuildErrorKind::CannotTargetMaster,
                "Guild master cannot be expelled",
            ),
            (GuildErrorKind::InvalidPosition, "Position is invalid"),
        ];
        for (error, copy) in expected {
            assert_eq!(guild_error_text(error), copy);
        }
        assert_eq!(
            guild_error_text(GuildErrorKind::Unknown(99)),
            "Guild operation failed"
        );
    }

    #[test]
    fn successful_notice_result_releases_pending_without_changing_durable_notice() {
        let generation = ZoneSessionGeneration(12);
        let mut app = App::new();
        app.add_message::<GuildIngress>();
        app.insert_resource(generation);
        app.insert_resource(GuildUi {
            pending: Some(PendingGuildMutation {
                action: "notice_edit",
                generation,
            }),
            ..default()
        });
        app.init_resource::<emblem::GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        app.add_systems(Update, apply_guild_results);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "notice_edit".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        });

        app.update();

        let ui = app.world().resource::<GuildUi>();
        assert!(ui.pending.is_none());
        assert_eq!(
            ui.feedback.as_deref(),
            Some("Notice saved. Waiting for guild information…")
        );
    }

    #[test]
    fn mismatched_management_result_is_ignored_then_matching_failure_is_shown() {
        let generation = ZoneSessionGeneration(13);
        let mut app = App::new();
        app.add_message::<GuildIngress>();
        app.insert_resource(generation);
        app.insert_resource(GuildUi {
            pending: Some(PendingGuildMutation {
                action: "position_edit",
                generation,
            }),
            ..default()
        });
        app.init_resource::<emblem::GuildEmblemImages>()
            .insert_resource(Assets::<Image>::default());
        app.add_systems(Update, apply_guild_results);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "notice_edit".into(),
                success: false,
                error: GuildErrorKind::NoPermission,
            }),
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<GuildUi>()
                .pending
                .as_ref()
                .unwrap()
                .action,
            "position_edit"
        );

        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "position_edit".into(),
                success: false,
                error: GuildErrorKind::InvalidPosition,
            }),
        });
        app.update();

        let ui = app.world().resource::<GuildUi>();
        assert!(ui.pending.is_none());
        assert_eq!(ui.feedback.as_deref(), Some("Position is invalid"));
    }

    #[test]
    fn one_pending_mutation_disables_every_management_control() {
        let generation = ZoneSessionGeneration(2);
        let mut app = App::new();
        app.insert_resource(GuildUi {
            pending: Some(PendingGuildMutation {
                action: "notice_edit",
                generation,
            }),
            ..default()
        });
        let first = app.world_mut().spawn(GuildMutationControl).id();
        let second = app.world_mut().spawn(GuildMutationControl).id();
        app.add_systems(Update, sync_management_controls);

        app.update();
        assert!(app.world().entity(first).contains::<InteractionDisabled>());
        assert!(app.world().entity(second).contains::<InteractionDisabled>());

        app.world_mut().resource_mut::<GuildUi>().pending = None;
        app.update();
        assert!(!app.world().entity(first).contains::<InteractionDisabled>());
        assert!(!app.world().entity(second).contains::<InteractionDisabled>());
    }

    #[test]
    fn same_generation_character_switch_resets_ui_and_blocks_old_session_work() {
        let mut app = App::new();
        app.add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(4))
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(GuildUi {
                selected_tab: GuildTab::Notice,
                create_name: "Character A draft".into(),
                feedback: Some("pending".into()),
                pending: Some(PendingGuildMutation {
                    action: "create",
                    generation: ZoneSessionGeneration(4),
                }),
                ..default()
            })
            .insert_resource(GuildUiSession {
                generation: ZoneSessionGeneration(4),
                char_id: 42,
                ..default()
            });
        let root = app
            .world_mut()
            .spawn((GuildWindowRoot, Visibility::Visible))
            .id();
        let create = app
            .world_mut()
            .spawn((GuildCreateNameField, EditableText::new("Character A")))
            .id();
        let invite = app
            .world_mut()
            .spawn((GuildInviteNameField, EditableText::new("Old target")))
            .id();
        app.add_systems(Update, reset_guild_ui_session);

        app.world_mut().resource_mut::<ZoneSession>().char_id = 43;
        app.update();

        assert_eq!(*app.world().resource::<GuildUi>(), GuildUi::default());
        assert_eq!(
            *app.world().entity(root).get::<Visibility>().unwrap(),
            Visibility::Hidden
        );
        assert!(app
            .world()
            .entity(create)
            .get::<EditableText>()
            .unwrap()
            .value()
            .to_string()
            .is_empty());
        assert!(app
            .world()
            .entity(invite)
            .get::<EditableText>()
            .unwrap()
            .value()
            .to_string()
            .is_empty());
        let session = app.world().resource::<GuildUiSession>();
        assert_eq!(session.char_id, 43);
        assert!(session.blocked);
        assert!(session.reset);
    }

    #[test]
    fn connection_replacement_disconnect_does_not_block_the_fresh_ui_epoch() {
        let mut app = App::new();
        app.add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(1))
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(GuildUi {
                feedback: Some("Character A".into()),
                ..default()
            })
            .insert_resource(GuildUiSession {
                generation: ZoneSessionGeneration(1),
                char_id: 42,
                ..default()
            });
        app.add_systems(Update, reset_guild_ui_session);

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(2);
        app.world_mut().resource_mut::<ZoneSession>().char_id = 43;
        app.world_mut().write_message(ZoneDisconnected {
            reason: "replaced".into(),
        });
        app.update();

        assert_eq!(*app.world().resource::<GuildUi>(), GuildUi::default());
        let session = app.world().resource::<GuildUiSession>();
        assert_eq!(session.generation, ZoneSessionGeneration(2));
        assert_eq!(session.char_id, 43);
        assert!(!session.blocked);
        assert!(session.reset);
    }
}
