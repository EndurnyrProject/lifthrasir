use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use game_engine::domain::guild::GuildState;
use net_contract::commands::GuildNoticeEditRequested;
use net_contract::dto::GuildInfo;
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};

use super::{
    GuildMutationContext, GuildMutationControl, GuildNoticeContent, GuildUi, PendingGuildMutation,
};

#[derive(Component, Default, Clone)]
pub(crate) struct GuildNoticeSubjectField;
#[derive(Component, Default, Clone)]
pub(crate) struct GuildNoticeBodyField;
#[derive(Component, Default, Clone)]
struct GuildNoticeSave;
#[derive(Component, Default, Clone)]
struct GuildNoticeEditControls;

pub(crate) fn request_notice_edit(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    subject: &str,
    body: &str,
) -> Option<GuildNoticeEditRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    if requester_char_id != info.master_char_id {
        ui.feedback = Some("Only the guild master can edit the notice.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "notice_edit",
        generation,
    });
    ui.feedback = Some("Saving notice…".to_string());
    ui.feedback_is_error = false;
    Some(GuildNoticeEditRequested {
        subject: subject.to_string(),
        body: body.to_string(),
    })
}

pub(crate) fn refresh_notice(
    mut commands: Commands,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    container: Query<(Entity, Option<&Children>), With<GuildNoticeContent>>,
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
    commands
        .spawn_scene(notice_management(
            info.notice_subject.clone(),
            info.notice_body.clone(),
            guild.is_master(session.char_id),
        ))
        .insert(ChildOf(container));
}

fn on_save_notice(
    _: On<Activate>,
    subject: Query<&EditableText, With<GuildNoticeSubjectField>>,
    body: Query<&EditableText, With<GuildNoticeBodyField>>,
    mut context: GuildMutationContext,
    mut writer: MessageWriter<GuildNoticeEditRequested>,
) {
    let Ok(subject) = subject.single() else {
        return;
    };
    let Ok(body) = body.single() else {
        return;
    };
    let Some(info) = context.guild.info() else {
        return;
    };
    if let Some(command) = request_notice_edit(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        &subject.value().to_string(),
        &body.value().to_string(),
    ) {
        writer.write(command);
    }
}

pub(crate) fn notice_management(subject: String, body: String, is_master: bool) -> impl Scene {
    let edit_visibility = if is_master {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let subject_editable = EditableText {
        max_characters: Some(60),
        ..EditableText::new(subject.clone())
    };
    let body_editable = EditableText {
        max_characters: Some(120),
        ..EditableText::new(body.clone())
    };
    let subject_display = if subject.is_empty() {
        "No subject".to_string()
    } else {
        subject
    };
    let body_display = if body.is_empty() {
        "No guild notice has been posted.".to_string()
    } else {
        body
    };
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch, row_gap: px(9) }
        ignore_picking()
        Children [
            chrome_text("Guild notice".to_string(), 13.0, theme::TEXT),
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(5), padding: {UiRect::all(px(10))}, border_radius: BorderRadius::all(px(8)) }
                BackgroundColor(theme::FIELD)
                ignore_picking()
                Children [
                    chrome_text(subject_display, 12.5, theme::TEXT),
                    chrome_text(body_display, 11.0, theme::TEXT_DIM),
                ]
            ),
            (
                GuildNoticeEditControls
                template_value(edit_visibility)
                Node { width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch, row_gap: px(7), padding: {UiRect::top(px(7))} }
                Pickable
                Children [
                    chrome_text("Edit notice".to_string(), 12.0, theme::TEXT),
                    (
                        GuildNoticeSubjectField
                        Pickable
                        template_value(subject_editable)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.5)} }
                        TextColor(theme::TEXT)
                        BackgroundColor(theme::FIELD)
                        Node { width: percent(100), height: px(34), padding: {UiRect::axes(px(9), px(7))}, border: px(1), border_radius: BorderRadius::all(px(7)) }
                        BorderColor::all(theme::STROKE)
                    ),
                    (
                        GuildNoticeBodyField
                        Pickable
                        template_value(body_editable)
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(12.5)} }
                        TextColor(theme::TEXT)
                        BackgroundColor(theme::FIELD)
                        Node { width: percent(100), height: px(82), padding: {UiRect::axes(px(9), px(7))}, border: px(1), border_radius: BorderRadius::all(px(7)) }
                        BorderColor::all(theme::STROKE)
                    ),
                    (
                        GuildNoticeSave GuildMutationControl
                        @FeathersButton {
                            @caption: bsn! { (Text("Save Notice") ThemedText) },
                            @variant: ButtonVariant::Primary,
                        }
                        Node { width: px(130), height: px(34) }
                        on(on_save_notice)
                    ),
                ]
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;
    use net_contract::dto::GuildInfo;
    use net_contract::state::ZoneSessionGeneration;

    use super::*;

    fn guild() -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 0,
            notice_subject: "Old subject".into(),
            notice_body: "Old body".into(),
            positions: vec![],
            members: vec![],
        }
    }

    #[test]
    fn master_notice_edit_writes_the_draft_and_waits_for_authoritative_refresh() {
        let info = guild();
        let mut ui = crate::widgets::guild_window::GuildUi::default();

        let command = request_notice_edit(
            &mut ui,
            ZoneSessionGeneration(3),
            &info,
            42,
            "Raid night",
            "Saturday at 20:00",
        )
        .unwrap();

        assert_eq!(command.subject, "Raid night");
        assert_eq!(command.body, "Saturday at 20:00");
        assert_eq!(ui.pending.as_ref().unwrap().action, "notice_edit");
        assert_eq!(info.notice_subject, "Old subject");
        assert_eq!(info.notice_body, "Old body");
    }

    #[test]
    fn notice_is_read_only_for_members_and_editable_for_the_master() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(notice_management("Subject".into(), "Body".into(), false))
            .unwrap();
        assert_eq!(
            *app.world_mut()
                .query_filtered::<&Visibility, With<GuildNoticeEditControls>>()
                .single(app.world())
                .unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<&EditableText, With<GuildNoticeSubjectField>>()
                .single(app.world())
                .unwrap()
                .value()
                .to_string(),
            "Subject"
        );

        let mut master = App::new();
        master.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        master.init_asset::<Image>();
        master.init_asset::<Font>();
        master
            .world_mut()
            .spawn_scene(notice_management("Subject".into(), "Body".into(), true))
            .unwrap();
        assert_eq!(
            *master
                .world_mut()
                .query_filtered::<&Visibility, With<GuildNoticeEditControls>>()
                .single(master.world())
                .unwrap(),
            Visibility::Inherited
        );
        for entity in [
            master
                .world_mut()
                .query_filtered::<Entity, With<GuildNoticeSubjectField>>()
                .single(master.world())
                .unwrap(),
            master
                .world_mut()
                .query_filtered::<Entity, With<GuildNoticeBodyField>>()
                .single(master.world())
                .unwrap(),
        ] {
            assert_eq!(
                master.world().get::<Node>(entity).unwrap().width,
                percent(100)
            );
            assert_eq!(
                master.world().get::<Pickable>(entity),
                Some(&Pickable::default())
            );
        }
    }
}
