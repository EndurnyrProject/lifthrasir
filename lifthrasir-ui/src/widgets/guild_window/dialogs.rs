use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use game_engine::domain::guild::GuildState;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use net_contract::commands::{GuildExpelRequested, GuildInviteResponded, GuildLeaveRequested};
use net_contract::dto::GuildInviteInfo;
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::ZoneSessionGeneration;

use super::{GuildMutationContext, GuildUi, PendingGuildMutation};
use crate::widgets::system_dialog::SystemDialogRoot;

const INVITE_TTL_SECS: f32 = 30.0;

#[derive(Resource, Default)]
pub struct PendingGuildInvite {
    invite: Option<GuildInviteInfo>,
    generation: ZoneSessionGeneration,
    timer: Timer,
    correlation: Option<u64>,
    next_correlation: u64,
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct PendingGuildConfirmation {
    pub(crate) kind: Option<SystemDialogKind>,
    pub(crate) generation: ZoneSessionGeneration,
    pub(crate) target_char_id: u32,
    pub(crate) target_name: String,
    pub(crate) reason: String,
    pub(crate) master_disband: bool,
    pub(crate) correlation: Option<u64>,
    next_correlation: u64,
}

impl PendingGuildConfirmation {
    pub(crate) fn is_pending(&self) -> bool {
        self.kind.is_some()
    }

    pub(crate) fn leave(&mut self, generation: ZoneSessionGeneration, master: bool) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.kind = Some(SystemDialogKind::GuildLeave);
        self.generation = generation;
        self.target_char_id = 0;
        self.target_name.clear();
        self.reason.clear();
        self.master_disband = master;
        self.correlation = Some(self.next_correlation);
    }

    pub(crate) fn expel(
        &mut self,
        generation: ZoneSessionGeneration,
        target_char_id: u32,
        target_name: &str,
        reason: &str,
    ) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.kind = Some(SystemDialogKind::GuildExpel);
        self.generation = generation;
        self.target_char_id = target_char_id;
        self.target_name = target_name.to_string();
        self.reason = reason.to_string();
        self.master_disband = false;
        self.correlation = Some(self.next_correlation);
    }

    pub(crate) fn clear(&mut self) {
        self.kind = None;
        self.target_char_id = 0;
        self.target_name.clear();
        self.reason.clear();
        self.master_disband = false;
    }
}

impl PendingGuildInvite {
    pub fn is_pending(&self) -> bool {
        self.invite.is_some()
    }

    fn set(&mut self, invite: GuildInviteInfo, generation: ZoneSessionGeneration) {
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        self.invite = Some(invite);
        self.generation = generation;
        self.timer = Timer::from_seconds(INVITE_TTL_SECS, TimerMode::Once);
        self.correlation = Some(self.next_correlation);
    }

    fn clear(&mut self) {
        self.invite = None;
        self.timer = Timer::default();
    }
}

