use std::collections::HashMap;

use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::{Activate, ValueChange, checkbox_self_update};
use bevy_feathers::controls::{ButtonVariant, FeathersButton, FeathersCheckbox};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::guild::GuildState;
use net_contract::commands::{GuildMemberPositionRequested, GuildPositionEditRequested};
use net_contract::dto::{GuildErrorKind, GuildInfo, GuildPositionInfo};
use net_contract::events::{GuildIngress, GuildIngressPayload};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::{
    GuildMutationContext, GuildMutationControl, GuildPositionsList, GuildUi, GuildUiSession,
    PendingGuildMutation,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PositionKey {
    guild_id: u32,
    index: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PositionEdits {
    name: Option<String>,
    can_invite: Option<bool>,
    can_expel: Option<bool>,
    can_storage: Option<bool>,
    tax: Option<String>,
}

impl PositionEdits {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.can_invite.is_none()
            && self.can_expel.is_none()
            && self.can_storage.is_none()
            && self.tax.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PositionSubmission {
    key: PositionKey,
    generation: ZoneSessionGeneration,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct PositionDraftState {
    edits: HashMap<PositionKey, PositionEdits>,
    submission: Option<PositionSubmission>,
    rebuild_requested: bool,
}

#[derive(Component, Clone, Debug, Default)]
pub(crate) struct PositionDraft {
    guild_id: u32,
    index: u32,
    can_invite: bool,
    can_expel: bool,
    can_storage: bool,
    baseline_name: String,
    baseline_can_invite: bool,
    baseline_can_expel: bool,
    baseline_can_storage: bool,
    baseline_tax: String,
}

#[derive(Component, Default, Clone)]
pub(crate) struct PositionNameField;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionTaxField;
type PositionTextFields<'w, 's> = Query<
    'w,
    's,
    (
        &'static EditableText,
        &'static ChildOf,
        Has<PositionNameField>,
        Has<PositionTaxField>,
    ),
    Or<(With<PositionNameField>, With<PositionTaxField>)>,
>;
type ChangedPositionTextFields<'w, 's> = Query<
    'w,
    's,
    (
        Ref<'static, EditableText>,
        &'static ChildOf,
        Has<PositionNameField>,
        Has<PositionTaxField>,
    ),
    Or<(With<PositionNameField>, With<PositionTaxField>)>,
>;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionInviteToggle;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionExpelToggle;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionStorageToggle;
#[derive(Component, Default, Clone)]
struct PositionSave;
type PermissionToggles<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ChildOf,
        Has<bevy::ui::Checked>,
        Has<PositionInviteToggle>,
        Has<PositionExpelToggle>,
    ),
    Or<(
        With<PositionInviteToggle>,
        With<PositionExpelToggle>,
        With<PositionStorageToggle>,
    )>,
>;

#[derive(Component, Clone, Debug, Default)]
struct AssignmentAction {
    target_char_id: u32,
    position_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PositionRow {
    guild_id: u32,
    pub index: u32,
    pub name: String,
    pub can_invite: bool,
    pub can_expel: bool,
    pub can_storage: bool,
    pub tax: u32,
    tax_input: String,
    baseline_name: String,
    baseline_can_invite: bool,
    baseline_can_expel: bool,
    baseline_can_storage: bool,
    baseline_tax: String,
    pub protected: bool,
    pub editable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PositionChoice {
    pub index: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemberAssignmentRow {
    pub char_id: u32,
    pub name: String,
    pub current_position: u32,
    pub positions: Vec<PositionChoice>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PositionRenderSignature {
    guild_id: Option<u32>,
    requester_char_id: u32,
    rows: Vec<PositionRow>,
    assignments: Vec<MemberAssignmentRow>,
}

fn master_position(info: &GuildInfo) -> Option<u32> {
    info.members
        .iter()
        .find(|member| member.char_id == info.master_char_id)
        .map(|member| member.position_index)
}

pub(crate) fn project_positions(info: &GuildInfo, is_master: bool) -> Vec<PositionRow> {
    let master_position = master_position(info);
    let mut rows: Vec<_> = info
        .positions
        .iter()
        .map(|position| {
            let protected = master_position == Some(position.index);
            PositionRow {
                guild_id: info.guild_id,
                index: position.index,
                name: position.name.clone(),
                can_invite: position.can_invite,
                can_expel: position.can_expel,
                can_storage: position.can_storage,
                tax: position.tax,
                tax_input: position.tax.to_string(),
                baseline_name: position.name.clone(),
                baseline_can_invite: position.can_invite,
                baseline_can_expel: position.can_expel,
                baseline_can_storage: position.can_storage,
                baseline_tax: position.tax.to_string(),
                protected,
                editable: is_master && !protected,
            }
        })
        .collect();
    rows.sort_by_key(|row| row.index);
    rows
}

fn project_positions_with_drafts(
    info: &GuildInfo,
    is_master: bool,
    drafts: &PositionDraftState,
) -> Vec<PositionRow> {
    let mut rows = project_positions(info, is_master);
    for row in &mut rows {
        let key = PositionKey {
            guild_id: info.guild_id,
            index: row.index,
        };
        let Some(edits) = drafts.edits.get(&key) else {
            continue;
        };
        if let Some(name) = &edits.name {
            row.name.clone_from(name);
        }
        if let Some(can_invite) = edits.can_invite {
            row.can_invite = can_invite;
        }
        if let Some(can_expel) = edits.can_expel {
            row.can_expel = can_expel;
        }
        if let Some(can_storage) = edits.can_storage {
            row.can_storage = can_storage;
        }
        if let Some(tax) = &edits.tax {
            row.tax_input.clone_from(tax);
        }
    }
    rows
}

pub(crate) fn project_assignments(info: &GuildInfo, is_master: bool) -> Vec<MemberAssignmentRow> {
    if !is_master {
        return Vec::new();
    }
    let protected = master_position(info);
    let mut positions: Vec<_> = info
        .positions
        .iter()
        .filter(|position| Some(position.index) != protected && !position.name.trim().is_empty())
        .map(|position| PositionChoice {
            index: position.index,
            name: position.name.clone(),
        })
        .collect();
    positions.sort_by_key(|position| position.index);

    info.members
        .iter()
        .filter(|member| member.char_id != info.master_char_id)
        .map(|member| MemberAssignmentRow {
            char_id: member.char_id,
            name: member.name.clone(),
            current_position: member.position_index,
            positions: positions.clone(),
        })
        .collect()
}

pub(crate) fn request_position_edit(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    mut command: GuildPositionEditRequested,
) -> Option<GuildPositionEditRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    if requester_char_id != info.master_char_id {
        ui.feedback = Some(
            super::feedback::guild_action_error_text("position_edit", GuildErrorKind::NoPermission)
                .to_string(),
        );
        ui.feedback_is_error = true;
        return None;
    }
    if !can_edit_position(info, requester_char_id, command.index) {
        ui.feedback = Some("This position cannot be edited.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    command.name = command.name.trim().to_string();
    if command.name.is_empty() {
        ui.feedback = Some("Enter a position name.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "position_edit",
        generation,
    });
    ui.feedback = Some("Saving position…".to_string());
    ui.feedback_is_error = false;
    Some(command)
}

pub(crate) fn request_member_assignment(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    target_char_id: u32,
    index: u32,
) -> Option<GuildMemberPositionRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    let valid_target = target_char_id != info.master_char_id
        && info
            .members
            .iter()
            .any(|member| member.char_id == target_char_id);
    let valid_position = Some(index) != master_position(info)
        && info
            .positions
            .iter()
            .any(|position| position.index == index);
    if requester_char_id != info.master_char_id || !valid_target || !valid_position {
        ui.feedback = Some("This member cannot be assigned to that position.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "member_position",
        generation,
    });
    ui.feedback = Some("Assigning position…".to_string());
    ui.feedback_is_error = false;
    Some(GuildMemberPositionRequested {
        target_char_id,
        index,
    })
}

fn can_edit_position(info: &GuildInfo, requester_char_id: u32, index: u32) -> bool {
    requester_char_id == info.master_char_id
        && Some(index) != master_position(info)
        && info
            .positions
            .iter()
            .any(|position| position.index == index)
}

fn set_edits(
    drafts: &mut ResMut<PositionDraftState>,
    key: PositionKey,
    update: impl FnOnce(&mut PositionEdits),
) {
    let mut edits = drafts.edits.get(&key).cloned().unwrap_or_default();
    update(&mut edits);
    if edits.is_empty() {
        if drafts.edits.contains_key(&key) {
            drafts.edits.remove(&key);
        }
    } else if drafts.edits.get(&key) != Some(&edits) {
        drafts.edits.insert(key, edits);
    }
}

fn position_for_draft<'a>(
    info: &'a GuildInfo,
    draft: &PositionDraft,
) -> Option<&'a GuildPositionInfo> {
    (info.guild_id == draft.guild_id)
        .then(|| {
            info.positions
                .iter()
                .find(|position| position.index == draft.index)
        })
        .flatten()
}

pub(crate) fn sync_position_drafts(
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    ui_session: Res<GuildUiSession>,
    changed_rows: Query<Ref<PositionDraft>>,
    changed_fields: ChangedPositionTextFields,
    rows: Query<&PositionDraft>,
    mut drafts: ResMut<PositionDraftState>,
) {
    if ui_session.reset || ui_session.blocked {
        return;
    }
    let Some(info) = guild.info() else {
        if !drafts.edits.is_empty() || drafts.submission.is_some() || drafts.rebuild_requested {
            *drafts = PositionDraftState::default();
        }
        return;
    };
    if !guild.is_master(session.char_id) {
        if !drafts.edits.is_empty() || drafts.submission.is_some() || drafts.rebuild_requested {
            *drafts = PositionDraftState::default();
        }
        return;
    }

    let invalid = |key: &PositionKey| {
        key.guild_id != info.guild_id || !can_edit_position(info, session.char_id, key.index)
    };
    if drafts.edits.keys().any(invalid) {
        drafts.edits.retain(|key, _| !invalid(key));
    }
    if drafts
        .submission
        .is_some_and(|submission| invalid(&submission.key))
    {
        drafts.submission = None;
    }

    for draft in &changed_rows {
        if !draft.is_changed() || position_for_draft(info, &draft).is_none() {
            continue;
        }
        if !can_edit_position(info, session.char_id, draft.index) {
            continue;
        }
        let key = PositionKey {
            guild_id: draft.guild_id,
            index: draft.index,
        };
        set_edits(&mut drafts, key, |edits| {
            edits.can_invite =
                (draft.can_invite != draft.baseline_can_invite).then_some(draft.can_invite);
            edits.can_expel =
                (draft.can_expel != draft.baseline_can_expel).then_some(draft.can_expel);
            edits.can_storage =
                (draft.can_storage != draft.baseline_can_storage).then_some(draft.can_storage);
        });
    }

    for (field, parent, is_name, is_tax) in &changed_fields {
        if !field.is_changed() {
            continue;
        }
        let Ok(draft) = rows.get(parent.parent()) else {
            continue;
        };
        if position_for_draft(info, draft).is_none()
            || !can_edit_position(info, session.char_id, draft.index)
        {
            continue;
        }
        let value = field.value().to_string();
        let key = PositionKey {
            guild_id: draft.guild_id,
            index: draft.index,
        };
        set_edits(&mut drafts, key, |edits| {
            if is_name {
                edits.name = (value != draft.baseline_name).then_some(value.clone());
            }
            if is_tax {
                edits.tax = (value != draft.baseline_tax).then_some(value);
            }
        });
    }
}

pub(crate) fn resolve_position_submission(
    mut ingress: MessageReader<GuildIngress>,
    generation: Res<ZoneSessionGeneration>,
    session: Res<GuildUiSession>,
    mut drafts: ResMut<PositionDraftState>,
) {
    if session.blocked {
        ingress.clear();
        return;
    }
    for event in ingress.read() {
        let GuildIngressPayload::ActionResult(result) = &event.payload else {
            continue;
        };
        let Some(submission) = drafts.submission else {
            continue;
        };
        if result.action != "position_edit"
            || event.generation != *generation
            || event.generation != submission.generation
        {
            continue;
        }
        if result.success {
            drafts.edits.remove(&submission.key);
            drafts.rebuild_requested = true;
        }
        drafts.submission = None;
    }
}

pub(crate) fn reset_position_drafts(
    session: Res<GuildUiSession>,
    mut drafts: ResMut<PositionDraftState>,
) {
    if session.reset
        && (!drafts.edits.is_empty() || drafts.submission.is_some() || drafts.rebuild_requested)
    {
        *drafts = PositionDraftState::default();
    }
}

pub(crate) fn clear_position_drafts(mut drafts: ResMut<PositionDraftState>) {
    if !drafts.edits.is_empty() || drafts.submission.is_some() || drafts.rebuild_requested {
        *drafts = PositionDraftState::default();
    }
}

pub(crate) fn refresh_positions(
    mut commands: Commands,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    ui_session: Res<GuildUiSession>,
    mut drafts: ResMut<PositionDraftState>,
    container: Query<(Entity, Option<&Children>), With<GuildPositionsList>>,
    mut rendered: Local<Option<PositionRenderSignature>>,
) {
    let Ok((container, children)) = container.single() else {
        return;
    };
    let is_master = guild.is_master(session.char_id);
    let rows = guild
        .info()
        .map(|info| project_positions(info, is_master))
        .unwrap_or_default();
    let assignments = guild
        .info()
        .map(|info| project_assignments(info, is_master))
        .unwrap_or_default();
    let signature = PositionRenderSignature {
        guild_id: guild.info().map(|info| info.guild_id),
        requester_char_id: session.char_id,
        rows,
        assignments,
    };
    let relevant_changed = rendered.as_ref() != Some(&signature);
    let empty = children.is_none_or(|children| children.is_empty());
    if !empty && !relevant_changed && !ui_session.reset && !drafts.rebuild_requested {
        return;
    }
    *rendered = Some(signature);
    if drafts.rebuild_requested {
        drafts.rebuild_requested = false;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let Some(info) = guild.info() else {
        return;
    };
    let rows = project_positions_with_drafts(info, is_master, &drafts);
    let assignments = project_assignments(info, is_master);
    commands
        .spawn_scene(position_management(rows, assignments, is_master))
        .insert(ChildOf(container));
}

/// Mirrors each row's draft flags onto its invite, expel and storage checkboxes.
pub(crate) fn sync_permission_toggles(
    rows: Query<&PositionDraft>,
    toggles: PermissionToggles,
    mut commands: Commands,
) {
    for (toggle, parent, checked, is_invite, is_expel) in &toggles {
        let Ok(draft) = rows.get(parent.parent()) else {
            continue;
        };
        let wanted = if is_invite {
            draft.can_invite
        } else if is_expel {
            draft.can_expel
        } else {
            draft.can_storage
        };
        if wanted == checked {
            continue;
        }
        let mut toggle = commands.entity(toggle);
        if wanted {
            toggle.insert(bevy::ui::Checked);
        } else {
            toggle.remove::<bevy::ui::Checked>();
        }
    }
}

fn flag_mark(value: bool) -> &'static str {
    if value { "✓" } else { "–" }
}

fn parent_draft(
    control: Entity,
    parents: &Query<&ChildOf>,
    drafts: &Query<(), With<PositionDraft>>,
) -> Option<Entity> {
    let parent = parents.get(control).ok()?.parent();
    drafts.contains(parent).then_some(parent)
}

fn on_toggle_permission(
    event: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    drafts: Query<(), With<PositionDraft>>,
    kinds: Query<(Has<PositionInviteToggle>, Has<PositionExpelToggle>)>,
    mut mutable_drafts: Query<&mut PositionDraft>,
) {
    let Some(row) = parent_draft(event.source, &parents, &drafts) else {
        return;
    };
    let Ok((is_invite, is_expel)) = kinds.get(event.source) else {
        return;
    };
    let Ok(mut draft) = mutable_drafts.get_mut(row) else {
        return;
    };
    if is_invite {
        draft.can_invite = event.value;
    } else if is_expel {
        draft.can_expel = event.value;
    } else {
        draft.can_storage = event.value;
    }
}

fn parse_tax(raw: &str) -> Option<u32> {
    if raw.is_empty() || !raw.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    raw.parse::<u32>().ok().filter(|tax| *tax <= 100)
}

fn command_from_draft(
    position: &GuildPositionInfo,
    draft: &PositionDraft,
    name: String,
    raw_tax: &str,
) -> Result<GuildPositionEditRequested, &'static str> {
    let Some(tax) = parse_tax(raw_tax) else {
        return Err("EXP tax must be a whole percentage from 0 to 100.");
    };
    Ok(GuildPositionEditRequested {
        index: draft.index,
        name,
        can_invite: draft.can_invite,
        can_expel: draft.can_expel,
        tax: (tax != position.tax).then_some(tax),
        can_storage: (draft.can_storage != position.can_storage).then_some(draft.can_storage),
    })
}

fn on_save_position(
    event: On<Activate>,
    parents: Query<&ChildOf>,
    drafts: Query<&PositionDraft>,
    fields: PositionTextFields,
    mut draft_state: ResMut<PositionDraftState>,
    mut context: GuildMutationContext,
    mut writer: MessageWriter<GuildPositionEditRequested>,
) {
    let Ok(parent) = parents.get(event.entity) else {
        return;
    };
    let row = parent.parent();
    let Ok(draft) = drafts.get(row) else {
        return;
    };
    let mut name = None;
    let mut raw_tax = None;
    for (field, parent, is_name, is_tax) in &fields {
        if parent.parent() != row {
            continue;
        }
        if is_name {
            name = Some(field.value().to_string());
        }
        if is_tax {
            raw_tax = Some(field.value().to_string());
        }
    }
    let (Some(name), Some(raw_tax)) = (name, raw_tax) else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    let Some(position) = position_for_draft(info, draft) else {
        return;
    };
    let command = match command_from_draft(position, draft, name.clone(), &raw_tax) {
        Ok(command) => command,
        Err(feedback) => {
            context.ui.feedback = Some(feedback.to_string());
            context.ui.feedback_is_error = true;
            return;
        }
    };
    let key = PositionKey {
        guild_id: draft.guild_id,
        index: draft.index,
    };
    set_edits(&mut draft_state, key, |edits| {
        edits.name = (name != position.name).then_some(name);
        edits.can_invite = (draft.can_invite != position.can_invite).then_some(draft.can_invite);
        edits.can_expel = (draft.can_expel != position.can_expel).then_some(draft.can_expel);
        edits.can_storage =
            (draft.can_storage != position.can_storage).then_some(draft.can_storage);
        edits.tax = (raw_tax != position.tax.to_string()).then_some(raw_tax);
    });
    if let Some(command) = request_position_edit(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        command,
    ) {
        draft_state.submission = Some(PositionSubmission {
            key,
            generation: *context.generation,
        });
        writer.write(command);
    }
}

fn on_assign_member(
    event: On<Activate>,
    actions: Query<&AssignmentAction>,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    generation: Res<ZoneSessionGeneration>,
    mut ui: ResMut<GuildUi>,
    mut writer: MessageWriter<GuildMemberPositionRequested>,
) {
    let Ok(action) = actions.get(event.entity) else {
        return;
    };
    let Some(info) = guild.info() else {
        return;
    };
    if let Some(command) = request_member_assignment(
        &mut ui,
        *generation,
        info,
        session.char_id,
        action.target_char_id,
        action.position_index,
    ) {
        writer.write(command);
    }
}

const COL_INDEX: f32 = 30.0;
const COL_FLAG: f32 = 58.0;
const COL_TAX: f32 = 54.0;
const COL_ACTION: f32 = 66.0;
const ROW_HEIGHT: f32 = 28.0;

/// Layout state of one of a row's two variants (read-only or editable).
fn variant(active: bool) -> (Display, Visibility) {
    if active {
        (Display::Flex, Visibility::Inherited)
    } else {
        (Display::None, Visibility::Hidden)
    }
}

fn cell(width: f32, text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Node { width: px(width), flex_shrink: 0.0 }
        chrome_text(text, size, color)
    }
}

pub(crate) fn position_management(
    rows: Vec<PositionRow>,
    assignments: Vec<MemberAssignmentRow>,
    is_master: bool,
) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(position_row).collect();
    let assignments: Vec<_> = assignments.into_iter().map(assignment_row).collect();
    let (assignment_display, assignment_visibility) = variant(is_master);
    let hint = if is_master {
        "Name a slot, set its permissions and save the row. Members can only be assigned to named slots."
    } else {
        "Permissions granted by each guild position."
    };
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [
            chrome_text("Positions".to_string(), 13.0, theme::TEXT),
            chrome_text(hint.to_string(), 10.5, theme::TEXT_DIM),
            table_heading(),
            (Node { flex_direction: FlexDirection::Column, row_gap: px(4) } ignore_picking() Children [ {rows} ]),
            (
                template_value(assignment_visibility)
                Node { flex_direction: FlexDirection::Column, row_gap: px(6), padding: {UiRect::top(px(10))}, display: {assignment_display} }
                ignore_picking()
                Children [
                    chrome_text("Member assignments".to_string(), 13.0, theme::TEXT),
                    chrome_text("Click a position to assign it. The highlighted one is the member's current position.".to_string(), 10.5, theme::TEXT_DIM),
                    (Node { flex_direction: FlexDirection::Column, row_gap: px(4) } ignore_picking() Children [ {assignments} ]),
                ]
            ),
        ]
    }
}

fn table_heading() -> impl Scene {
    let label = |text: &str, width: f32| cell(width, text.to_string(), 9.5, theme::TEXT_FAINT);
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(6), padding: {UiRect::horizontal(px(8))} }
        ignore_picking()
        Children [
            label("#", COL_INDEX),
            (Node { flex_grow: 1.0, min_width: px(0) } chrome_text("Name".to_string(), 9.5, theme::TEXT_FAINT)),
            label("Invite", COL_FLAG),
            label("Expel", COL_FLAG),
            label("Storage", COL_FLAG),
            label("Tax %", COL_TAX),
            label("", COL_ACTION),
        ]
    }
}

