use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::guild::GuildState;
use net_contract::commands::{GuildAllianceRequested, GuildAntagonistRemoveRequested};
use net_contract::dto::{GuildErrorKind, GuildInfo, GuildRelationInfo, GuildRelationKind};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use super::feedback::guild_action_error_text;
use super::relation_dialogs::PendingRelationConfirmation;
use super::{GuildMutationContext, GuildMutationControl, GuildUi, PendingGuildMutation};
use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

#[derive(Component, Default, Clone)]
pub(crate) struct GuildAllianceNameField;
#[derive(Component, Default, Clone)]
pub(crate) struct GuildAntagonistNameField;
#[derive(Component, Default, Clone)]
pub(crate) struct GuildRelationMasterControls;
#[derive(Component, Default, Clone)]
pub(crate) struct GuildRelationsList;

#[derive(Component, Clone, Debug, Default)]
struct GuildAllianceBreakButton {
    guild_id: u32,
    guild_name: String,
}

#[derive(Component, Clone, Debug, Default)]
struct GuildAntagonistRemoveButton {
    guild_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelationRenderSignature {
    guild_id: Option<u32>,
    requester_char_id: u32,
    can_manage: bool,
    relations: Vec<GuildRelationInfo>,
}

pub(crate) fn request_alliance(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    raw_name: &str,
) -> Option<GuildAllianceRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".into());
        ui.feedback_is_error = true;
        return None;
    }
    if requester_char_id == 0 || requester_char_id != info.master_char_id {
        ui.feedback =
            Some(guild_action_error_text("alliance_request", GuildErrorKind::NoPermission).into());
        ui.feedback_is_error = true;
        return None;
    }
    let target_name = raw_name.trim();
    if target_name.is_empty() {
        ui.feedback = Some("Enter an online guild master's character name.".into());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "alliance_request",
        generation,
    });
    ui.feedback = Some("Sending alliance request…".into());
    ui.feedback_is_error = false;
    Some(GuildAllianceRequested {
        target_char_id: 0,
        target_name: target_name.into(),
    })
}

fn request_antagonist_remove(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    guild_id: u32,
) -> Option<GuildAntagonistRemoveRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".into());
        ui.feedback_is_error = true;
        return None;
    }
    if requester_char_id == 0 || requester_char_id != info.master_char_id {
        ui.feedback =
            Some(guild_action_error_text("antagonist_remove", GuildErrorKind::NoPermission).into());
        ui.feedback_is_error = true;
        return None;
    }
    let valid = info.relations.iter().any(|relation| {
        relation.guild_id == guild_id && relation.kind == GuildRelationKind::Antagonist
    });
    if !valid {
        ui.feedback =
            Some(guild_action_error_text("antagonist_remove", GuildErrorKind::NotRelated).into());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "antagonist_remove",
        generation,
    });
    ui.feedback = Some("Removing antagonist…".into());
    ui.feedback_is_error = false;
    Some(GuildAntagonistRemoveRequested { guild_id })
}

pub(crate) fn on_request_alliance(
    _: On<Activate>,
    field: Query<&EditableText, With<GuildAllianceNameField>>,
    mut context: GuildMutationContext,
    mut writer: MessageWriter<GuildAllianceRequested>,
) {
    let Ok(field) = field.single() else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if let Some(command) = request_alliance(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        &field.value().to_string(),
    ) {
        writer.write(command);
    }
}

pub(crate) fn on_declare_antagonist(
    _: On<Activate>,
    field: Query<&EditableText, With<GuildAntagonistNameField>>,
    mut context: GuildMutationContext,
    mut confirmation: ResMut<PendingRelationConfirmation>,
) {
    let Ok(field) = field.single() else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if context.ui.pending.is_some() || confirmation.is_pending() {
        context.ui.feedback = Some("A guild action is already pending.".into());
        context.ui.feedback_is_error = true;
        return;
    }
    if !context.guild.is_master(context.session.char_id) {
        context.ui.feedback =
            Some(guild_action_error_text("antagonist", GuildErrorKind::NoPermission).into());
        context.ui.feedback_is_error = true;
        return;
    }
    let target_name = field.value().to_string();
    let target_name = target_name.trim();
    if target_name.is_empty() {
        context.ui.feedback = Some("Enter a character name from the target guild.".into());
        context.ui.feedback_is_error = true;
        return;
    }
    confirmation.declare_antagonist(*context.generation, info.guild_id, target_name.to_string());
}

