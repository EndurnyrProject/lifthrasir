use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::guild::GuildState;
use net_contract::commands::{GuildMemberPositionRequested, GuildPositionEditRequested};
use net_contract::dto::GuildInfo;
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::{
    GuildMutationContext, GuildMutationControl, GuildPositionsList, GuildUi, PendingGuildMutation,
};

#[derive(Component, Clone, Debug, Default)]
pub(crate) struct PositionDraft {
    index: u32,
    can_invite: bool,
    can_expel: bool,
}

#[derive(Component, Default, Clone)]
pub(crate) struct PositionNameField;
#[derive(Component, Default, Clone)]
struct PositionInviteToggle;
#[derive(Component, Default, Clone)]
struct PositionExpelToggle;
#[derive(Component, Default, Clone)]
struct PositionSave;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionInviteLabel;
#[derive(Component, Default, Clone)]
pub(crate) struct PositionExpelLabel;

#[derive(Component, Clone, Debug, Default)]
struct AssignmentAction {
    target_char_id: u32,
    position_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PositionRow {
    pub index: u32,
    pub name: String,
    pub can_invite: bool,
    pub can_expel: bool,
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
                index: position.index,
                name: position.name.clone(),
                can_invite: position.can_invite,
                can_expel: position.can_expel,
                protected,
                editable: is_master && !protected,
            }
        })
        .collect();
    rows.sort_by_key(|row| row.index);
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
        .filter(|position| Some(position.index) != protected)
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
    let protected = master_position(info);
    if requester_char_id != info.master_char_id
        || Some(command.index) == protected
        || !info
            .positions
            .iter()
            .any(|position| position.index == command.index)
    {
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

pub(crate) fn refresh_positions(
    mut commands: Commands,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    container: Query<(Entity, Option<&Children>), With<GuildPositionsList>>,
) {
    let Ok((container, children)) = container.single() else {
        return;
    };
    let empty = children.is_none_or(|children| children.is_empty());
    if !empty && !guild.is_changed() && !session.is_changed() {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let Some(info) = guild.info() else {
        return;
    };
    let is_master = guild.is_master(session.char_id);
    let rows = project_positions(info, is_master);
    let assignments = project_assignments(info, is_master);
    commands
        .spawn_scene(position_management(rows, assignments, is_master))
        .insert(ChildOf(container));
}

pub(crate) fn sync_invite_labels(
    drafts: Query<&PositionDraft>,
    mut labels: Query<(&mut Text, &ChildOf), With<PositionInviteLabel>>,
) {
    for (mut label, parent) in &mut labels {
        if let Ok(draft) = drafts.get(parent.parent()) {
            label.0 = format!("Invite: {}", yes_no(draft.can_invite));
        }
    }
}

pub(crate) fn sync_expel_labels(
    drafts: Query<&PositionDraft>,
    mut labels: Query<(&mut Text, &ChildOf), With<PositionExpelLabel>>,
) {
    for (mut label, parent) in &mut labels {
        if let Ok(draft) = drafts.get(parent.parent()) {
            label.0 = format!("Expel: {}", yes_no(draft.can_expel));
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

fn parent_draft(
    control: Entity,
    parents: &Query<&ChildOf>,
    drafts: &Query<(), With<PositionDraft>>,
) -> Option<Entity> {
    let parent = parents.get(control).ok()?.parent();
    drafts.contains(parent).then_some(parent)
}

fn on_toggle_invite(
    event: On<Activate>,
    parents: Query<&ChildOf>,
    drafts: Query<(), With<PositionDraft>>,
    mut mutable_drafts: Query<&mut PositionDraft>,
) {
    let Some(row) = parent_draft(event.entity, &parents, &drafts) else {
        return;
    };
    if let Ok(mut draft) = mutable_drafts.get_mut(row) {
        draft.can_invite = !draft.can_invite;
    }
}

fn on_toggle_expel(
    event: On<Activate>,
    parents: Query<&ChildOf>,
    drafts: Query<(), With<PositionDraft>>,
    mut mutable_drafts: Query<&mut PositionDraft>,
) {
    let Some(row) = parent_draft(event.entity, &parents, &drafts) else {
        return;
    };
    if let Ok(mut draft) = mutable_drafts.get_mut(row) {
        draft.can_expel = !draft.can_expel;
    }
}

fn on_save_position(
    event: On<Activate>,
    parents: Query<&ChildOf>,
    drafts: Query<&PositionDraft>,
    names: Query<(&EditableText, &ChildOf), With<PositionNameField>>,
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
    let Some(name) = names
        .iter()
        .find(|(_, parent)| parent.parent() == row)
        .map(|(name, _)| name.value().to_string())
    else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if let Some(command) = request_position_edit(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        GuildPositionEditRequested {
            index: draft.index,
            name,
            can_invite: draft.can_invite,
            can_expel: draft.can_expel,
        },
    ) {
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

pub(crate) fn position_management(
    rows: Vec<PositionRow>,
    assignments: Vec<MemberAssignmentRow>,
    is_master: bool,
) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(position_row).collect();
    let assignments: Vec<_> = assignments.into_iter().map(assignment_row).collect();
    let assignment_visibility = if is_master {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [
            chrome_text("Fixed positions".to_string(), 13.0, theme::TEXT),
            chrome_text("Rename slots and control invitation or expulsion permission.".to_string(), 10.5, theme::TEXT_DIM),
            (Node { flex_direction: FlexDirection::Column, row_gap: px(6) } ignore_picking() Children [ {rows} ]),
            (
                template_value(assignment_visibility)
                Node { flex_direction: FlexDirection::Column, row_gap: px(6), padding: {UiRect::top(px(8))} }
                ignore_picking()
                Children [
                    chrome_text("Member assignments".to_string(), 13.0, theme::TEXT),
                    (Node { flex_direction: FlexDirection::Column, row_gap: px(6) } ignore_picking() Children [ {assignments} ]),
                ]
            ),
        ]
    }
}

fn position_row(row: PositionRow) -> impl Scene {
    let edit_visibility = if row.editable {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let read_visibility = if row.editable {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    let protected = if row.protected { " · Master" } else { "" };
    let editable = EditableText {
        max_characters: Some(24),
        ..EditableText::new(row.name.clone())
    };
    bsn! {
        template_value(PositionDraft {
            index: row.index,
            can_invite: row.can_invite,
            can_expel: row.can_expel,
        })
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(7),
            padding: {UiRect::axes(px(9), px(7))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        Children [
            (Node { width: px(55) } chrome_text(format!("#{}{}", row.index, protected), 10.5, theme::TEXT_DIM)),
            (
                template_value(read_visibility)
                Node { flex_grow: 1.0 }
                chrome_text(row.name, 12.5, theme::TEXT)
            ),
            (
                PositionNameField
                Pickable
                template_value(edit_visibility)
                template_value(editable)
                TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.5)} }
                TextColor(theme::TEXT)
                BackgroundColor(theme::GLASS_2)
                Node { flex_grow: 1.0, height: px(30), padding: {UiRect::axes(px(8), px(5))} }
            ),
            (PositionInviteLabel chrome_text(format!("Invite: {}", yes_no(row.can_invite)), 10.0, theme::TEXT_DIM)),
            (PositionExpelLabel chrome_text(format!("Expel: {}", yes_no(row.can_expel)), 10.0, theme::TEXT_DIM)),
            (
                PositionInviteToggle GuildMutationControl
                template_value(edit_visibility)
                @FeathersButton { @caption: bsn! { (Text("Invite") ThemedText) } }
                Node { width: px(65), height: px(30) }
                on(on_toggle_invite)
            ),
            (
                PositionExpelToggle GuildMutationControl
                template_value(edit_visibility)
                @FeathersButton { @caption: bsn! { (Text("Expel") ThemedText) } }
                Node { width: px(65), height: px(30) }
                on(on_toggle_expel)
            ),
            (
                PositionSave GuildMutationControl
                template_value(edit_visibility)
                @FeathersButton {
                    @caption: bsn! { (Text("Save") ThemedText) },
                    @variant: ButtonVariant::Primary,
                }
                Node { width: px(60), height: px(30) }
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
            column_gap: px(7),
            padding: {UiRect::axes(px(9), px(7))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        Children [
            (Node { width: px(130) } chrome_text(row.name, 12.0, theme::TEXT)),
            (Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: px(5), row_gap: px(5) } ignore_picking() Children [ {buttons} ]),
        ]
    }
}

fn assignment_button(
    target_char_id: u32,
    current_position: u32,
    position: PositionChoice,
) -> impl Scene {
    let caption = if current_position == position.index {
        format!("{} ✓", position.name)
    } else {
        position.name
    };
    bsn! {
        template_value(AssignmentAction { target_char_id, position_index: position.index })
        GuildMutationControl
        @FeathersButton { @caption: bsn! { (Text(caption) ThemedText) } }
        Node { height: px(28), padding: {UiRect::horizontal(px(8))} }
        on(on_assign_member)
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;
    use net_contract::commands::GuildPositionEditRequested;
    use net_contract::dto::{GuildInfo, GuildMemberInfo, GuildPositionInfo};
    use net_contract::state::ZoneSessionGeneration;

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
        }
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
    fn ordinary_members_and_the_protected_position_cannot_submit_edits() {
        let command = GuildPositionEditRequested {
            index: 2,
            name: "Officer".into(),
            can_invite: true,
            can_expel: false,
        };
        let mut member_ui = crate::widgets::guild_window::GuildUi::default();
        assert!(request_position_edit(
            &mut member_ui,
            ZoneSessionGeneration(1),
            &guild(),
            43,
            command,
        )
        .is_none());

        let mut master_ui = crate::widgets::guild_window::GuildUi::default();
        assert!(request_position_edit(
            &mut master_ui,
            ZoneSessionGeneration(1),
            &guild(),
            42,
            GuildPositionEditRequested {
                index: 7,
                name: "Renamed".into(),
                can_invite: false,
                can_expel: false,
            },
        )
        .is_none());
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
            "Tax",
            "Storage",
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
