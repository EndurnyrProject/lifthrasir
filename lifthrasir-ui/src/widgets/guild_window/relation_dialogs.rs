use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use game_engine::domain::guild::GuildState;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use net_contract::commands::{
    GuildAllianceBreakRequested, GuildAllianceResponded, GuildAntagonistRequested,
};
use net_contract::dto::{GuildAllianceInviteInfo, GuildRelationKind};
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use super::{GuildMutationContext, GuildUi, GuildUiSession, PendingGuildMutation};
use crate::widgets::system_dialog::SystemDialogRoot;

const ALLIANCE_INVITE_TTL_SECS: f32 = 30.0;

#[derive(Resource, Default)]
pub(crate) struct PendingAllianceInvite {
    invite: Option<GuildAllianceInviteInfo>,
    generation: ZoneSessionGeneration,
    own_guild_id: u32,
    timer: Timer,
    choice: Option<bool>,
    correlation: Option<u64>,
    next_correlation: u64,
}

impl PendingAllianceInvite {
    pub(crate) fn is_pending(&self) -> bool {
        self.invite.is_some()
    }

    fn set(
        &mut self,
        invite: GuildAllianceInviteInfo,
        generation: ZoneSessionGeneration,
        own_guild_id: u32,
    ) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.invite = Some(invite);
        self.generation = generation;
        self.own_guild_id = own_guild_id;
        self.timer = Timer::from_seconds(ALLIANCE_INVITE_TTL_SECS, TimerMode::Once);
        self.choice = None;
        self.correlation = Some(self.next_correlation);
    }

    fn clear(&mut self) {
        self.invite = None;
        self.own_guild_id = 0;
        self.timer = Timer::default();
        self.choice = None;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RelationConfirmationAction {
    BreakAlliance { guild_id: u32, guild_name: String },
    DeclareAntagonist { target_name: String },
}

#[derive(Resource, Default)]
pub(crate) struct PendingRelationConfirmation {
    action: Option<RelationConfirmationAction>,
    generation: ZoneSessionGeneration,
    own_guild_id: u32,
    choice: Option<bool>,
    correlation: Option<u64>,
    next_correlation: u64,
}

impl PendingRelationConfirmation {
    pub(crate) fn is_pending(&self) -> bool {
        self.action.is_some()
    }

    pub(crate) fn break_alliance(
        &mut self,
        generation: ZoneSessionGeneration,
        own_guild_id: u32,
        guild_id: u32,
        guild_name: String,
    ) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.action = Some(RelationConfirmationAction::BreakAlliance {
            guild_id,
            guild_name,
        });
        self.generation = generation;
        self.own_guild_id = own_guild_id;
        self.choice = None;
        self.correlation = Some(self.next_correlation);
    }

    pub(crate) fn declare_antagonist(
        &mut self,
        generation: ZoneSessionGeneration,
        own_guild_id: u32,
        target_name: String,
    ) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.action = Some(RelationConfirmationAction::DeclareAntagonist { target_name });
        self.generation = generation;
        self.own_guild_id = own_guild_id;
        self.choice = None;
        self.correlation = Some(self.next_correlation);
    }

    fn kind(&self) -> Option<SystemDialogKind> {
        match self.action {
            Some(RelationConfirmationAction::BreakAlliance { .. }) => {
                Some(SystemDialogKind::GuildAllianceBreak)
            }
            Some(RelationConfirmationAction::DeclareAntagonist { .. }) => {
                Some(SystemDialogKind::GuildAntagonistDeclare)
            }
            None => None,
        }
    }

    fn clear(&mut self) {
        self.action = None;
        self.own_guild_id = 0;
        self.choice = None;
    }
}

#[derive(SystemParam)]
pub(crate) struct RelationValidationContext<'w> {
    guild: Res<'w, GuildState>,
    zone_session: Res<'w, ZoneSession>,
    generation: Res<'w, ZoneSessionGeneration>,
    ui_session: Res<'w, GuildUiSession>,
}