fn on_break_alliance(
    activate: On<Activate>,
    buttons: Query<&GuildAllianceBreakButton>,
    context: GuildMutationContext,
    mut confirmation: ResMut<PendingRelationConfirmation>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if context.ui.pending.is_some()
        || confirmation.is_pending()
        || !context.guild.is_master(context.session.char_id)
    {
        return;
    }
    let valid = info.relations.iter().any(|relation| {
        relation.guild_id == button.guild_id && relation.kind == GuildRelationKind::Ally
    });
    if !valid {
        return;
    }
    confirmation.break_alliance(
        *context.generation,
        info.guild_id,
        button.guild_id,
        button.guild_name.clone(),
    );
}

fn on_remove_antagonist(
    activate: On<Activate>,
    buttons: Query<&GuildAntagonistRemoveButton>,
    mut context: GuildMutationContext,
    mut writer: MessageWriter<GuildAntagonistRemoveRequested>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if let Some(command) = request_antagonist_remove(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        button.guild_id,
    ) {
        writer.write(command);
    }
}

pub(crate) fn sync_relation_controls(
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    mut controls: Query<(&mut Visibility, &mut Node), With<GuildRelationMasterControls>>,
) {
    let visible = guild.is_master(session.char_id);
    for (mut visibility, mut node) in &mut controls {
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn refresh_relations(
    mut commands: Commands,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    container: Query<(Entity, Option<&Children>), With<GuildRelationsList>>,
    mut rendered: Local<Option<RelationRenderSignature>>,
) {
    let Ok((container, children)) = container.single() else {
        return;
    };
    let signature = RelationRenderSignature {
        guild_id: guild.info().map(|info| info.guild_id),
        requester_char_id: session.char_id,
        can_manage: guild.is_master(session.char_id),
        relations: guild
            .info()
            .map(|info| info.relations.clone())
            .unwrap_or_default(),
    };
    let empty = children.is_none_or(|children| children.is_empty());
    if !empty && rendered.as_ref() == Some(&signature) {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if let Some(info) = guild.info() {
        commands
            .spawn_scene(relation_lists(
                info.relations.clone(),
                guild.is_master(session.char_id),
            ))
            .insert(ChildOf(container));
    }
    *rendered = Some(signature);
}

pub(crate) fn relation_controls() -> impl Scene {
    bsn! {
        GuildRelationMasterControls
        Node { width: percent(100), display: Display::None, flex_direction: FlexDirection::Column, row_gap: px(8) }
        Visibility::Hidden
        Pickable
        Children [ alliance_request_control(), antagonist_declaration_control() ]
    }
}

fn alliance_request_control() -> impl Scene {
    let editable = EditableText {
        max_characters: Some(24),
        ..default()
    };
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(4) }
        ignore_picking()
        Children [
            chrome_text("Alliance request".to_string(), 11.0, theme::GOLD),
            chrome_text("Online guild master's character name".to_string(), 9.5, theme::TEXT_DIM),
            (
                Node { width: percent(100), flex_direction: FlexDirection::Row, column_gap: px(8), align_items: AlignItems::Center }
                Pickable
                Children [
                    (
                        GuildAllianceNameField
                        Pickable
                        template_value(editable)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.0)} }
                        TextColor(theme::TEXT)
                        BackgroundColor(theme::FIELD)
                        Node { flex_grow: 1.0, height: px(34), padding: {UiRect::axes(px(9), px(7))}, border: px(1), border_radius: BorderRadius::all(px(8)) }
                        BorderColor::all(theme::STROKE)
                    ),
                    (
                        GuildMutationControl
                        @FeathersButton { @caption: bsn! { (Text("Request Alliance") ThemedText) }, @variant: ButtonVariant::Primary }
                        Node { width: px(155), height: px(34) }
                        on(on_request_alliance)
                    ),
                ]
            ),
        ]
    }
}

fn antagonist_declaration_control() -> impl Scene {
    let editable = EditableText {
        max_characters: Some(24),
        ..default()
    };
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(4) }
        ignore_picking()
        Children [
            chrome_text("Declare antagonist".to_string(), 11.0, theme::GOLD),
            chrome_text("Character name from the target guild".to_string(), 9.5, theme::TEXT_DIM),
            (
                Node { width: percent(100), flex_direction: FlexDirection::Row, column_gap: px(8), align_items: AlignItems::Center }
                Pickable
                Children [
                    (
                        GuildAntagonistNameField
                        Pickable
                        template_value(editable)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.0)} }
                        TextColor(theme::TEXT)
                        BackgroundColor(theme::FIELD)
                        Node { flex_grow: 1.0, height: px(34), padding: {UiRect::axes(px(9), px(7))}, border: px(1), border_radius: BorderRadius::all(px(8)) }
                        BorderColor::all(theme::STROKE)
                    ),
                    (
                        GuildMutationControl
                        @FeathersButton { @caption: bsn! { (Text("Declare Antagonist") ThemedText) }, @variant: ButtonVariant::Primary }
                        Node { width: px(155), height: px(34) }
                        on(on_declare_antagonist)
                    ),
                ]
            ),
        ]
    }
}

