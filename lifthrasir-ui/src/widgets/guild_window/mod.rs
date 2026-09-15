mod dialogs;
pub(crate) mod emblem;
mod feedback;
mod members;
mod notice;
mod positions;
mod relation_dialogs;
mod relations;
pub mod scene;
mod skills;
pub(crate) mod slash;

pub(crate) use members::request_invite;

use feedback::apply_guild_results;
#[cfg(test)]
use feedback::guild_error_text;

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
use net_contract::events::ZoneDisconnected;
#[cfg(test)]
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::theme::feathers_theme::install_norse_theme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuildTab {
    #[default]
    Members,
    Positions,
    Notice,
    Skills,
    Relations,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGuildMutation {
    pub action: &'static str,
    pub generation: ZoneSessionGeneration,
}

#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct GuildUi {
    pub selected_tab: GuildTab,
    pub feedback: Option<String>,
    pub pending: Option<PendingGuildMutation>,
    feedback_is_error: bool,
}

#[derive(Resource, Default)]
pub(crate) struct GuildUiSession {
    generation: ZoneSessionGeneration,
    char_id: u32,
    guild_id: u32,
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
pub struct GuildMembersPanel;
#[derive(Component, Default, Clone)]
pub struct GuildPositionsPanel;
#[derive(Component, Default, Clone)]
pub struct GuildNoticePanel;
#[derive(Component, Default, Clone)]
pub struct GuildSkillsPanel;
#[derive(Component, Default, Clone)]
pub struct GuildRelationsPanel;
#[derive(Component, Default, Clone)]
pub struct GuildPositionsList;
#[derive(Component, Default, Clone)]
pub struct GuildSkillsList;
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
pub struct GuildLevelText;
#[derive(Component, Default, Clone)]
pub struct GuildExpText;
#[derive(Component, Default, Clone)]
pub struct GuildSkillPointsText;
#[derive(Component, Default, Clone)]
pub struct GuildExpFill;
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
pub struct SkillsTabButton;
#[derive(Component, Default, Clone)]
pub struct RelationsTabButton;
#[derive(Component, Default, Clone)]
pub struct GuildTabButton;
#[derive(Component, Default, Clone)]
pub struct GuildTabPage;
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
    With<GuildInviteNameField>,
    With<positions::PositionNameField>,
    With<positions::PositionTaxField>,
    With<notice::GuildNoticeSubjectField>,
    With<notice::GuildNoticeBodyField>,
    With<members::GuildExpelReasonField>,
    With<relations::GuildAllianceNameField>,
    With<relations::GuildAntagonistNameField>,
)>;
type GuildTextFields<'w, 's> = Query<'w, 's, Entity, GuildTextFieldFilter>;
type GuildEditableTextFields<'w, 's> =
    Query<'w, 's, &'static mut EditableText, GuildTextFieldFilter>;

pub struct GuildWindowPlugin;