fn interaction_is_valid(
    guild: &GuildState,
    session: &ZoneSession,
    generation: ZoneSessionGeneration,
    expected_generation: ZoneSessionGeneration,
    own_guild_id: u32,
) -> bool {
    generation == expected_generation
        && guild
            .info()
            .is_some_and(|info| info.guild_id == own_guild_id)
        && guild.is_master(session.char_id)
}

pub(crate) fn reset_invalid_relation_dialogs(
    context: RelationValidationContext,
    mut invite: ResMut<PendingAllianceInvite>,
    mut confirmation: ResMut<PendingRelationConfirmation>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    let invite_invalid = invite.is_pending()
        && (context.ui_session.reset
            || context.ui_session.blocked
            || !interaction_is_valid(
                &context.guild,
                &context.zone_session,
                *context.generation,
                invite.generation,
                invite.own_guild_id,
            ));
    if invite_invalid {
        let correlation = invite.correlation;
        invite.clear();
        despawn_owned(
            &roots,
            &mut commands,
            SystemDialogKind::GuildAllianceInvite,
            correlation,
        );
    }

    let confirmation_invalid = confirmation.is_pending()
        && (context.ui_session.reset
            || context.ui_session.blocked
            || !interaction_is_valid(
                &context.guild,
                &context.zone_session,
                *context.generation,
                confirmation.generation,
                confirmation.own_guild_id,
            ));
    if confirmation_invalid {
        let kind = confirmation.kind();
        let correlation = confirmation.correlation;
        confirmation.clear();
        if let Some(kind) = kind {
            despawn_owned(&roots, &mut commands, kind, correlation);
        }
    }
}

pub(crate) fn queue_incoming_alliance(
    mut ingress: MessageReader<GuildIngress>,
    guild: Res<GuildState>,
    zone_session: Res<ZoneSession>,
    generation: Res<ZoneSessionGeneration>,
    ui_session: Res<GuildUiSession>,
    mut pending: ResMut<PendingAllianceInvite>,
) {
    if ui_session.blocked {
        ingress.clear();
        return;
    }
    let mut newest = None;
    for event in ingress.read() {
        if event.generation != *generation {
            continue;
        }
        if let GuildIngressPayload::AllianceRequestNotified(invite) = &event.payload {
            newest = Some(invite.clone());
        }
    }
    if pending.is_pending() || !guild.is_master(zone_session.char_id) {
        return;
    }
    let Some(info) = guild.info() else {
        return;
    };
    if let Some(invite) = newest {
        pending.set(invite, *generation, info.guild_id);
    }
}

pub(crate) fn claim_alliance_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    guild: Res<GuildState>,
    zone_session: Res<ZoneSession>,
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingAllianceInvite>,
    mut ui: ResMut<GuildUi>,
    mut responses: MessageWriter<GuildAllianceResponded>,
) {
    if !pending.is_pending() {
        return;
    }
    if let Some(choice) = choices
        .read()
        .filter(|choice| {
            choice.kind == SystemDialogKind::GuildAllianceInvite
                && choice.correlation == pending.correlation
        })
        .last()
    {
        pending.choice = Some(choice.primary);
    }
    let Some(accept) = pending.choice else {
        return;
    };
    if !interaction_is_valid(
        &guild,
        &zone_session,
        *generation,
        pending.generation,
        pending.own_guild_id,
    ) {
        pending.clear();
        return;
    }
    if ui.pending.is_some() {
        return;
    }
    let guild_id = pending.invite.as_ref().unwrap().guild_id;
    ui.pending = Some(PendingGuildMutation {
        action: "alliance_response",
        generation: *generation,
    });
    ui.feedback = Some(if accept {
        "Accepting alliance request…".into()
    } else {
        "Declining alliance request…".into()
    });
    ui.feedback_is_error = false;
    responses.write(GuildAllianceResponded { guild_id, accept });
    pending.clear();
}