pub(crate) fn relation_lists(relations: Vec<GuildRelationInfo>, can_manage: bool) -> impl Scene {
    let allies: Vec<_> = relations
        .iter()
        .filter(|relation| relation.kind == GuildRelationKind::Ally)
        .cloned()
        .map(|relation| ally_row(relation, can_manage))
        .collect();
    let antagonists: Vec<_> = relations
        .iter()
        .filter(|relation| relation.kind == GuildRelationKind::Antagonist)
        .cloned()
        .map(|relation| antagonist_row(relation, can_manage))
        .collect();
    let unsupported: Vec<_> = relations
        .into_iter()
        .filter_map(|relation| match relation.kind {
            GuildRelationKind::Unknown(value) => Some(unsupported_row(relation, value)),
            GuildRelationKind::Ally | GuildRelationKind::Antagonist => None,
        })
        .collect();
    let unsupported_section = (!unsupported.is_empty()).then(|| {
        EntityScene(relation_section(
            format!("Unsupported relations ({})", unsupported.len()),
            unsupported,
        ))
    });
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [
            relation_section(format!("Allies ({})", allies.len()), allies),
            relation_section(format!("Antagonists ({})", antagonists.len()), antagonists),
            {unsupported_section},
        ]
    }
}

fn relation_section<S: Scene>(title: String, rows: Vec<S>) -> impl Scene {
    let empty = rows
        .is_empty()
        .then(|| EntityScene(chrome_text("None".to_string(), 10.5, theme::TEXT_FAINT)));
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(5) }
        ignore_picking()
        Children [
            chrome_text(title, 12.5, theme::TEXT),
            {empty},
            (Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(5) } ignore_picking() Children [ {rows} ]),
        ]
    }
}

fn ally_row(relation: GuildRelationInfo, can_manage: bool) -> impl Scene {
    let visibility = if can_manage {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let display = if can_manage {
        Display::Flex
    } else {
        Display::None
    };
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: {UiRect::axes(px(10), px(7))}, border_radius: BorderRadius::all(px(8)) }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            (Node { flex_grow: 1.0 } chrome_text(relation.name.clone(), 12.0, theme::TEXT)),
            (
                template_value(GuildAllianceBreakButton { guild_id: relation.guild_id, guild_name: relation.name })
                GuildMutationControl
                template_value(visibility)
                @FeathersButton { @caption: bsn! { (Text("Break Alliance") ThemedText) }, @variant: ButtonVariant::Normal }
                Node { width: px(125), height: px(30), display: {display} }
                on(on_break_alliance)
            ),
        ]
    }
}

fn antagonist_row(relation: GuildRelationInfo, can_manage: bool) -> impl Scene {
    let visibility = if can_manage {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let display = if can_manage {
        Display::Flex
    } else {
        Display::None
    };
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: {UiRect::axes(px(10), px(7))}, border_radius: BorderRadius::all(px(8)) }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            (Node { flex_grow: 1.0 } chrome_text(relation.name, 12.0, theme::TEXT)),
            (
                template_value(GuildAntagonistRemoveButton { guild_id: relation.guild_id })
                GuildMutationControl
                template_value(visibility)
                @FeathersButton { @caption: bsn! { (Text("Remove") ThemedText) }, @variant: ButtonVariant::Normal }
                Node { width: px(90), height: px(30), display: {display} }
                on(on_remove_antagonist)
            ),
        ]
    }
}

