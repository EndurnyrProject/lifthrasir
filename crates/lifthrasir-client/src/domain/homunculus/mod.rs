//! Reconciles owner snapshots independently of public companion actors.

use std::time::Duration;

use bevy::prelude::*;
use net_contract::{
    commands::{HomunculusCommand, LeaveZone},
    dto::{HomunculusAction as Action, HomunculusError, HomunculusLifecycle, HomunculusSnapshot},
    events::{
        HomunculusIngress, HomunculusIngressPayload as Ingress, HomunculusReady,
        HomunculusSendFailed, ZoneDisconnected,
    },
    state::ZoneSessionGeneration,
};

use crate::domain::{
    entities::{markers::Homunculus, registry::EntityRegistry},
    system_sets::EntityLifecycleSystems,
};

mod skills;
pub use skills::{HomunculusCastIntent, HomunculusSkillSpec, homunculus_skill_spec};

const TIMEOUT: Duration = Duration::from_secs(5);

pub struct HomunculusPlugin;
impl Plugin for HomunculusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HomunculusState>()
            .add_message::<HomunculusIntent>()
            .add_message::<HomunculusFeedback>()
            .add_message::<HomunculusCastIntent>()
            .add_systems(
                Update,
                skills::resolve_cast
                    .run_if(resource_exists::<crate::domain::input::TargetingMode>)
                    .after(resolve_actor)
                    .before(dispatch),
            )
            .add_systems(
                Update,
                (receive, resolve_actor, dispatch)
                    .chain()
                    .after(EntityLifecycleSystems::Despawning),
            );
    }
}

#[derive(Debug, Default)]
pub enum HomunculusPresence {
    #[default]
    Unknown,
    Absent,
    Present(Box<HomunculusSnapshot>),
}

#[derive(Message, Debug, Clone)]
pub enum HomunculusIntent {
    Inspect,
    WindowVisibility(bool),
    Act { durable_id: u64, action: Action },
}

#[derive(Message, Debug, Clone)]
pub struct HomunculusFeedback {
    pub request_id: u64,
    pub kind: HomunculusFeedbackKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomunculusFeedbackKind {
    Completed,
    Rejected(HomunculusError),
    SendFailed(String),
    TimedOut,
    Removed,
}

struct Pending {
    id: u64,
    deadline: Duration,
    may_remove: bool,
}

#[derive(Resource, Default)]
pub struct HomunculusState {
    generation: ZoneSessionGeneration,
    presence: HomunculusPresence,
    last_state: Option<u32>,
    map_floor: Option<u32>,
    ready: bool,
    window_visible: bool,
    wants_inspect: bool,
    recovering: bool,
    serial: u64,
    inspect: Option<Pending>,
    mutation: Option<Pending>,
    actor: Option<(u32, Entity)>,
    snapshot_received: Duration,
    last_feedback: Option<HomunculusFeedbackKind>,
}

impl HomunculusState {
    pub fn presence(&self) -> &HomunculusPresence {
        &self.presence
    }
    pub fn snapshot(&self) -> Option<&HomunculusSnapshot> {
        match &self.presence {
            HomunculusPresence::Present(state) => Some(state),
            _ => None,
        }
    }
    pub fn controlled_gid(&self) -> Option<u32> {
        self.actor.map(|(gid, _)| gid)
    }
    pub fn controlled_actor(&self) -> Option<Entity> {
        self.actor.map(|(_, entity)| entity)
    }
    pub fn pending_mutation(&self) -> Option<u64> {
        self.mutation.as_ref().map(|p| p.id)
    }
    pub fn can_mutate(&self) -> bool {
        self.ready && !self.recovering && self.mutation.is_none()
    }
    pub fn last_feedback(&self) -> Option<&HomunculusFeedbackKind> {
        self.last_feedback.as_ref()
    }
    pub fn remaining(&self, ms: u64, now: Duration) -> u64 {
        if !self.ready
            || !self
                .snapshot()
                .is_some_and(|s| s.lifecycle == HomunculusLifecycle::Active)
        {
            return ms;
        }
        ms.saturating_sub(
            now.saturating_sub(self.snapshot_received)
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
        )
    }

