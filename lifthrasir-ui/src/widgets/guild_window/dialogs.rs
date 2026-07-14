use bevy::prelude::*;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use net_contract::commands::GuildInviteResponded;
use net_contract::dto::GuildInviteInfo;
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::ZoneSessionGeneration;

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