fn unsupported_row(relation: GuildRelationInfo, value: i32) -> impl Scene {
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, padding: {UiRect::axes(px(10), px(7))}, row_gap: px(2), border_radius: BorderRadius::all(px(8)) }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            chrome_text(relation.name, 12.0, theme::TEXT),
            chrome_text(format!("Unsupported relation type {value} · read-only"), 9.5, theme::WARN),
        ]
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;
    use net_contract::dto::GuildRelationInfo;

    use super::*;

    fn info(relations: Vec<GuildRelationInfo>) -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
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
            relations,
        }
    }

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    #[test]
    fn alliance_request_trims_master_character_name_and_uses_name_targeting() {
        let mut ui = GuildUi::default();

        let command = request_alliance(
            &mut ui,
            ZoneSessionGeneration(3),
            &info(vec![]),
            42,
            "  Freya  ",
        )
        .unwrap();

        assert_eq!(command.target_char_id, 0);
        assert_eq!(command.target_name, "Freya");
        assert_eq!(ui.pending.as_ref().unwrap().action, "alliance_request");
    }

    #[test]
    fn alliance_request_rejects_empty_and_non_master_inputs() {
        let mut empty = GuildUi::default();
        assert!(
            request_alliance(
                &mut empty,
                ZoneSessionGeneration(3),
                &info(vec![]),
                42,
                "  ",
            )
            .is_none()
        );
        assert!(empty.pending.is_none());

        let mut member = GuildUi::default();
        assert!(
            request_alliance(
                &mut member,
                ZoneSessionGeneration(3),
                &info(vec![]),
                43,
                "Freya",
            )
            .is_none()
        );
        assert!(member.pending.is_none());
    }

    #[test]
    fn relation_scene_separates_known_kinds_and_keeps_unknown_visible_read_only() {
        let mut app = scene_app();
        app.world_mut()
            .spawn_scene(relation_lists(
                vec![
                    GuildRelationInfo {
                        guild_id: 8,
                        name: "Aesir".into(),
                        kind: GuildRelationKind::Ally,
                    },
                    GuildRelationInfo {
                        guild_id: 9,
                        name: "Jotun".into(),
                        kind: GuildRelationKind::Antagonist,
                    },
                    GuildRelationInfo {
                        guild_id: 10,
                        name: "Mystery".into(),
                        kind: GuildRelationKind::Unknown(77),
                    },
                ],
                false,
            ))
            .unwrap();

        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains(&"Allies (1)".to_string()));
        assert!(texts.contains(&"Antagonists (1)".to_string()));
        assert!(texts.contains(&"Unsupported relations (1)".to_string()));
        assert!(texts.contains(&"Unsupported relation type 77 · read-only".to_string()));
        assert!(
            app.world_mut()
                .query_filtered::<&Visibility, With<GuildAllianceBreakButton>>()
                .iter(app.world())
                .all(|visibility| *visibility == Visibility::Hidden)
        );
        assert!(
            app.world_mut()
                .query_filtered::<&Visibility, With<GuildAntagonistRemoveButton>>()
                .iter(app.world())
                .all(|visibility| *visibility == Visibility::Hidden)
        );
    }

    #[test]
    fn antagonist_remove_revalidates_selected_snapshot_relation() {
        let relations = vec![GuildRelationInfo {
            guild_id: 9,
            name: "Jotun".into(),
            kind: GuildRelationKind::Antagonist,
        }];
        let mut ui = GuildUi::default();

        let command =
            request_antagonist_remove(&mut ui, ZoneSessionGeneration(4), &info(relations), 42, 9)
                .unwrap();

        assert_eq!(command.guild_id, 9);
        assert_eq!(ui.pending.as_ref().unwrap().action, "antagonist_remove");
    }

    #[test]
    fn action_acknowledgement_does_not_change_lists_before_authoritative_snapshot() {
        use game_engine::domain::guild::{GuildPlugin, GuildSystems};
        use net_contract::dto::{GuildActionResult, GuildErrorKind};
        use net_contract::events::{GuildIngress, GuildIngressPayload, ZoneDisconnected};

        let generation = ZoneSessionGeneration(5);
        let mut app = scene_app();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .insert_resource(generation)
            .insert_resource(ZoneSession {
                char_id: 42,
                ..default()
            })
            .add_plugins(GuildPlugin)
            .add_systems(Update, refresh_relations.in_set(GuildSystems::UiSync));
        app.world_mut().spawn(GuildRelationsList);
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(info(vec![GuildRelationInfo {
                guild_id: 8,
                name: "Aesir".into(),
                kind: GuildRelationKind::Ally,
            }])),
        });
        app.update();
        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "alliance_break".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        });

        app.update();

        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "Aesir")
        );

        app.world_mut().write_message(GuildIngress {
            generation,
            payload: GuildIngressPayload::Info(info(vec![])),
        });
        app.update();
        assert!(
            !app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "Aesir")
        );
    }
}