fn text_field(field: EditableText, width: Val, size: f32, display: Display) -> impl Scene {
    bsn! {
        Pickable
        template_value(field)
        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(size)} }
        TextColor(theme::TEXT)
        BackgroundColor(theme::GLASS_2)
        BorderColor::all(theme::STROKE)
        Node {
            width: width,
            flex_shrink: 0.0,
            height: px(ROW_HEIGHT),
            padding: {UiRect::axes(px(7), px(4))},
            border: px(1),
            border_radius: BorderRadius::all(px(5)),
            display: {display},
        }
    }
}

fn permission_toggle(display: Display) -> impl Scene {
    bsn! {
        GuildMutationControl
        @FeathersCheckbox {}
        on(checkbox_self_update)
        on(on_toggle_permission)
        Node { width: px(COL_FLAG), flex_shrink: 0.0, display: {display} }
    }
}

fn position_row(row: PositionRow) -> impl Scene {
    let (edit_display, edit_visibility) = variant(row.editable);
    let (read_display, read_visibility) = variant(!row.editable);
    let name = if row.name.is_empty() {
        "(unnamed)".to_string()
    } else {
        row.name.clone()
    };
    let name_color = if row.name.is_empty() {
        theme::TEXT_FAINT
    } else {
        theme::TEXT
    };
    let badge = if row.protected { "Master" } else { "" };
    let name_field = EditableText {
        max_characters: Some(24),
        ..EditableText::new(row.name.clone())
    };
    let tax_field = EditableText {
        max_characters: Some(3),
        ..EditableText::new(&row.tax_input)
    };
    bsn! {
        template_value(PositionDraft {
            guild_id: row.guild_id,
            index: row.index,
            can_invite: row.can_invite,
            can_expel: row.can_expel,
            can_storage: row.can_storage,
            baseline_name: row.baseline_name,
            baseline_can_invite: row.baseline_can_invite,
            baseline_can_expel: row.baseline_can_expel,
            baseline_can_storage: row.baseline_can_storage,
            baseline_tax: row.baseline_tax,
        })
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(6),
            padding: {UiRect::axes(px(8), px(5))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        Children [
            cell(COL_INDEX, row.index.to_string(), 11.0, theme::TEXT_DIM),
            (
                template_value(read_visibility)
                Node { flex_grow: 1.0, min_width: px(0), display: {read_display} }
                chrome_text(name, 12.0, name_color)
            ),
            (
                PositionNameField GuildMutationControl
                template_value(edit_visibility)
                text_field(name_field, auto(), 12.0, edit_display)
                Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: px(0) }
            ),
            (
                template_value(read_visibility)
                Node { display: {read_display} }
                cell(COL_FLAG, flag_mark(row.can_invite).to_string(), 12.0, theme::TEXT_DIM)
            ),
            (
                PositionInviteToggle
                template_value(edit_visibility)
                permission_toggle(edit_display)
            ),
            (
                template_value(read_visibility)
                Node { display: {read_display} }
                cell(COL_FLAG, flag_mark(row.can_expel).to_string(), 12.0, theme::TEXT_DIM)
            ),
            (
                PositionExpelToggle
                template_value(edit_visibility)
                permission_toggle(edit_display)
            ),
            (
                template_value(read_visibility)
                Node { display: {read_display} }
                cell(COL_FLAG, flag_mark(row.can_storage).to_string(), 12.0, theme::TEXT_DIM)
            ),
            (
                PositionStorageToggle
                template_value(edit_visibility)
                permission_toggle(edit_display)
            ),
            (
                template_value(read_visibility)
                Node { display: {read_display} }
                cell(COL_TAX, format!("{}%", row.tax), 11.0, theme::TEXT_DIM)
            ),
            (
                PositionTaxField GuildMutationControl
                template_value(edit_visibility)
                text_field(tax_field, px(COL_TAX), 11.0, edit_display)
            ),
            (
                template_value(read_visibility)
                Node { display: {read_display} }
                cell(COL_ACTION, badge.to_string(), 10.0, theme::GOLD)
            ),
            (
                PositionSave GuildMutationControl
                template_value(edit_visibility)
                @FeathersButton {
                    @caption: bsn! { (Text("Save") ThemedText) },
                    @variant: ButtonVariant::Primary,
                }
                Node { width: px(COL_ACTION), flex_shrink: 0.0, height: px(ROW_HEIGHT), display: {edit_display} }
                on(on_save_position)
            ),
        ]
    }
}