pub fn reset_stale_invite(
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingGuildInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() || pending.generation == *generation {
        return;
    }
    let old_correlation = pending.correlation;
    pending.clear();
    for (entity, root) in &roots {
        if root.matches(SystemDialogKind::GuildInvite, old_correlation) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn queue_incoming_invite(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingGuildInvite>,
) {
    let mut newest = None;
    for event in ingress.read() {
        if event.generation != *generation {
            continue;
        }
        if let GuildIngressPayload::InviteNotified(invite) = &event.payload {
            newest = Some(invite.clone());
        }
    }
    if !pending.is_pending() {
        if let Some(invite) = newest {
            pending.set(invite, *generation);
        }
    }
}

/// Runs after Update's shared-dialog consumer. A busy dialog keeps the invite queued;
/// an unclaimed request is retried next frame with the same correlation token.
pub fn show_pending_invite(
    pending: Res<PendingGuildInvite>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(invite) = pending.invite.as_ref() else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    dialogs.write(ShowSystemDialog {
        severity: DialogSeverity::Info,
        kind: SystemDialogKind::GuildInvite,
        kicker: "Guild".into(),
        title: "Guild Invite".into(),
        message: format!(
            "{} invites you to {}.",
            invite.inviter_name, invite.guild_name
        ),
        code: String::new(),
        button_label: "Accept".into(),
        secondary_label: "Decline".into(),
        confirm_state: None,
        correlation: pending.correlation,
    });
}

pub fn claim_invite_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingGuildInvite>,
    mut responses: MessageWriter<GuildInviteResponded>,
) {
    if !pending.is_pending() || pending.generation != *generation {
        return;
    }
    let Some(choice) = choices
        .read()
        .filter(|choice| {
            choice.kind == SystemDialogKind::GuildInvite
                && choice.correlation == pending.correlation
        })
        .last()
    else {
        return;
    };
    let guild_id = pending.invite.as_ref().unwrap().guild_id;
    responses.write(GuildInviteResponded {
        guild_id,
        accept: choice.primary,
    });
    pending.clear();
}

pub fn expire_pending_invite(
    time: Res<Time>,
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingGuildInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() {
        return;
    }
    let stale = pending.generation != *generation;
    if !stale && !pending.timer.tick(time.delta()).just_finished() {
        return;
    }
    let correlation = pending.correlation;
    pending.clear();
    if let Some((entity, _)) = roots
        .iter()
        .find(|(_, root)| root.matches(SystemDialogKind::GuildInvite, correlation))
    {
        commands.entity(entity).despawn();
    }
}

pub fn clear_pending_invite(
    mut pending: ResMut<PendingGuildInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    let correlation = pending.correlation;
    pending.clear();
    for (entity, root) in &roots {
        if root.matches(SystemDialogKind::GuildInvite, correlation) {
            commands.entity(entity).despawn();
        }
    }
}

/// A queued dialog request can be consumed after expiry or a state transition. Keep the
/// last token long enough to remove only that orphan if it appears later.
pub fn close_finished_invite_dialog(
    pending: Res<PendingGuildInvite>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if pending.is_pending() {
        return;
    }
    for (entity, root) in &roots {
        if root.matches(SystemDialogKind::GuildInvite, pending.correlation) {
            commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn on_leave(
    _: On<Activate>,
    guild: Res<GuildState>,
    session: Res<net_contract::state::ZoneSession>,
    generation: Res<ZoneSessionGeneration>,
    ui: Res<GuildUi>,
    mut confirmation: ResMut<PendingGuildConfirmation>,
) {
    if !guild.in_guild() || ui.pending.is_some() || confirmation.is_pending() {
        return;
    }
    confirmation.leave(*generation, guild.is_master(session.char_id));
}

pub(crate) fn show_pending_confirmation(
    pending: Res<PendingGuildConfirmation>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut dialogs: MessageWriter<ShowSystemDialog>,
) {
    let Some(kind) = pending.kind else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    let (title, message, button_label) = confirmation_copy(&pending, kind);
    let message = if kind == SystemDialogKind::GuildExpel {
        format!(
            "Are you sure you want to expel {}?\nReason: {}",
            pending.target_name, pending.reason
        )
    } else {
        message.to_string()
    };
    dialogs.write(ShowSystemDialog {
        severity: if kind == SystemDialogKind::GuildExpel {
            DialogSeverity::Warn
        } else {
            DialogSeverity::Error
        },
        kind,
        kicker: "Guild".into(),
        title: title.into(),
        message,
        code: String::new(),
        button_label: button_label.into(),
        secondary_label: "Cancel".into(),
        confirm_state: None,
        correlation: pending.correlation,
    });
}

fn confirmation_copy(
    pending: &PendingGuildConfirmation,
    kind: SystemDialogKind,
) -> (&'static str, &'static str, &'static str) {
    match kind {
        SystemDialogKind::GuildLeave if pending.master_disband => (
            "Disband Guild",
            "Leaving will disband the guild. Are you sure you want to continue?",
            "Leave and Disband",
        ),
        SystemDialogKind::GuildLeave => (
            "Leave Guild",
            "Are you sure you want to leave the guild?",
            "Leave Guild",
        ),
        SystemDialogKind::GuildExpel => ("Expel Guild Member", "", "Expel Member"),
        _ => ("Guild Action", "Confirm this guild action?", "Confirm"),
    }
}

pub(crate) fn claim_confirmation_choice(
    mut choices: MessageReader<SystemDialogChoice>,
    mut context: GuildMutationContext,
    mut confirmation: ResMut<PendingGuildConfirmation>,
    mut leave: MessageWriter<GuildLeaveRequested>,
    mut expel: MessageWriter<GuildExpelRequested>,
) {
    if !confirmation.is_pending() || confirmation.generation != *context.generation {
        return;
    }
    let Some(choice) = choices
        .read()
        .filter(|choice| {
            Some(choice.kind) == confirmation.kind && choice.correlation == confirmation.correlation
        })
        .last()
        .cloned()
    else {
        return;
    };
    if choice.primary {
        match confirmation.kind {
            Some(SystemDialogKind::GuildLeave) => {
                let still_member = context.guild.in_guild()
                    && context.guild.member(context.session.char_id).is_some();
                let warning_still_matches =
                    confirmation.master_disband == context.guild.is_master(context.session.char_id);
                if !still_member || !warning_still_matches {
                    context.ui.feedback =
                        Some("Guild membership changed; leaving was cancelled.".into());
                    context.ui.feedback_is_error = true;
                } else if context.ui.pending.is_none() {
                    context.ui.pending = Some(PendingGuildMutation {
                        action: "leave",
                        generation: *context.generation,
                    });
                    context.ui.feedback = Some("Leaving guild…".into());
                    context.ui.feedback_is_error = false;
                    leave.write(GuildLeaveRequested);
                }
            }
            Some(SystemDialogKind::GuildExpel) => {
                let reason = confirmation.reason.trim();
                let target = confirmation.target_char_id;
                let valid = !reason.is_empty()
                    && target != 0
                    && target != context.session.char_id
                    && context.guild.member(context.session.char_id).is_some()
                    && context.guild.can_expel(context.session.char_id)
                    && context.guild.member(target).is_some()
                    && !context.guild.is_master(target);
                if !valid {
                    context.ui.feedback = Some(
                        "Guild membership or permissions changed; expulsion was cancelled.".into(),
                    );
                    context.ui.feedback_is_error = true;
                } else if context.ui.pending.is_none() {
                    context.ui.pending = Some(PendingGuildMutation {
                        action: "expel",
                        generation: *context.generation,
                    });
                    context.ui.feedback = Some("Expelling guild member…".into());
                    context.ui.feedback_is_error = false;
                    expel.write(GuildExpelRequested {
                        target_char_id: target,
                        reason: reason.to_string(),
                    });
                }
            }
            _ => {}
        }
    }
    confirmation.clear();
}

pub(crate) fn reset_stale_confirmation(
    generation: Res<ZoneSessionGeneration>,
    mut pending: ResMut<PendingGuildConfirmation>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    if !pending.is_pending() || pending.generation == *generation {
        return;
    }
    let kind = pending.kind;
    let correlation = pending.correlation;
    pending.clear();
    if let Some(kind) = kind {
        for (entity, root) in &roots {
            if root.matches(kind, correlation) {
                commands.entity(entity).despawn();
            }
        }
    }
}

pub(crate) fn clear_pending_confirmation(
    mut pending: ResMut<PendingGuildConfirmation>,
    roots: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
) {
    let kind = pending.kind;
    let correlation = pending.correlation;
    pending.clear();
    if let Some(kind) = kind {
        for (entity, root) in &roots {
            if root.matches(kind, correlation) {
                commands.entity(entity).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::domain::guild::{GuildPlugin, GuildSystems};
    use net_contract::dto::{GuildInfo, GuildMemberInfo, GuildPositionInfo};
    use std::time::Duration;

    fn invite() -> GuildInviteInfo {
        GuildInviteInfo {
            guild_id: 7,
            guild_name: "Vikings".into(),
            inviter_name: "Odin".into(),
        }
    }

    fn ingress(generation: ZoneSessionGeneration) -> GuildIngress {
        GuildIngress {
            generation,
            payload: GuildIngressPayload::InviteNotified(invite()),
        }
    }

    fn member(char_id: u32, position_index: u32, name: &str) -> GuildMemberInfo {
        GuildMemberInfo {
            char_id,
            name: name.into(),
            job_id: 1,
            base_level: 50,
            online: true,
            map: "prontera".into(),
            position_index,
            hp: 100,
            max_hp: 100,
            sp: 50,
            max_sp: 50,
            ap: 0,
            max_ap: 0,
        }
    }

    fn guild_info(master_char_id: u32, master_can_expel: bool) -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id,
            emblem_id: 1,
            notice_subject: String::new(),
            notice_body: String::new(),
            positions: vec![
                GuildPositionInfo {
                    index: 0,
                    name: "Master".into(),
                    can_invite: true,
                    can_expel: master_can_expel,
                    can_storage: false,
                    tax: 0,
                },
                GuildPositionInfo {
                    index: 1,
                    name: "Member".into(),
                    can_invite: false,
                    can_expel: false,
                    can_storage: false,
                    tax: 0,
                },
            ],
            members: vec![member(42, 0, "Odin"), member(43, 1, "Thor")],
        }
    }

    fn confirmation_app() -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<net_contract::events::ZoneDisconnected>()
            .add_message::<SystemDialogChoice>()
            .add_message::<GuildLeaveRequested>()
            .add_message::<GuildExpelRequested>()
            .insert_resource(ZoneSessionGeneration(2))
            .insert_resource(net_contract::state::ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(GuildUi::default())
            .init_resource::<PendingGuildConfirmation>()
            .add_plugins(GuildPlugin)
            .add_systems(
                Update,
                claim_confirmation_choice.in_set(GuildSystems::UiSync),
            );
        app
    }

    fn set_guild(app: &mut App, info: GuildInfo) {
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(2),
            payload: GuildIngressPayload::Info(info),
        });
        app.update();
    }

    fn accept_confirmation(app: &mut App) {
        let token = app
            .world()
            .resource::<PendingGuildConfirmation>()
            .correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildExpel,
            correlation: token,
        });
        app.update();
    }

    fn dialog_app() -> App {
        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .add_message::<ShowSystemDialog>()
            .add_message::<SystemDialogChoice>()
            .add_message::<GuildInviteResponded>()
            .insert_resource(ZoneSessionGeneration(9))
            .init_resource::<Time>()
            .init_resource::<PendingGuildInvite>()
            .add_systems(Update, (queue_incoming_invite, claim_invite_choice))
            .add_systems(
                PostUpdate,
                (
                    expire_pending_invite,
                    show_pending_invite,
                    close_finished_invite_dialog,
                )
                    .chain(),
            );
        app
    }

    fn shown_dialogs(app: &App) -> Vec<ShowSystemDialog> {
        let messages = app.world().resource::<Messages<ShowSystemDialog>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    fn responses(app: &App) -> Vec<GuildInviteResponded> {
        let messages = app.world().resource::<Messages<GuildInviteResponded>>();
        let mut cursor = messages.get_cursor();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn incoming_invite_is_retained_while_dialog_is_occupied() {
        let mut app = dialog_app();
        let occupied = app.world_mut().spawn(SystemDialogRoot::default()).id();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));

        app.update();

        assert!(app.world().resource::<PendingGuildInvite>().is_pending());
        assert!(shown_dialogs(&app).is_empty());

        app.world_mut().entity_mut(occupied).despawn();
        app.update();
        let dialogs = shown_dialogs(&app);
        assert_eq!(dialogs.len(), 1);
        assert_eq!(dialogs[0].kind, SystemDialogKind::GuildInvite);
        assert_eq!(
            dialogs[0].correlation,
            app.world().resource::<PendingGuildInvite>().correlation
        );
    }

    #[test]
    fn queued_invite_expires_without_replacing_occupied_dialog() {
        let mut app = dialog_app();
        let occupied = app.world_mut().spawn(SystemDialogRoot::default()).id();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));

        app.update();

        assert!(!app.world().resource::<PendingGuildInvite>().is_pending());
        assert!(app.world().get_entity(occupied).is_ok());
        assert!(shown_dialogs(&app).is_empty());
    }

    #[test]
    fn matching_accept_writes_response_without_changing_authoritative_state() {
        let mut app = dialog_app();
        app.init_resource::<game_engine::domain::guild::GuildState>();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));
        app.update();
        let token = app.world().resource::<PendingGuildInvite>().correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildInvite,
            correlation: token,
        });

        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].guild_id, 7);
        assert!(written[0].accept);
        assert!(!app
            .world()
            .resource::<game_engine::domain::guild::GuildState>()
            .in_guild());
    }

    #[test]
    fn matching_decline_writes_a_non_accept_response() {
        let mut app = dialog_app();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));
        app.update();
        let token = app.world().resource::<PendingGuildInvite>().correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: false,
            kind: SystemDialogKind::GuildInvite,
            correlation: token,
        });

        app.update();

        let written = responses(&app);
        assert_eq!(written.len(), 1);
        assert!(!written[0].accept);
    }

    #[test]
    fn stale_choice_cannot_claim_a_later_invite() {
        let mut app = dialog_app();
        let stale = {
            let mut pending = app.world_mut().resource_mut::<PendingGuildInvite>();
            pending.set(invite(), ZoneSessionGeneration(9));
            let stale = pending.correlation;
            pending.clear();
            pending.set(invite(), ZoneSessionGeneration(9));
            assert_ne!(stale, pending.correlation);
            stale
        };
        app.world_mut().write_message(SystemDialogChoice {
            primary: false,
            kind: SystemDialogKind::GuildInvite,
            correlation: stale,
        });

        app.update();

        assert!(responses(&app).is_empty());
        assert!(app.world().resource::<PendingGuildInvite>().is_pending());
    }

    #[test]
    fn displayed_expiry_closes_only_the_matching_guild_dialog() {
        let mut app = dialog_app();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));
        app.update();
        let token = app.world().resource::<PendingGuildInvite>().correlation;
        let owned = app
            .world_mut()
            .spawn(SystemDialogRoot::new(
                None,
                SystemDialogKind::GuildInvite,
                token,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));

        app.update();

        assert!(app.world().get_entity(owned).is_err());
        assert!(!app.world().resource::<PendingGuildInvite>().is_pending());
    }

    #[test]
    fn late_dialog_spawn_after_expiry_is_closed_by_its_token() {
        let mut app = dialog_app();
        app.world_mut()
            .write_message(ingress(ZoneSessionGeneration(9)));
        app.update();
        let token = app.world().resource::<PendingGuildInvite>().correlation;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(INVITE_TTL_SECS + 1.0));
        app.update();
        let late = app
            .world_mut()
            .spawn(SystemDialogRoot::new(
                None,
                SystemDialogKind::GuildInvite,
                token,
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(late).is_err());
    }

    #[test]
    fn generation_switch_replaces_stale_pending_with_new_invite_in_same_frame() {
        use game_engine::domain::guild::GuildSystems;

        let mut app = App::new();
        app.add_message::<GuildIngress>()
            .insert_resource(ZoneSessionGeneration(1))
            .init_resource::<PendingGuildInvite>()
            .configure_sets(
                Update,
                (GuildSystems::SessionReset, GuildSystems::UiSync).chain(),
            )
            .add_systems(
                Update,
                reset_stale_invite.in_set(GuildSystems::SessionReset),
            )
            .add_systems(Update, queue_incoming_invite.in_set(GuildSystems::UiSync));
        let old_token = {
            let mut pending = app.world_mut().resource_mut::<PendingGuildInvite>();
            pending.set(invite(), ZoneSessionGeneration(1));
            pending.correlation
        };
        let old_dialog = app
            .world_mut()
            .spawn(SystemDialogRoot::new(
                None,
                SystemDialogKind::GuildInvite,
                old_token,
            ))
            .id();
        app.insert_resource(ZoneSessionGeneration(2));
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(2),
            payload: GuildIngressPayload::InviteNotified(GuildInviteInfo {
                guild_id: 8,
                guild_name: "Aesir".into(),
                inviter_name: "Freya".into(),
            }),
        });

        app.update();

        let pending = app.world().resource::<PendingGuildInvite>();
        assert_eq!(pending.generation, ZoneSessionGeneration(2));
        assert_eq!(pending.invite.as_ref().unwrap().guild_id, 8);
        assert_ne!(pending.correlation, old_token);
        assert!(app.world().get_entity(old_dialog).is_err());
    }

    #[test]
    fn leave_confirmation_copy_distinguishes_master_disband_warning() {
        let mut pending = PendingGuildConfirmation::default();
        pending.leave(ZoneSessionGeneration(1), false);
        assert_eq!(
            confirmation_copy(&pending, SystemDialogKind::GuildLeave).1,
            "Are you sure you want to leave the guild?"
        );

        pending.leave(ZoneSessionGeneration(1), true);
        assert!(confirmation_copy(&pending, SystemDialogKind::GuildLeave)
            .1
            .contains("disband the guild"));
    }

    #[test]
    fn wrong_kind_or_token_confirmation_choice_writes_no_destructive_command() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>()
            .add_message::<GuildLeaveRequested>()
            .add_message::<GuildExpelRequested>()
            .insert_resource(ZoneSessionGeneration(2))
            .insert_resource(net_contract::state::ZoneSession::default())
            .insert_resource(GuildState::default())
            .insert_resource(GuildUi::default())
            .insert_resource(PendingGuildConfirmation::default())
            .add_systems(Update, claim_confirmation_choice);
        {
            let mut pending = app.world_mut().resource_mut::<PendingGuildConfirmation>();
            pending.leave(ZoneSessionGeneration(2), false);
        }
        let token = app
            .world()
            .resource::<PendingGuildConfirmation>()
            .correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildExpel,
            correlation: token,
        });
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildLeave,
            correlation: token.map(|token| token + 1),
        });
        app.update();

        let leaves = app.world().resource::<Messages<GuildLeaveRequested>>();
        let expels = app.world().resource::<Messages<GuildExpelRequested>>();
        assert_eq!(leaves.len(), 0);
        assert_eq!(expels.len(), 0);
        assert!(app
            .world()
            .resource::<PendingGuildConfirmation>()
            .is_pending());
    }

    #[test]
    fn revoked_expel_permission_between_prompt_and_acceptance_writes_no_command() {
        let mut app = confirmation_app();
        set_guild(&mut app, guild_info(42, true));
        app.world_mut()
            .resource_mut::<PendingGuildConfirmation>()
            .expel(ZoneSessionGeneration(2), 43, "Thor", "  reason  ");
        set_guild(&mut app, guild_info(42, false));

        accept_confirmation(&mut app);

        assert_eq!(
            app.world()
                .resource::<Messages<GuildExpelRequested>>()
                .len(),
            0
        );
        assert!(!app
            .world()
            .resource::<PendingGuildConfirmation>()
            .is_pending());
    }

    #[test]
    fn promoted_target_between_prompt_and_acceptance_writes_no_command() {
        let mut app = confirmation_app();
        set_guild(&mut app, guild_info(42, true));
        app.world_mut()
            .resource_mut::<PendingGuildConfirmation>()
            .expel(ZoneSessionGeneration(2), 43, "Thor", "reason");
        set_guild(&mut app, guild_info(43, true));

        accept_confirmation(&mut app);

        assert_eq!(
            app.world()
                .resource::<Messages<GuildExpelRequested>>()
                .len(),
            0
        );
    }

    #[test]
    fn self_target_between_prompt_and_acceptance_writes_no_command() {
        let mut app = confirmation_app();
        set_guild(&mut app, guild_info(42, true));
        app.world_mut()
            .resource_mut::<PendingGuildConfirmation>()
            .expel(ZoneSessionGeneration(2), 42, "Odin", "reason");

        accept_confirmation(&mut app);

        assert_eq!(
            app.world()
                .resource::<Messages<GuildExpelRequested>>()
                .len(),
            0
        );
    }

    #[test]
    fn valid_expel_trims_reason_and_writes_exact_command() {
        let mut app = confirmation_app();
        set_guild(&mut app, guild_info(42, true));
        app.world_mut()
            .resource_mut::<PendingGuildConfirmation>()
            .expel(ZoneSessionGeneration(2), 43, "Thor", "  too noisy  ");

        accept_confirmation(&mut app);

        let messages = app.world().resource::<Messages<GuildExpelRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).cloned().collect();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].target_char_id, 43);
        assert_eq!(written[0].reason, "too noisy");
    }

    #[test]
    fn master_role_change_after_leave_prompt_requires_a_new_warning() {
        let mut app = confirmation_app();
        let mut initial = guild_info(43, true);
        initial.members[0].position_index = 1;
        initial.members[1].position_index = 0;
        set_guild(&mut app, initial);
        app.world_mut()
            .resource_mut::<PendingGuildConfirmation>()
            .leave(ZoneSessionGeneration(2), false);

        set_guild(&mut app, guild_info(42, true));
        let token = app
            .world()
            .resource::<PendingGuildConfirmation>()
            .correlation;
        app.world_mut().write_message(SystemDialogChoice {
            primary: true,
            kind: SystemDialogKind::GuildLeave,
            correlation: token,
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<GuildLeaveRequested>>()
                .len(),
            0
        );
    }
}