pub(crate) fn show_pending_alliance(
    pending: Res<PendingAllianceInvite>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(invite) = pending.invite.as_ref() else {
        return;
    };
    if pending.choice.is_some() || !existing.is_empty() {
        return;
    }
    dialogs.write(ShowSystemDialog {
        severity: DialogSeverity::Info,
        kind: SystemDialogKind::GuildAllianceInvite,
        kicker: "Guild Relations".into(),
        title: "Alliance Request".into(),
        message: format!(
            "{} of {} requests an alliance.",
            invite.requester_name, invite.guild_name
        ),
        code: String::new(),
        button_label: "Accept".into(),
        secondary_label: "Decline".into(),
        confirm_state: None,
        correlation: pending.correlation,
    });
}

pub(crate) fn expire_pending_alliance(
    time: Res<Time>,
    mut pending: ResMut<PendingAllianceInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() || !pending.timer.tick(time.delta()).just_finished() {
        return;
    }
    let correlation = pending.correlation;
    pending.clear();
    despawn_owned(
        &roots,
        &mut commands,
        SystemDialogKind::GuildAllianceInvite,
        correlation,
    );
}

pub(crate) fn show_pending_relation_confirmation(
    pending: Res<PendingRelationConfirmation>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(action) = pending.action.as_ref() else {
        return;
    };
    if pending.choice.is_some() || !existing.is_empty() {
        return;
    }
    let (kind, title, message, button_label) = match action {
        RelationConfirmationAction::BreakAlliance { guild_name, .. } => (
            SystemDialogKind::GuildAllianceBreak,
            "Break Alliance",
            format!("Break the alliance with {guild_name}?"),
            "Break Alliance",
        ),
        RelationConfirmationAction::DeclareAntagonist { target_name } => (
            SystemDialogKind::GuildAntagonistDeclare,
            "Declare Antagonist",
            format!(
                "Declare the guild represented by {target_name} an antagonist? Any existing alliance will be broken for both guilds."
            ),
            "Declare Antagonist",
        ),
    };
    dialogs.write(ShowSystemDialog {
        severity: DialogSeverity::Warn,
        kind,
        kicker: "Guild Relations".into(),
        title: title.into(),
        message,
        code: String::new(),
        button_label: button_label.into(),
        secondary_label: "Cancel".into(),
        confirm_state: None,
        correlation: pending.correlation,
    });
}

pub(crate) fn claim_relation_confirmation(
    mut choices: MessageReader<SystemDialogChoice>,
    mut context: GuildMutationContext,
    mut pending: ResMut<PendingRelationConfirmation>,
    mut breaks: MessageWriter<GuildAllianceBreakRequested>,
    mut antagonists: MessageWriter<GuildAntagonistRequested>,
) {
    let Some(kind) = pending.kind() else {
        return;
    };
    if let Some(choice) = choices
        .read()
        .filter(|choice| choice.kind == kind && choice.correlation == pending.correlation)
        .last()
    {
        pending.choice = Some(choice.primary);
    }
    let Some(confirmed) = pending.choice else {
        return;
    };
    if !confirmed {
        pending.clear();
        return;
    }
    if !interaction_is_valid(
        &context.guild,
        &context.session,
        *context.generation,
        pending.generation,
        pending.own_guild_id,
    ) {
        pending.clear();
        return;
    }
    if context.ui.pending.is_some() {
        return;
    }
    match pending.action.as_ref().unwrap() {
        RelationConfirmationAction::BreakAlliance { guild_id, .. } => {
            let valid = context.guild.info().is_some_and(|info| {
                info.relations.iter().any(|relation| {
                    relation.guild_id == *guild_id && relation.kind == GuildRelationKind::Ally
                })
            });
            if !valid {
                context.ui.feedback = Some("Alliance changed; breaking it was cancelled.".into());
                context.ui.feedback_is_error = true;
                pending.clear();
                return;
            }
            breaks.write(GuildAllianceBreakRequested {
                guild_id: *guild_id,
            });
            context.ui.pending = Some(PendingGuildMutation {
                action: "alliance_break",
                generation: *context.generation,
            });
            context.ui.feedback = Some("Breaking alliance…".into());
        }
        RelationConfirmationAction::DeclareAntagonist { target_name } => {
            let target_name = target_name.trim();
            if target_name.is_empty() {
                pending.clear();
                return;
            }
            antagonists.write(GuildAntagonistRequested {
                target_char_id: 0,
                target_name: target_name.into(),
            });
            context.ui.pending = Some(PendingGuildMutation {
                action: "antagonist",
                generation: *context.generation,
            });
            context.ui.feedback = Some("Declaring antagonist…".into());
        }
    }
    context.ui.feedback_is_error = false;
    pending.clear();
}