impl Plugin for GuildWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_message::<slash::GuildSlashSubmitted>()
            .init_resource::<GuildUi>()
            .init_resource::<GuildUiSession>()
            .init_resource::<positions::PositionDraftState>()
            .init_resource::<emblem::GuildEmblemPreview>()
            .init_resource::<dialogs::PendingGuildInvite>()
            .init_resource::<dialogs::PendingGuildConfirmation>()
            .init_resource::<relation_dialogs::PendingAllianceInvite>()
            .init_resource::<relation_dialogs::PendingRelationConfirmation>()
            .add_systems(
                Update,
                (
                    reset_guild_ui_session,
                    positions::reset_position_drafts,
                    dialogs::reset_stale_invite,
                    dialogs::reset_stale_confirmation,
                    emblem::reset_emblem_preview,
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
                        reset_guild_ui_guild,
                        relation_dialogs::reset_invalid_relation_dialogs,
                        slash::dispatch_guild_slash
                            .after(crate::widgets::chat_box::chat_input_control),
                        positions::sync_position_drafts,
                        apply_guild_results,
                        relation_dialogs::expire_pending_alliance,
                        relation_dialogs::queue_incoming_alliance,
                        relation_dialogs::claim_alliance_choice,
                        relation_dialogs::claim_relation_confirmation,
                        positions::resolve_position_submission,
                        feedback::ingest_guild_announcements,
                        sync_membership_mode,
                        sync_header,
                        emblem::invalidate_picker_when_hidden,
                        emblem::poll_picker,
                        emblem::receive_emblem_changes,
                        emblem::queue_current_guild_emblem,
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
                        skills::refresh_skills,
                        relations::refresh_relations,
                        relations::sync_relation_controls,
                        positions::sync_invite_labels,
                        positions::sync_expel_labels,
                        positions::sync_storage_toggles,
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
                positions::clear_position_drafts,
                dialogs::clear_pending_invite,
                dialogs::clear_pending_confirmation,
                relation_dialogs::clear_relation_dialogs,
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

fn reset_guild_ui_guild(
    guild: Res<GuildState>,
    mut session: ResMut<GuildUiSession>,
    mut ui: ResMut<GuildUi>,
    mut drafts: ResMut<positions::PositionDraftState>,
    mut roots: Query<&mut Visibility, With<GuildWindowRoot>>,
    mut fields: GuildEditableTextFields,
) {
    let guild_id = guild.info().map_or(0, |info| info.guild_id);
    if session.guild_id == guild_id {
        return;
    }
    session.guild_id = guild_id;
    session.reset = true;
    *ui = GuildUi::default();
    *drafts = positions::PositionDraftState::default();
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
    relation_confirmation: Option<Res<relation_dialogs::PendingRelationConfirmation>>,
    controls: Query<Entity, With<GuildMutationControl>>,
    mut commands: Commands,
) {
    for control in &controls {
        if ui.pending.is_some()
            || confirmation
                .as_deref()
                .is_some_and(|pending| pending.is_pending())
            || relation_confirmation
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

pub(crate) fn select_members(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Members;
}

pub(crate) fn select_positions(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Positions;
}

pub(crate) fn select_notice(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Notice;
}

pub(crate) fn select_skills(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Skills;
}

pub(crate) fn select_relations(_: On<Activate>, mut ui: ResMut<GuildUi>) {
    ui.selected_tab = GuildTab::Relations;
}

fn sync_membership_mode(
    guild: Res<GuildState>,
    mut root: Query<&mut Visibility, With<GuildWindowRoot>>,
) {
    if !guild.in_guild() {
        for mut visibility in &mut root {
            *visibility = Visibility::Hidden;
        }
    }
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
        Query<&mut Text, With<GuildLevelText>>,
        Query<&mut Text, With<GuildExpText>>,
        Query<&mut Text, With<GuildSkillPointsText>>,
    )>,
    mut exp_fill: Query<&mut Node, With<GuildExpFill>>,
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
    set_single_text(&mut texts.p5(), format!("Guild Level {}", info.level));
    let exp_label = if info.next_exp == 0 {
        format!("Guild EXP {} · MAX", info.exp)
    } else {
        format!("Guild EXP {} / {}", info.exp, info.next_exp)
    };
    set_single_text(&mut texts.p6(), exp_label);
    let point_label = if info.skill_points == 1 {
        "skill point"
    } else {
        "skill points"
    };
    set_single_text(
        &mut texts.p7(),
        format!("{} {point_label} available", info.skill_points),
    );
    if let Ok(mut fill) = exp_fill.single_mut() {
        let progress_percent = if info.next_exp == 0 {
            100.0
        } else {
            (info.exp as f64 / info.next_exp as f64 * 100.0).clamp(0.0, 100.0) as f32
        };
        let width = percent(progress_percent);
        if fill.width != width {
            fill.width = width;
        }
    }
}

fn set_single_text<F: bevy::ecs::query::QueryFilter>(
    query: &mut Query<&mut Text, F>,
    value: String,
) {
    if let Ok(mut text) = query.single_mut()
        && text.0 != value
    {
        text.0 = value;
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
            Without<GuildSkillsPanel>,
            Without<GuildRelationsPanel>,
        ),
    >,
    mut positions: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildPositionsPanel>,
            Without<GuildMembersPanel>,
            Without<GuildNoticePanel>,
            Without<GuildSkillsPanel>,
            Without<GuildRelationsPanel>,
        ),
    >,
    mut notice: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildNoticePanel>,
            Without<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
            Without<GuildSkillsPanel>,
            Without<GuildRelationsPanel>,
        ),
    >,
    mut skills: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildSkillsPanel>,
            Without<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
            Without<GuildNoticePanel>,
            Without<GuildRelationsPanel>,
        ),
    >,
    mut relations: Query<
        (&mut Visibility, &mut Node),
        (
            With<GuildRelationsPanel>,
            Without<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
            Without<GuildNoticePanel>,
            Without<GuildSkillsPanel>,
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
    if let Ok((mut visibility, mut node)) = skills.single_mut() {
        let active = ui.selected_tab == GuildTab::Skills;
        *visibility = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if active { Display::Flex } else { Display::None };
    }
    if let Ok((mut visibility, mut node)) = relations.single_mut() {
        let active = ui.selected_tab == GuildTab::Relations;
        *visibility = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if active { Display::Flex } else { Display::None };
    }
}

fn sync_feedback(
    ui: Res<GuildUi>,
    mut feedback: Query<(&mut Text, &mut TextColor, &mut Visibility), With<GuildFeedbackText>>,
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
            .spawn((GuildInviteNameField, EditableText::new("")))
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
    fn unguilded_hotkey_keeps_window_closed_without_claiming_focus() {
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
        assert_eq!(*visibility, Visibility::Hidden);
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
    fn selecting_skills_removes_every_other_page_from_layout() {
        let mut app = App::new();
        app.insert_resource(GuildUi {
            selected_tab: GuildTab::Skills,
            ..default()
        });
        app.world_mut()
            .spawn((GuildMembersPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildPositionsPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildNoticePanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildSkillsPanel, Node::default(), Visibility::Hidden));
        app.add_systems(Update, sync_tabs);

        app.update();

        assert_eq!(node::<GuildMembersPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildPositionsPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildNoticePanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildSkillsPanel>(&mut app).display, Display::Flex);
        assert_eq!(
            visibility::<GuildSkillsPanel>(&mut app),
            Visibility::Inherited
        );
    }

    #[test]
    fn selecting_relations_removes_all_four_other_pages_from_layout() {
        let mut app = App::new();
        app.insert_resource(GuildUi {
            selected_tab: GuildTab::Relations,
            ..default()
        });
        app.world_mut()
            .spawn((GuildMembersPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildPositionsPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildNoticePanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildSkillsPanel, Node::default(), Visibility::Inherited));
        app.world_mut()
            .spawn((GuildRelationsPanel, Node::default(), Visibility::Hidden));
        app.add_systems(Update, sync_tabs);

        app.update();

        assert_eq!(node::<GuildMembersPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildPositionsPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildNoticePanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildSkillsPanel>(&mut app).display, Display::None);
        assert_eq!(node::<GuildRelationsPanel>(&mut app).display, Display::Flex);
        assert_eq!(
            visibility::<GuildRelationsPanel>(&mut app),
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
        let field = app.world_mut().spawn(GuildInviteNameField).id();
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
        let field = app.world_mut().spawn(positions::PositionNameField).id();
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
    fn hidden_root_releases_position_tax_field_focus() {
        let mut app = App::new();
        app.init_resource::<InputFocus>();
        let tax = app.world_mut().spawn(positions::PositionTaxField).id();
        app.world_mut().spawn((GuildWindowRoot, Visibility::Hidden));
        app.add_systems(Update, release_hidden_guild_focus);
        app.insert_resource(InputFocus::from_entity(tax));

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
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
            .spawn((GuildWindowRoot, Visibility::Visible));
        app.world_mut().spawn((GuildNameText, Text::default()));
        app.world_mut().spawn((GuildMasterText, Text::default()));
        app.world_mut().spawn((GuildNoticeText, Text::default()));
        app.world_mut()
            .spawn((GuildMemberCountText, Text::default()));
        app.world_mut()
            .spawn((GuildOnlineCountText, Text::default()));
        app.world_mut().spawn((GuildLevelText, Text::default()));
        app.world_mut().spawn((GuildExpText, Text::default()));
        app.world_mut()
            .spawn((GuildSkillPointsText, Text::default()));
        app.world_mut().spawn((GuildExpFill, Node::default()));
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
    fn authoritative_snapshot_enables_management_without_opening_window() {
        let mut app = authoritative_ui_app();
        app.update();
        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Hidden);

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
                level: 1,
                exp: 0,
                next_exp: 0,
                skill_points: 0,
                skills: vec![],
                relations: vec![],
            }),
        });
        app.update();

        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Hidden);
        assert_eq!(marked_text::<GuildNameText>(&mut app), "Vikings");
        assert_eq!(
            marked_text::<GuildMasterText>(&mut app),
            "Guild Master Odin"
        );
        assert_eq!(marked_text::<GuildNoticeText>(&mut app), "Welcome");
        assert_eq!(marked_text::<GuildMemberCountText>(&mut app), "1 member");
        assert_eq!(marked_text::<GuildOnlineCountText>(&mut app), "0 online");

        app.init_resource::<UiFocus>();
        app.init_resource::<InputFocus>();
        let player = app
            .world_mut()
            .spawn((LocalPlayer, ActionState::<PlayerAction>::default()))
            .id();
        app.add_systems(Update, toggle_guild_window);
        press_guild(&mut app, player);
        app.update();
        assert_eq!(visibility::<GuildWindowRoot>(&mut app), Visibility::Visible);
    }

    #[test]
    fn progression_header_keeps_large_exp_exact_and_caps_without_division() {
        let mut app = authoritative_ui_app();
        let large_exp = u32::MAX as u64 + 99;
        let next_exp = u32::MAX as u64 + 1_099;
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(GuildInfo {
                guild_id: 7,
                name: "Vikings".into(),
                master_char_id: 42,
                emblem_id: 0,
                notice_subject: String::new(),
                notice_body: String::new(),
                positions: vec![],
                members: vec![],
                level: 27,
                exp: large_exp,
                next_exp,
                skill_points: 4,
                skills: vec![],
                relations: vec![],
            }),
        });
        app.update();

        assert_eq!(marked_text::<GuildLevelText>(&mut app), "Guild Level 27");
        assert_eq!(
            marked_text::<GuildExpText>(&mut app),
            format!("Guild EXP {large_exp} / {next_exp}")
        );
        assert_eq!(
            marked_text::<GuildSkillPointsText>(&mut app),
            "4 skill points available"
        );
        assert_eq!(node::<GuildExpFill>(&mut app).width, percent(99.999_98));

        let mut capped = app.world().resource::<GuildState>().info().unwrap().clone();
        capped.next_exp = 0;
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(capped),
        });
        app.update();

        assert_eq!(
            marked_text::<GuildExpText>(&mut app),
            format!("Guild EXP {large_exp} · MAX")
        );
        assert_eq!(node::<GuildExpFill>(&mut app).width, percent(100));
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
        app.init_resource::<emblem::GuildEmblemPreview>()
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
        app.init_resource::<emblem::GuildEmblemPreview>()
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
    fn guild_error_copy_maps_progression_and_relation_errors_distinctly() {
        let expected = [
            (
                GuildErrorKind::NoSkillPoints,
                "No guild skill points available",
            ),
            (
                GuildErrorKind::SkillRequirement,
                "Guild skill requirements are not met",
            ),
            (
                GuildErrorKind::SkillMaxed,
                "Guild skill is already at maximum level",
            ),
            (GuildErrorKind::AllyLimit, "Guild alliance limit reached"),
            (
                GuildErrorKind::AntagonistLimit,
                "Guild antagonist limit reached",
            ),
            (GuildErrorKind::AlreadyAllied, "Guilds are already allied"),
            (
                GuildErrorKind::AlreadyAntagonist,
                "Guild is already an antagonist",
            ),
            (GuildErrorKind::NotRelated, "Guild relation does not exist"),
            (GuildErrorKind::SameGuild, "Cannot target the same guild"),
            (
                GuildErrorKind::SiegeActive,
                "Guild relations cannot change during a siege",
            ),
            (
                GuildErrorKind::RequestPending,
                "An alliance request is already pending",
            ),
            (
                GuildErrorKind::AllianceDeclined,
                "Alliance request was declined",
            ),
        ];

        for (error, copy) in expected {
            assert_eq!(guild_error_text(error), copy);
        }
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
        app.init_resource::<emblem::GuildEmblemPreview>()
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
        app.init_resource::<emblem::GuildEmblemPreview>()
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
        let position = app
            .world_mut()
            .spawn((
                positions::PositionNameField,
                EditableText::new("Character A"),
            ))
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
        assert!(
            app.world()
                .entity(position)
                .get::<EditableText>()
                .unwrap()
                .value()
                .to_string()
                .is_empty()
        );
        assert!(
            app.world()
                .entity(invite)
                .get::<EditableText>()
                .unwrap()
                .value()
                .to_string()
                .is_empty()
        );
        let session = app.world().resource::<GuildUiSession>();
        assert_eq!(session.char_id, 43);
        assert!(session.blocked);
        assert!(session.reset);
    }

    #[test]
    fn own_guild_change_resets_pending_ui_and_relation_fields_before_capture() {
        let generation = ZoneSessionGeneration(9);
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(generation)
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(GuildUiSession {
                generation,
                char_id: 42,
                guild_id: 7,
                ..default()
            })
            .insert_resource(GuildUi {
                selected_tab: GuildTab::Relations,
                feedback: Some("old guild".into()),
                pending: Some(PendingGuildMutation {
                    action: "alliance_request",
                    generation,
                }),
                ..default()
            })
            .init_resource::<positions::PositionDraftState>()
            .add_plugins(game_engine::domain::guild::GuildPlugin)
            .add_systems(Update, reset_guild_ui_guild.in_set(GuildSystems::UiSync));
        let alliance = app
            .world_mut()
            .spawn((
                relations::GuildAllianceNameField,
                EditableText::new("Freya"),
            ))
            .id();
        let antagonist = app
            .world_mut()
            .spawn((
                relations::GuildAntagonistNameField,
                EditableText::new("Loki"),
            ))
            .id();
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(GuildInfo {
                guild_id: 11,
                name: "New Guild".into(),
                master_char_id: 42,
                emblem_id: 0,
                notice_subject: String::new(),
                notice_body: String::new(),
                positions: vec![],
                members: vec![],
                level: 1,
                exp: 0,
                next_exp: 0,
                skill_points: 0,
                skills: vec![],
                relations: vec![],
            }),
        });

        app.update();

        assert_eq!(*app.world().resource::<GuildUi>(), GuildUi::default());
        assert!(
            app.world()
                .entity(alliance)
                .get::<EditableText>()
                .unwrap()
                .value()
                .to_string()
                .is_empty()
        );
        assert!(
            app.world()
                .entity(antagonist)
                .get::<EditableText>()
                .unwrap()
                .value()
                .to_string()
                .is_empty()
        );
        let session = app.world().resource::<GuildUiSession>();
        assert_eq!(session.guild_id, 11);
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
