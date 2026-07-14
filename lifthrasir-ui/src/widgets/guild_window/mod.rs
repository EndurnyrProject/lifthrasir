mod members;
pub mod scene;

use bevy::input_focus::{FocusCause, InputFocus};
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
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::ZoneSessionGeneration;

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

#[derive(Resource, Debug, Default)]
pub struct GuildUi {
    pub selected_tab: GuildTab,
    pub create_name: String,
    pub feedback: Option<String>,
    pub pending: Option<PendingGuildMutation>,
    feedback_is_error: bool,
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
pub struct GuildMembersList;
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
pub struct MembersTabButton;
#[derive(Component, Default, Clone)]
pub struct PositionsTabButton;
#[derive(Component, Default, Clone)]
pub struct NoticeTabButton;

pub struct GuildWindowPlugin;

impl Plugin for GuildWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<GuildUi>()
            .add_systems(
                Update,
                toggle_guild_window.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_create_draft,
                    apply_guild_results,
                    sync_membership_mode,
                    sync_header,
                    sync_tabs,
                    sync_feedback,
                    refresh_members,
                    release_hidden_guild_focus,
                )
                    .chain()
                    .in_set(GuildSystems::UiSync)
                    .run_if(in_state(GameState::InGame)),
            );
        app.add_systems(OnExit(GameState::InGame), clear_guild_focus_on_exit);
    }
}

fn toggle_guild_window(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    guild: Res<GuildState>,
    ui_focus: Res<UiFocus>,
    mut root: Query<&mut Visibility, With<GuildWindowRoot>>,
    field: Query<Entity, With<GuildCreateNameField>>,
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
        if !guild.in_guild() {
            if let Ok(field) = field.single() {
                input_focus.set(field, FocusCause::Navigated);
            }
        }
        return;
    }
    *visibility = Visibility::Hidden;
    clear_create_focus(&mut input_focus, &field);
}

fn clear_create_focus(
    input_focus: &mut InputFocus,
    field: &Query<Entity, With<GuildCreateNameField>>,
) {
    if input_focus
        .get()
        .is_some_and(|focused| field.contains(focused))
    {
        input_focus.clear();
    }
}

fn release_hidden_guild_focus(
    root: Query<&Visibility, With<GuildWindowRoot>>,
    field: Query<Entity, With<GuildCreateNameField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    if root
        .single()
        .is_ok_and(|visibility| *visibility == Visibility::Hidden)
    {
        clear_create_focus(&mut input_focus, &field);
    }
}

fn clear_guild_focus_on_exit(
    field: Query<Entity, With<GuildCreateNameField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    clear_create_focus(&mut input_focus, &field);
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
    mut ui: ResMut<GuildUi>,
) {
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
            continue;
        }
        ui.pending = None;
        if result.success {
            ui.feedback = Some("Guild created. Waiting for guild information…".to_string());
            ui.feedback_is_error = false;
        } else {
            ui.feedback = Some(guild_error_text(result.error).to_string());
            ui.feedback_is_error = true;
        }
    }
}

fn guild_error_text(error: GuildErrorKind) -> &'static str {
    match error {
        GuildErrorKind::NameTaken => "Guild name is already taken.",
        GuildErrorKind::AlreadyInGuild => "Character already belongs to a guild.",
        GuildErrorKind::GuildFull => "Guild is full.",
        GuildErrorKind::NoPermission => "Current position lacks permission.",
        GuildErrorKind::NotMember => "Character is not a guild member.",
        GuildErrorKind::TargetOffline => "Target is offline.",
        GuildErrorKind::NoEmperium => "Creation requires an Emperium.",
        GuildErrorKind::InvalidEmblem => "Emblem is invalid.",
        GuildErrorKind::CannotTargetMaster => "Guild master cannot be expelled.",
        GuildErrorKind::InvalidPosition => "Position is invalid.",
        GuildErrorKind::None | GuildErrorKind::Unknown(_) => "Guild action failed.",
    }
}

fn sync_membership_mode(
    guild: Res<GuildState>,
    mut create: Query<&mut Visibility, (With<GuildUnguildedPanel>, Without<GuildGuildedPanel>)>,
    mut guilded: Query<&mut Visibility, (With<GuildGuildedPanel>, Without<GuildUnguildedPanel>)>,
) {
    let Ok(mut create) = create.single_mut() else {
        return;
    };
    let Ok(mut guilded) = guilded.single_mut() else {
        return;
    };
    if guild.in_guild() {
        *create = Visibility::Hidden;
        *guilded = Visibility::Inherited;
    } else {
        *create = Visibility::Inherited;
        *guilded = Visibility::Hidden;
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
        &mut Visibility,
        (
            With<GuildMembersPanel>,
            Without<GuildPositionsPanel>,
            Without<GuildNoticePanel>,
        ),
    >,
    mut positions: Query<
        &mut Visibility,
        (
            With<GuildPositionsPanel>,
            Without<GuildMembersPanel>,
            Without<GuildNoticePanel>,
        ),
    >,
    mut notice: Query<
        &mut Visibility,
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
    *members = if ui.selected_tab == GuildTab::Members {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    *positions = if ui.selected_tab == GuildTab::Positions {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    *notice = if ui.selected_tab == GuildTab::Notice {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

fn sync_feedback(
    ui: Res<GuildUi>,
    mut feedback: Query<(&mut Text, &mut TextColor, &mut Visibility), With<GuildFeedbackText>>,
    button: Query<Entity, With<GuildCreateButton>>,
    mut commands: Commands,
) {
    if let Ok((mut text, mut color, mut visibility)) = feedback.single_mut() {
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
    fn passive_primary_window_focus_opens_and_focuses_the_unguilded_create_field() {
        let (mut app, field, player) = toggle_app(Visibility::Hidden);
        let primary_window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.insert_resource(InputFocus::from_entity(primary_window));
        app.update();
        assert!(!app.world().resource::<UiFocus>().text_input_active);

        press_guild(&mut app, player);
        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
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
        let other = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((GuildWindowRoot, Visibility::Hidden));
        app.add_systems(Update, release_hidden_guild_focus);

        app.insert_resource(InputFocus::from_entity(field));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);

        app.insert_resource(InputFocus::from_entity(other));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(other));
    }

    fn authoritative_ui_app() -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(ZoneSessionGeneration(9))
            .add_plugins(game_engine::domain::guild::GuildPlugin);
        app.world_mut()
            .spawn((GuildUnguildedPanel, Visibility::Hidden));
        app.world_mut()
            .spawn((GuildGuildedPanel, Visibility::Inherited));
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
        assert_eq!(marked_text::<GuildNameText>(&mut app), "Vikings");
        assert_eq!(
            marked_text::<GuildMasterText>(&mut app),
            "Guild Master Odin"
        );
        assert_eq!(marked_text::<GuildNoticeText>(&mut app), "Welcome");
        assert_eq!(marked_text::<GuildMemberCountText>(&mut app), "1 member");
        assert_eq!(marked_text::<GuildOnlineCountText>(&mut app), "0 online");
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
}