pub(crate) fn close_finished_relation_dialogs(
    invite: Res<PendingAllianceInvite>,
    confirmation: Res<PendingRelationConfirmation>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !invite.is_pending() {
        despawn_owned(
            &roots,
            &mut commands,
            SystemDialogKind::GuildAllianceInvite,
            invite.correlation,
        );
    }
    if !confirmation.is_pending() {
        for kind in [
            SystemDialogKind::GuildAllianceBreak,
            SystemDialogKind::GuildAntagonistDeclare,
        ] {
            despawn_owned(&roots, &mut commands, kind, confirmation.correlation);
        }
    }
}

pub(crate) fn clear_relation_dialogs(
    mut invite: ResMut<PendingAllianceInvite>,
    mut confirmation: ResMut<PendingRelationConfirmation>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    let invite_correlation = invite.correlation;
    let confirmation_kind = confirmation.kind();
    let confirmation_correlation = confirmation.correlation;
    invite.clear();
    confirmation.clear();
    despawn_owned(
        &roots,
        &mut commands,
        SystemDialogKind::GuildAllianceInvite,
        invite_correlation,
    );
    if let Some(kind) = confirmation_kind {
        despawn_owned(&roots, &mut commands, kind, confirmation_correlation);
    }
}

fn despawn_owned(
    roots: &Query<(Entity, &SystemDialogRoot)>,
    commands: &mut Commands,
    kind: SystemDialogKind,
    correlation: Option<u64>,
) {
    for (entity, root) in roots {
        if root.matches(kind, correlation) {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use game_engine::domain::guild::{GuildPlugin, GuildSystems};
    use net_contract::dto::{GuildInfo, GuildRelationInfo};
    use net_contract::events::ZoneDisconnected;

    use super::*;

    fn guild_info(guild_id: u32, master_char_id: u32) -> GuildInfo {
        GuildInfo {
            guild_id,
            name: "Vikings".into(),
            master_char_id,
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
            relations: vec![GuildRelationInfo {
                guild_id: 8,
                name: "Aesir".into(),
                kind: GuildRelationKind::Ally,
            }],
        }
    }

    fn invite() -> GuildAllianceInviteInfo {
        GuildAllianceInviteInfo {
            guild_id: 8,
            guild_name: "Aesir".into(),
            requester_name: "Freya".into(),
        }
    }

    fn app() -> App {
        let generation = ZoneSessionGeneration(9);
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .add_message::<ShowSystemDialog>()
            .add_message::<SystemDialogChoice>()
            .add_message::<GuildAllianceResponded>()
            .add_message::<GuildAllianceBreakRequested>()
            .add_message::<GuildAntagonistRequested>()
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
            .init_resource::<GuildUi>()
            .init_resource::<super::super::emblem::GuildEmblemPreview>()
            .init_resource::<PendingAllianceInvite>()
            .init_resource::<PendingRelationConfirmation>()
            .init_resource::<Time>()
            .add_plugins(GuildPlugin)
            .add_systems(
                Update,
                (
                    reset_invalid_relation_dialogs,
                    super::super::feedback::apply_guild_results,
                    expire_pending_alliance,
                    queue_incoming_alliance,
                    claim_alliance_choice,
                    claim_relation_confirmation,
                )
                    .chain()
                    .in_set(GuildSystems::UiSync),
            )
            .add_systems(
                PostUpdate,
                (
                    show_pending_alliance,
                    show_pending_relation_confirmation,
                    close_finished_relation_dialogs,
                )
                    .chain(),
            );
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(guild_info(7, 42)),
        });
        app.update();
        app
    }

    fn notify(app: &mut App) {
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::AllianceRequestNotified(invite()),
        });
        app.update();
    }

    fn choose(app: &mut App, primary: bool) {
        let token = app.world().resource::<PendingAllianceInvite>().correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary,
            kind: SystemDialogKind::GuildAllianceInvite,
            correlation: token,
        });
        app.update();
    }

    fn alliance_responses(app: &App) -> Vec<GuildAllianceResponded> {
        let messages = app.world().resource::<Messages<GuildAllianceResponded>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn foreign_requesting_guild_is_accepted_for_the_recipient_own_guild() {
        let mut app = app();
        notify(&mut app);

        choose(&mut app, true);

        let responses = alliance_responses(&app);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].guild_id, 8);
        assert!(responses[0].accept);
        assert_eq!(
            app.world()
                .resource::<GuildUi>()
                .pending
                .as_ref()
                .unwrap()
                .action,
            "alliance_response"
        );
    }

    #[test]
    fn incoming_alliance_decline_sends_the_actual_choice() {
        let mut app = app();
        notify(&mut app);

        choose(&mut app, false);

        let responses = alliance_responses(&app);
        assert_eq!(responses.len(), 1);
        assert!(!responses[0].accept);
    }

    #[test]
    fn chosen_response_waits_for_an_existing_mutation_slot_without_reshowing() {
        let mut app = app();
        notify(&mut app);
        app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
            action: "skill_up",
            generation: ZoneSessionGeneration(9),
        });
        let before = app.world().resource::<Messages<ShowSystemDialog>>().len();

        choose(&mut app, true);

        assert!(alliance_responses(&app).is_empty());
        assert_eq!(
            app.world().resource::<Messages<ShowSystemDialog>>().len(),
            before
        );
        assert_eq!(
            app.world().resource::<PendingAllianceInvite>().choice,
            Some(true)
        );

        app.world_mut().resource_mut::<GuildUi>().pending = None;
        app.update();
        assert_eq!(alliance_responses(&app).len(), 1);
    }

    #[test]
    fn retained_choice_expires_before_released_mutation_can_dispatch_it() {
        use net_contract::dto::{GuildActionResult, GuildErrorKind};

        let mut app = app();
        notify(&mut app);
        app.world_mut().resource_mut::<GuildUi>().pending = Some(PendingGuildMutation {
            action: "skill_up",
            generation: ZoneSessionGeneration(9),
        });
        let token = app.world().resource::<PendingAllianceInvite>().correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildAllianceInvite,
            correlation: token,
        });
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(ALLIANCE_INVITE_TTL_SECS - 0.1));
        app.update();
        assert_eq!(
            app.world().resource::<PendingAllianceInvite>().choice,
            Some(true)
        );

        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "skill_up".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        });
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(0.2));
        app.update();

        assert!(alliance_responses(&app).is_empty());
        assert!(!app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(app.world().resource::<GuildUi>().pending.is_none());
    }

    #[test]
    fn queued_invite_expires_while_an_unrelated_modal_is_busy_without_sending() {
        let mut app = app();
        let occupied = app.world_mut().spawn(SystemDialogRoot::default()).id();
        notify(&mut app);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(ALLIANCE_INVITE_TTL_SECS + 1.0));

        app.update();

        assert!(!app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(alliance_responses(&app).is_empty());
        assert!(app.world().get_entity(occupied).is_ok());
    }

    #[test]
    fn hidden_guild_window_does_not_stop_alliance_notification_capture() {
        let mut app = app();
        app.world_mut()
            .spawn((super::super::GuildWindowRoot, Visibility::Hidden));

        notify(&mut app);

        assert!(app.world().resource::<PendingAllianceInvite>().is_pending());
    }

    #[test]
    fn receipt_frame_does_not_charge_the_preceding_frame_delta() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(ALLIANCE_INVITE_TTL_SECS + 1.0));

        notify(&mut app);

        assert!(app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(alliance_responses(&app).is_empty());
    }

    #[test]
    fn stale_alliance_choice_cannot_claim_a_new_request() {
        let mut app = app();
        let stale = {
            let mut pending = app.world_mut().resource_mut::<PendingAllianceInvite>();
            pending.set(invite(), ZoneSessionGeneration(9), 7);
            let stale = pending.correlation;
            pending.clear();
            pending.set(invite(), ZoneSessionGeneration(9), 7);
            stale
        };
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildAllianceInvite,
            correlation: stale,
        });

        app.update();

        assert!(alliance_responses(&app).is_empty());
        assert!(app.world().resource::<PendingAllianceInvite>().is_pending());
    }

    #[test]
    fn generation_change_discards_pending_invite_without_a_response() {
        let mut app = app();
        notify(&mut app);
        app.insert_resource(ZoneSessionGeneration(10));

        app.update();

        assert!(!app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(alliance_responses(&app).is_empty());
    }

    #[test]
    fn own_guild_change_discards_foreign_request_bound_to_the_previous_guild() {
        let mut app = app();
        notify(&mut app);
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild_info(11, 42)),
        });

        app.update();

        assert!(!app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(alliance_responses(&app).is_empty());
    }

    #[test]
    fn demotion_discards_pending_invite_and_owned_confirmation() {
        let mut app = app();
        notify(&mut app);
        app.world_mut()
            .resource_mut::<PendingRelationConfirmation>()
            .declare_antagonist(ZoneSessionGeneration(9), 7, "Loki".into());
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild_info(7, 99)),
        });

        app.update();

        assert!(!app.world().resource::<PendingAllianceInvite>().is_pending());
        assert!(
            !app.world()
                .resource::<PendingRelationConfirmation>()
                .is_pending()
        );
    }

    #[test]
    fn confirmed_break_revalidates_and_sends_selected_guild_id() {
        let mut app = app();
        let token = {
            let mut pending = app
                .world_mut()
                .resource_mut::<PendingRelationConfirmation>();
            pending.break_alliance(ZoneSessionGeneration(9), 7, 8, "Aesir".into());
            pending.correlation
        };
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildAllianceBreak,
            correlation: token,
        });

        app.update();

        let messages = app
            .world()
            .resource::<Messages<GuildAllianceBreakRequested>>();
        let mut cursor = messages.get_cursor();
        let sent: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].guild_id, 8);
    }

    #[test]
    fn antagonist_warning_and_confirmation_use_character_name_targeting() {
        let mut app = app();
        let token = {
            let mut pending = app
                .world_mut()
                .resource_mut::<PendingRelationConfirmation>();
            pending.declare_antagonist(ZoneSessionGeneration(9), 7, "  Loki  ".into());
            pending.correlation
        };
        app.update();
        let dialogs = app.world().resource::<Messages<ShowSystemDialog>>();
        let mut cursor = dialogs.get_cursor();
        assert!(
            cursor
                .read(dialogs)
                .any(|dialog| dialog.message.contains("broken for both guilds"))
        );
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildAntagonistDeclare,
            correlation: token,
        });

        app.update();

        let messages = app.world().resource::<Messages<GuildAntagonistRequested>>();
        let mut cursor = messages.get_cursor();
        let sent: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].target_char_id, 0);
        assert_eq!(sent[0].target_name, "Loki");
    }

    #[test]
    fn cancelling_destructive_confirmation_sends_nothing() {
        let mut app = app();
        let token = {
            let mut pending = app
                .world_mut()
                .resource_mut::<PendingRelationConfirmation>();
            pending.break_alliance(ZoneSessionGeneration(9), 7, 8, "Aesir".into());
            pending.correlation
        };
        app.world_mut().write_message(SystemDialogChoice {
            primary: false,
            kind: SystemDialogKind::GuildAllianceBreak,
            correlation: token,
        });

        app.update();

        assert!(
            app.world()
                .resource::<Messages<GuildAllianceBreakRequested>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<GuildAntagonistRequested>>()
                .is_empty()
        );
    }
}