fn assignment_row(row: MemberAssignmentRow) -> impl Scene {
    let buttons: Vec<_> = row
        .positions
        .into_iter()
        .map(|position| assignment_button(row.char_id, row.current_position, position))
        .collect();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(8), px(5))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        Children [
            (Node { width: px(150), flex_shrink: 0.0 } chrome_text(row.name, 12.0, theme::TEXT)),
            (Node { flex_grow: 1.0, min_width: px(0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5) } ignore_picking() Children [ {buttons} ]),
        ]
    }
}

fn assignment_button(
    target_char_id: u32,
    current_position: u32,
    position: PositionChoice,
) -> impl Scene {
    let variant = if current_position == position.index {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Normal
    };
    let caption = position.name;
    bsn! {
        template_value(AssignmentAction { target_char_id, position_index: position.index })
        GuildMutationControl
        @FeathersButton { @caption: bsn! { (Text(caption) ThemedText) }, @variant: variant }
        Node { height: px(ROW_HEIGHT), padding: {UiRect::horizontal(px(8))} }
        on(on_assign_member)
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;
    use game_engine::domain::guild::{GuildPlugin, GuildSystems};
    use net_contract::commands::GuildPositionEditRequested;
    use net_contract::dto::{
        GuildActionResult, GuildErrorKind, GuildInfo, GuildMemberInfo, GuildPositionInfo,
    };
    use net_contract::events::{GuildIngress, GuildIngressPayload, ZoneDisconnected};
    use net_contract::state::{ZoneSession, ZoneSessionGeneration};

    use super::*;

    fn guild() -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 0,
            notice_subject: String::new(),
            notice_body: String::new(),
            positions: vec![
                GuildPositionInfo {
                    index: 7,
                    name: "Master".into(),
                    can_invite: true,
                    can_expel: true,
                    can_storage: true,
                    tax: 50,
                },
                GuildPositionInfo {
                    index: 2,
                    name: "Member".into(),
                    can_invite: false,
                    can_expel: false,
                    can_storage: false,
                    tax: 0,
                },
            ],
            members: vec![
                GuildMemberInfo {
                    char_id: 42,
                    name: "Odin".into(),
                    position_index: 7,
                    job_id: 1,
                    base_level: 99,
                    online: true,
                    map: "prontera".into(),
                    hp: 1,
                    max_hp: 1,
                    sp: 1,
                    max_sp: 1,
                    ap: 0,
                    max_ap: 0,
                },
                GuildMemberInfo {
                    char_id: 43,
                    name: "Thor".into(),
                    position_index: 2,
                    job_id: 2,
                    base_level: 80,
                    online: true,
                    map: "geffen".into(),
                    hp: 1,
                    max_hp: 1,
                    sp: 1,
                    max_sp: 1,
                    ap: 0,
                    max_ap: 0,
                },
            ],
            level: 1,
            exp: 0,
            next_exp: 0,
            skill_points: 0,
            skills: vec![],
            relations: vec![],
        }
    }

    fn draft_flow_app() -> App {
        let generation = ZoneSessionGeneration(9);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .add_message::<GuildPositionEditRequested>()
            .insert_resource(generation)
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .insert_resource(GuildUiSession {
                generation,
                char_id: 42,
                ..default()
            })
            .init_resource::<GuildUi>()
            .init_resource::<PositionDraftState>()
            .init_resource::<super::super::emblem::GuildEmblemPreview>()
            .add_plugins(GuildPlugin);
        app.world_mut().spawn(GuildPositionsList);
        app.add_systems(
            Update,
            (super::super::reset_guild_ui_session, reset_position_drafts)
                .chain()
                .in_set(GuildSystems::SessionReset),
        );
        app.add_systems(
            Update,
            (
                sync_position_drafts,
                super::super::feedback::apply_guild_results,
                resolve_position_submission,
                refresh_positions,
                sync_permission_toggles,
            )
                .chain()
                .in_set(GuildSystems::UiSync),
        );
        app
    }

    fn send_snapshot(app: &mut App, info: GuildInfo) {
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(info),
        });
        app.update();
    }

    fn position_row_entity(app: &mut App, index: u32) -> Entity {
        app.world_mut()
            .query::<(Entity, &PositionDraft)>()
            .iter(app.world())
            .find_map(|(entity, draft)| (draft.index == index).then_some(entity))
            .unwrap()
    }

    fn row_field<M: Component>(app: &mut App, row: Entity) -> Entity {
        app.world_mut()
            .query_filtered::<(Entity, &ChildOf), With<M>>()
            .iter(app.world())
            .find_map(|(entity, parent)| (parent.parent() == row).then_some(entity))
            .unwrap()
    }

    fn set_field(app: &mut App, entity: Entity, value: &str) {
        app.world_mut()
            .entity_mut(entity)
            .get_mut::<EditableText>()
            .unwrap()
            .editor_mut()
            .set_text(value);
    }

    fn field_value<M: Component>(app: &mut App, index: u32) -> String {
        let row = position_row_entity(app, index);
        let field = row_field::<M>(app, row);
        app.world()
            .entity(field)
            .get::<EditableText>()
            .unwrap()
            .value()
            .to_string()
    }

    #[test]
    fn invalid_tax_shows_local_feedback_and_writes_no_message() {
        let mut app = draft_flow_app();
        send_snapshot(&mut app, guild());
        let row = position_row_entity(&mut app, 2);
        let tax = row_field::<PositionTaxField>(&mut app, row);
        let save = row_field::<PositionSave>(&mut app, row);
        set_field(&mut app, tax, "101");

        app.world_mut().trigger(Activate { entity: save });

        assert_eq!(
            app.world().resource::<GuildUi>().feedback.as_deref(),
            Some("EXP tax must be a whole percentage from 0 to 100.")
        );
        assert!(
            app.world()
                .resource::<Messages<GuildPositionEditRequested>>()
                .is_empty()
        );
        assert!(app.world().resource::<GuildUi>().pending.is_none());
    }

    #[test]
    fn consecutive_text_edits_and_idle_updates_keep_field_identity_and_focus() {
        let mut app = draft_flow_app();
        app.init_resource::<bevy::input_focus::InputFocus>();
        send_snapshot(&mut app, guild());
        let row = position_row_entity(&mut app, 2);
        let name = row_field::<PositionNameField>(&mut app, row);
        app.insert_resource(bevy::input_focus::InputFocus::from_entity(name));

        set_field(&mut app, name, "Off");
        app.update();
        assert_eq!(row_field::<PositionNameField>(&mut app, row), name);
        assert_eq!(
            app.world()
                .resource::<bevy::input_focus::InputFocus>()
                .get(),
            Some(name)
        );

        set_field(&mut app, name, "Officer");
        app.update();
        app.update();
        app.update();

        assert_eq!(row_field::<PositionNameField>(&mut app, row), name);
        assert_eq!(field_value::<PositionNameField>(&mut app, 2), "Officer");
        assert_eq!(
            app.world()
                .resource::<bevy::input_focus::InputFocus>()
                .get(),
            Some(name)
        );
    }

    #[test]
    fn unrelated_guild_snapshots_keep_both_editors_and_focus() {
        let mut app = draft_flow_app();
        app.init_resource::<bevy::input_focus::InputFocus>();
        send_snapshot(&mut app, guild());
        let row = position_row_entity(&mut app, 2);
        let name = row_field::<PositionNameField>(&mut app, row);
        let tax = row_field::<PositionTaxField>(&mut app, row);
        set_field(&mut app, name, "Officer");
        set_field(&mut app, tax, "17");
        app.insert_resource(bevy::input_focus::InputFocus::from_entity(name));
        app.update();

        let mut level_update = guild();
        level_update.level = 2;
        send_snapshot(&mut app, level_update);

        assert_eq!(row_field::<PositionNameField>(&mut app, row), name);
        assert_eq!(row_field::<PositionTaxField>(&mut app, row), tax);
        assert_eq!(field_value::<PositionNameField>(&mut app, 2), "Officer");
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "17");
        assert_eq!(
            app.world()
                .resource::<bevy::input_focus::InputFocus>()
                .get(),
            Some(name)
        );

        app.insert_resource(bevy::input_focus::InputFocus::from_entity(tax));
        let mut health_update = guild();
        health_update.members[1].hp = 99;
        send_snapshot(&mut app, health_update);

        assert_eq!(row_field::<PositionNameField>(&mut app, row), name);
        assert_eq!(row_field::<PositionTaxField>(&mut app, row), tax);
        assert_eq!(field_value::<PositionNameField>(&mut app, 2), "Officer");
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "17");
        assert_eq!(
            app.world()
                .resource::<bevy::input_focus::InputFocus>()
                .get(),
            Some(tax)
        );
    }

    #[test]
    fn dirty_row_survives_an_unrelated_snapshot_rebuild() {
        let mut app = draft_flow_app();
        send_snapshot(&mut app, guild());
        let row = position_row_entity(&mut app, 2);
        let name = row_field::<PositionNameField>(&mut app, row);
        let tax = row_field::<PositionTaxField>(&mut app, row);
        set_field(&mut app, name, "Officer");
        set_field(&mut app, tax, "17");
        {
            let mut entity = app.world_mut().entity_mut(row);
            let mut draft = entity.get_mut::<PositionDraft>().unwrap();
            draft.can_invite = true;
            draft.can_expel = true;
            draft.can_storage = true;
        }
        app.update();

        let mut unrelated = guild();
        unrelated.level = 2;
        send_snapshot(&mut app, unrelated);

        assert_eq!(field_value::<PositionNameField>(&mut app, 2), "Officer");
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "17");
        let row = position_row_entity(&mut app, 2);
        let draft = app.world().entity(row).get::<PositionDraft>().unwrap();
        assert!(draft.can_invite);
        assert!(draft.can_expel);
        assert!(draft.can_storage);
        let storage = row_field::<PositionStorageToggle>(&mut app, row);
        assert!(app.world().entity(storage).contains::<bevy::ui::Checked>());
    }

    fn prepare_position_submission(app: &mut App) {
        send_snapshot(app, guild());
        let row = position_row_entity(app, 2);
        let name = row_field::<PositionNameField>(app, row);
        let tax = row_field::<PositionTaxField>(app, row);
        set_field(app, name, "Officer");
        set_field(app, tax, "99");
        app.world_mut()
            .entity_mut(row)
            .get_mut::<PositionDraft>()
            .unwrap()
            .can_storage = true;
        app.update();
        let row = position_row_entity(app, 2);
        let save = row_field::<PositionSave>(app, row);
        app.world_mut().trigger(Activate { entity: save });
        assert_eq!(
            app.world()
                .resource::<GuildUi>()
                .pending
                .as_ref()
                .map(|pending| pending.action),
            Some("position_edit")
        );
    }

    fn send_position_result(app: &mut App, success: bool) {
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "position_edit".into(),
                success,
                error: if success {
                    GuildErrorKind::None
                } else {
                    GuildErrorKind::NoPermission
                },
            }),
        });
        app.update();
    }

    fn clamped_snapshot() -> GuildInfo {
        let mut info = guild();
        let position = &mut info.positions[1];
        position.name = "Officer".into();
        position.can_storage = true;
        position.tax = 50;
        info
    }

    #[test]
    fn snapshot_before_success_keeps_the_draft_then_renders_clamped_values() {
        let mut app = draft_flow_app();
        prepare_position_submission(&mut app);
        assert_eq!(
            app.world()
                .resource::<GuildState>()
                .position(2)
                .unwrap()
                .tax,
            0
        );

        send_snapshot(&mut app, clamped_snapshot());
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "99");

        send_position_result(&mut app, true);
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "50");
        assert!(
            app.world()
                .resource::<PositionDraftState>()
                .edits
                .is_empty()
        );
    }

    #[test]
    fn success_before_snapshot_exits_draft_mode_and_later_renders_clamped_values() {
        let mut app = draft_flow_app();
        prepare_position_submission(&mut app);

        send_position_result(&mut app, true);
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "0");

        send_snapshot(&mut app, clamped_snapshot());
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "50");
    }

    #[test]
    fn rejected_submission_releases_pending_but_preserves_the_dirty_row() {
        let mut app = draft_flow_app();
        prepare_position_submission(&mut app);

        send_position_result(&mut app, false);

        assert!(app.world().resource::<GuildUi>().pending.is_none());
        assert_eq!(field_value::<PositionNameField>(&mut app, 2), "Officer");
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "99");
        assert_eq!(app.world().resource::<PositionDraftState>().edits.len(), 1);
    }

    #[test]
    fn reset_frame_cannot_recapture_cleared_editors_against_fresh_same_guild_state() {
        let mut app = draft_flow_app();
        send_snapshot(&mut app, guild());
        let row = position_row_entity(&mut app, 2);
        let name = row_field::<PositionNameField>(&mut app, row);
        let tax = row_field::<PositionTaxField>(&mut app, row);
        set_field(&mut app, name, "Stale Officer");
        set_field(&mut app, tax, "17");
        app.update();
        assert_eq!(app.world().resource::<PositionDraftState>().edits.len(), 1);

        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(10);
        let mut fresh = guild();
        fresh.positions[1].name = "Server Officer".into();
        fresh.positions[1].tax = 25;
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(10),
            payload: GuildIngressPayload::Info(fresh),
        });
        app.update();

        assert!(app.world().resource::<GuildUiSession>().reset);
        assert!(
            app.world()
                .resource::<PositionDraftState>()
                .edits
                .is_empty()
        );
        assert_eq!(
            field_value::<PositionNameField>(&mut app, 2),
            "Server Officer"
        );
        assert_eq!(field_value::<PositionTaxField>(&mut app, 2), "25");

        app.update();
        assert!(!app.world().resource::<GuildUiSession>().reset);
        assert!(
            app.world()
                .resource::<PositionDraftState>()
                .edits
                .is_empty()
        );
    }

    #[test]
    fn permission_loss_and_session_reset_invalidate_position_drafts() {
        let mut app = draft_flow_app();
        prepare_position_submission(&mut app);
        let mut demoted = guild();
        demoted.master_char_id = 43;

        send_snapshot(&mut app, demoted);

        let state = app.world().resource::<PositionDraftState>();
        assert!(state.edits.is_empty());
        assert!(state.submission.is_none());
        let row = position_row_entity(&mut app, 2);
        let save = row_field::<PositionSave>(&mut app, row);
        assert_eq!(
            *app.world().entity(save).get::<Visibility>().unwrap(),
            Visibility::Hidden
        );

        app.world_mut()
            .resource_mut::<PositionDraftState>()
            .edits
            .insert(
                PositionKey {
                    guild_id: 7,
                    index: 2,
                },
                PositionEdits {
                    name: Some("Stale".into()),
                    ..default()
                },
            );
        *app.world_mut().resource_mut::<ZoneSessionGeneration>() = ZoneSessionGeneration(10);
        app.update();
        assert!(
            app.world()
                .resource::<PositionDraftState>()
                .edits
                .is_empty()
        );
    }

    #[test]
    fn projection_preserves_fixed_slots_and_protects_the_roster_derived_master_position() {
        let rows = project_positions(&guild(), true);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 2);
        assert!(rows[0].editable);
        assert_eq!(rows[1].index, 7);
        assert!(rows[1].protected);
        assert!(!rows[1].editable);
    }

    #[test]
    fn assignments_are_master_only_and_exclude_the_master_member_and_position() {
        assert!(project_assignments(&guild(), false).is_empty());

        let assignments = project_assignments(&guild(), true);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].char_id, 43);
        assert_eq!(assignments[0].positions.len(), 1);
        assert_eq!(assignments[0].positions[0].index, 2);
    }

    #[test]
    fn master_position_edit_writes_exact_mutable_fields_and_reserves_the_shared_pending_slot() {
        let mut ui = crate::widgets::guild_window::GuildUi::default();
        let generation = ZoneSessionGeneration(5);

        let command = request_position_edit(
            &mut ui,
            generation,
            &guild(),
            42,
            GuildPositionEditRequested {
                index: 2,
                name: "  Officer  ".into(),
                can_invite: true,
                can_expel: false,
                tax: None,
                can_storage: None,
            },
        )
        .unwrap();

        assert_eq!(command.index, 2);
        assert_eq!(command.name, "Officer");
        assert!(command.can_invite);
        assert!(!command.can_expel);
        assert_eq!(ui.pending.as_ref().unwrap().action, "position_edit");
    }

    #[test]
    fn position_command_sends_only_changed_optional_storage_and_tax() {
        let info = guild();
        let member = &info.positions[1];
        let unchanged = PositionDraft {
            guild_id: info.guild_id,
            index: member.index,
            can_invite: true,
            can_expel: false,
            can_storage: member.can_storage,
            ..default()
        };

        let command = command_from_draft(member, &unchanged, "Officer".into(), "0").unwrap();

        assert_eq!(command.tax, None);
        assert_eq!(command.can_storage, None);
        assert!(command.can_invite);

        let master = &info.positions[0];
        let reset = PositionDraft {
            guild_id: info.guild_id,
            index: master.index,
            can_invite: master.can_invite,
            can_expel: master.can_expel,
            can_storage: false,
            ..default()
        };
        let command = command_from_draft(master, &reset, master.name.clone(), "0").unwrap();
        assert_eq!(command.tax, Some(0));
        assert_eq!(command.can_storage, Some(false));
    }

    #[test]
    fn invalid_tax_is_rejected_locally_without_constructing_a_command() {
        let info = guild();
        let position = &info.positions[1];
        let draft = PositionDraft {
            guild_id: info.guild_id,
            index: position.index,
            can_invite: position.can_invite,
            can_expel: position.can_expel,
            can_storage: position.can_storage,
            ..default()
        };

        for invalid in ["", "ten", "-1", "101"] {
            assert_eq!(
                command_from_draft(position, &draft, position.name.clone(), invalid).unwrap_err(),
                "EXP tax must be a whole percentage from 0 to 100."
            );
        }
    }

    #[test]
    fn ordinary_members_and_the_protected_position_cannot_submit_edits() {
        let command = GuildPositionEditRequested {
            index: 2,
            name: "Officer".into(),
            can_invite: true,
            can_expel: false,
            tax: None,
            can_storage: None,
        };
        let mut member_ui = crate::widgets::guild_window::GuildUi::default();
        assert!(
            request_position_edit(
                &mut member_ui,
                ZoneSessionGeneration(1),
                &guild(),
                43,
                command,
            )
            .is_none()
        );

        let mut master_ui = crate::widgets::guild_window::GuildUi::default();
        assert!(
            request_position_edit(
                &mut master_ui,
                ZoneSessionGeneration(1),
                &guild(),
                42,
                GuildPositionEditRequested {
                    index: 7,
                    name: "Renamed".into(),
                    can_invite: false,
                    can_expel: false,
                    tax: None,
                    can_storage: None,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn master_assignment_writes_target_and_fixed_slot_without_mutating_the_snapshot() {
        let info = guild();
        let mut ui = crate::widgets::guild_window::GuildUi::default();

        let command =
            request_member_assignment(&mut ui, ZoneSessionGeneration(8), &info, 42, 43, 2).unwrap();

        assert_eq!(command.target_char_id, 43);
        assert_eq!(command.index, 2);
        assert_eq!(ui.pending.as_ref().unwrap().action, "member_position");
        assert_eq!(info.members[1].position_index, 2);
    }

    #[test]
    fn projection_includes_storage_access_and_exp_tax() {
        let rows = project_positions(&guild(), true);

        assert!(!rows[0].can_storage);
        assert_eq!(rows[0].tax, 0);
        assert!(rows[1].can_storage);
        assert_eq!(rows[1].tax, 50);
    }

    #[test]
    fn dirty_fields_overlay_unrelated_authoritative_snapshot_changes() {
        let mut info = guild();
        let mut drafts = PositionDraftState::default();
        drafts.edits.insert(
            PositionKey {
                guild_id: info.guild_id,
                index: 2,
            },
            PositionEdits {
                name: Some("Officer".into()),
                can_storage: Some(true),
                tax: Some("17".into()),
                ..default()
            },
        );
        info.positions[1].can_invite = true;
        info.positions[1].can_expel = true;

        let rows = project_positions_with_drafts(&info, true, &drafts);

        assert_eq!(rows[0].name, "Officer");
        assert!(rows[0].can_invite);
        assert!(rows[0].can_expel);
        assert!(rows[0].can_storage);
        assert_eq!(rows[0].tax_input, "17");
    }

    #[test]
    fn editable_rows_render_focusable_tax_and_storage_controls() {
        let info = guild();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.add_plugins(crate::focus::UiFocusMirrorPlugin);
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(position_management(
                project_positions(&info, true),
                project_assignments(&info, true),
                true,
            ))
            .unwrap();

        let tax_fields: Vec<_> = app
            .world_mut()
            .query_filtered::<(Entity, &EditableText, &Visibility), With<PositionTaxField>>()
            .iter(app.world())
            .map(|(entity, value, visibility)| (entity, value.value().to_string(), *visibility))
            .collect();
        assert!(tax_fields.iter().any(|(_, value, visibility)| {
            value == "0" && *visibility == Visibility::Inherited
        }));
        for (entity, _, _) in tax_fields {
            assert_eq!(
                app.world().get::<Pickable>(entity),
                Some(&Pickable::default())
            );
            assert!(
                app.world()
                    .get::<bevy::input_focus::tab_navigation::TabIndex>(entity)
                    .is_some()
            );
        }

        let storage: Vec<_> = app
            .world_mut()
            .query_filtered::<&Visibility, With<PositionStorageToggle>>()
            .iter(app.world())
            .copied()
            .collect();
        assert!(storage.contains(&Visibility::Inherited));
        assert!(storage.contains(&Visibility::Hidden));
    }

    #[test]
    fn master_scene_exposes_only_non_protected_fixed_slot_controls() {
        let info = guild();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(position_management(
                project_positions(&info, true),
                project_assignments(&info, true),
                true,
            ))
            .unwrap();

        let drafts: std::collections::HashMap<_, _> = app
            .world_mut()
            .query::<(Entity, &PositionDraft)>()
            .iter(app.world())
            .map(|(entity, draft)| (entity, draft.index))
            .collect();
        let saves: Vec<_> = app
            .world_mut()
            .query_filtered::<(&Visibility, &ChildOf), With<PositionSave>>()
            .iter(app.world())
            .map(|(visibility, parent)| (drafts[&parent.parent()], *visibility))
            .collect();
        assert!(saves.contains(&(2, Visibility::Inherited)));
        assert!(saves.contains(&(7, Visibility::Hidden)));

        for pickable in app
            .world_mut()
            .query_filtered::<&Pickable, With<PositionNameField>>()
            .iter(app.world())
        {
            assert_eq!(pickable, &Pickable::default());
        }

        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        for unsupported in [
            "Skills",
            "War",
            "Diplomacy",
            "Create position",
            "Delete position",
        ] {
            assert!(!texts.contains(&unsupported.to_string()));
        }
    }
}