    fn feedback(
        &mut self,
        id: u64,
        kind: HomunculusFeedbackKind,
        out: &mut MessageWriter<HomunculusFeedback>,
    ) {
        self.last_feedback = Some(kind.clone());
        out.write(HomunculusFeedback {
            request_id: id,
            kind,
        });
    }

    fn map_changed(&mut self, sequence: u32) {
        if self.map_floor.is_some_and(|floor| floor >= sequence) {
            return;
        }
        self.map_floor = Some(sequence);
        self.ready = false;
        self.actor = None;
        self.inspect = None;
        self.mutation = None;
        self.recovering = false;
        self.last_feedback = None;
        self.wants_inspect |= self.window_visible || self.snapshot().is_some();
        if self.last_state.is_some_and(|last| last <= sequence) {
            self.presence = HomunculusPresence::Unknown;
        }
    }

    fn take_request(&mut self, id: u64) -> Option<Pending> {
        if self.inspect.as_ref().is_some_and(|p| p.id == id) {
            self.inspect.take()
        } else if self.mutation.as_ref().is_some_and(|p| p.id == id) {
            self.mutation.take()
        } else {
            None
        }
    }

    fn apply_ingress(
        &mut self,
        msg: &HomunculusIngress,
        now: Duration,
        out: &mut MessageWriter<HomunculusFeedback>,
    ) {
        if msg.generation != self.generation {
            return;
        }
        let sequence = msg.sequence;
        let after_map = self.map_floor.is_none_or(|floor| sequence > floor);
        let newer = after_map && self.last_state.is_none_or(|last| sequence > last);
        let update = match &msg.payload {
            Ingress::MapBarrier => {
                self.map_changed(sequence);
                return;
            }
            Ingress::State(snapshot) if newer => {
                Some(HomunculusPresence::Present(Box::new(snapshot.clone())))
            }
            Ingress::Removed { .. } if newer => {
                if !matches!(self.presence, HomunculusPresence::Absent) {
                    self.feedback(0, HomunculusFeedbackKind::Removed, out);
                }
                Some(HomunculusPresence::Absent)
            }
            Ingress::Result {
                request_id,
                success,
                error,
                state,
            } if after_map => {
                let inspecting = self.inspect.as_ref().is_some_and(|p| p.id == *request_id);
                let pending = self.take_request(*request_id);
                if pending.is_some() {
                    let kind = if !success {
                        HomunculusFeedbackKind::Rejected(*error)
                    } else if state.is_none() && pending.as_ref().is_some_and(|p| p.may_remove) {
                        HomunculusFeedbackKind::Removed
                    } else {
                        HomunculusFeedbackKind::Completed
                    };
                    if !inspecting || !success && *error != HomunculusError::NoCompanion {
                        self.feedback(*request_id, kind, out);
                    }
                    if inspecting && (state.is_some() || *error == HomunculusError::NoCompanion) {
                        self.recovering = false;
                    }
                }
                if !newer {
                    return;
                }
                if let Some(snapshot) = state {
                    Some(HomunculusPresence::Present(Box::new(snapshot.clone())))
                } else if *error == HomunculusError::NoCompanion
                    || *success && pending.is_some_and(|p| p.may_remove)
                {
                    Some(HomunculusPresence::Absent)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(update) = update {
            self.presence = update;
            self.last_state = Some(sequence);
            self.snapshot_received = now;
        }
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct Incoming<'w, 's> {
    ingress: MessageReader<'w, 's, HomunculusIngress>,
    ready: MessageReader<'w, 's, HomunculusReady>,
    failures: MessageReader<'w, 's, HomunculusSendFailed>,
    left: MessageReader<'w, 's, LeaveZone>,
    disconnects: MessageReader<'w, 's, ZoneDisconnected>,
}

fn receive(
    mut state: ResMut<HomunculusState>,
    generation: Res<ZoneSessionGeneration>,
    time: Res<Time<Real>>,
    mut incoming: Incoming,
    mut out: MessageWriter<HomunculusFeedback>,
) {
    let disconnected = incoming.disconnects.read().count() > 0;
    let left = incoming.left.read().count() > 0;
    if state.generation != *generation || disconnected || left {
        *state = HomunculusState {
            generation: *generation,
            ..default()
        };
    }
    if disconnected || left {
        incoming.ingress.clear();
        incoming.ready.clear();
        incoming.failures.clear();
        return;
    }
    for event in incoming.ingress.read() {
        state.apply_ingress(event, time.elapsed(), &mut out);
    }
    for event in incoming.ready.read() {
        if event.generation == state.generation {
            state.ready = true;
        }
    }
    for fail in incoming.failures.read() {
        if fail.generation == state.generation && state.take_request(fail.request_id).is_some() {
            state.feedback(
                fail.request_id,
                HomunculusFeedbackKind::SendFailed(fail.reason.clone()),
                &mut out,
            );
        }
    }
}

fn resolve_actor(
    mut state: ResMut<HomunculusState>,
    registry: Res<EntityRegistry>,
    actors: Query<(), With<Homunculus>>,
) {
    let gid = state
        .snapshot()
        .filter(|s| state.ready && s.lifecycle == HomunculusLifecycle::Active)
        .and_then(|s| s.world_gid);
    let actor = gid.and_then(|gid| {
        registry
            .get_entity(gid)
            .filter(|entity| actors.contains(*entity))
            .map(|e| (gid, e))
    });
    if actor == state.actor {
        return;
    }
    if state.window_visible
        && let Some((old_gid, _)) = state.actor
        && gid == Some(old_gid)
        && actor.is_none()
    {
        state.wants_inspect = true;
    }
    state.actor = actor;
}

fn dispatch(
    mut state: ResMut<HomunculusState>,
    time: Res<Time<Real>>,
    mut input: MessageReader<HomunculusIntent>,
    mut commands: MessageWriter<HomunculusCommand>,
    mut out: MessageWriter<HomunculusFeedback>,
) {
    let now = time.elapsed();
    if state.mutation.as_ref().is_some_and(|p| p.deadline <= now)
        && let Some(p) = state.mutation.take()
    {
        state.recovering = true;
        state.wants_inspect = true;
        // An Inspect sent before this timeout cannot settle recovery.
        state.inspect = None;
        state.feedback(p.id, HomunculusFeedbackKind::TimedOut, &mut out);
    }
    if state.inspect.as_ref().is_some_and(|p| p.deadline <= now)
        && let Some(p) = state.inspect.take()
    {
        state.feedback(p.id, HomunculusFeedbackKind::TimedOut, &mut out);
    }
    for intent in input.read() {
        let (durable_id, action) = match intent {
            HomunculusIntent::Inspect => {
                state.wants_inspect |= state.inspect.is_none();
                continue;
            }
            HomunculusIntent::WindowVisibility(visible) => {
                state.wants_inspect |= *visible && !state.window_visible && state.inspect.is_none();
                state.window_visible = *visible;
                continue;
            }
            HomunculusIntent::Act { durable_id, action } => (*durable_id, action),
        };
        if !state.can_mutate()
            || !state
                .snapshot()
                .is_some_and(|snapshot| snapshot.durable_id == durable_id)
        {
            continue;
        }
        if matches!(
            action,
            Action::Inspect | Action::Delete { confirmed: false }
        ) {
            continue;
        }
        if matches!(
            action,
            Action::Move { .. }
                | Action::Attack { .. }
                | Action::Follow
                | Action::Standby
                | Action::CastSkill { .. }
                | Action::Feed
                | Action::Rest
        ) && state.controlled_gid().is_none()
        {
            continue;
        }
        let id = send(&mut state, action.clone(), &mut commands);
        state.mutation = Some(Pending {
            id,
            deadline: now + TIMEOUT,
            may_remove: matches!(action, Action::Feed | Action::Delete { .. }),
        });
        state.last_feedback = None;
    }
    if state.ready && state.wants_inspect && state.inspect.is_none() {
        state.wants_inspect = false;
        let id = send(&mut state, Action::Inspect, &mut commands);
        state.inspect = Some(Pending {
            id,
            deadline: now + TIMEOUT,
            may_remove: false,
        });
    }
}

fn send(
    state: &mut HomunculusState,
    action: Action,
    out: &mut MessageWriter<HomunculusCommand>,
) -> u64 {
    state.serial = state.serial.wrapping_add(1).max(1);
    out.write(HomunculusCommand {
        generation: state.generation,
        request_id: state.serial,
        action,
    });
    state.serial
}

#[cfg(test)]
mod tests;
