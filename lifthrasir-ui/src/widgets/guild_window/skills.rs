use bevy::prelude::*;
use bevy::scene::EntityScene;
use game_engine::domain::guild::GuildState;
use game_engine::infrastructure::skill::SkillCatalog;
use net_contract::commands::GuildSkillUpRequested;
use net_contract::dto::{GuildErrorKind, GuildInfo};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use super::{
    GuildMutationContext, GuildMutationControl, GuildSkillsList, GuildUi, PendingGuildMutation,
    feedback::guild_action_error_text,
};
use crate::theme;
use crate::widgets::chrome::{chrome_text, ignore_picking};
use crate::widgets::info_modal::{InfoTarget, ShowInfoModal};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillRow {
    pub skill_id: u32,
    pub name: String,
    pub icon_path: Option<String>,
    pub level: u32,
    pub max_level: u32,
    pub can_upgrade: bool,
}

pub(crate) fn project_rows(
    info: &GuildInfo,
    requester_char_id: u32,
    catalog: Option<&SkillCatalog>,
) -> Vec<SkillRow> {
    info.skills
        .iter()
        .map(|skill| {
            let metadata = catalog.and_then(|catalog| catalog.get(skill.skill_id));
            SkillRow {
                skill_id: skill.skill_id,
                name: metadata
                    .filter(|metadata| !metadata.display_name.is_empty())
                    .map(|metadata| metadata.display_name.clone())
                    .unwrap_or_else(|| format!("Skill {}", skill.skill_id)),
                icon_path: catalog.and_then(|catalog| catalog.icon_path(skill.skill_id)),
                level: skill.level,
                max_level: skill.max_level,
                can_upgrade: requester_char_id != 0
                    && requester_char_id == info.master_char_id
                    && info.skill_points > 0
                    && skill.max_level > 0
                    && skill.level < skill.max_level,
            }
        })
        .collect()
}

#[derive(Component, Clone, Debug, Default)]
struct GuildSkillUpgrade(u32);

#[derive(Component, Clone, Debug, Default)]
struct GuildSkillCell(u32);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillRenderSignature {
    guild_id: Option<u32>,
    requester_char_id: u32,
    skill_points: u32,
    rows: Vec<SkillRow>,
}

pub(super) fn request_skill_up(
    ui: &mut GuildUi,
    generation: ZoneSessionGeneration,
    info: &GuildInfo,
    requester_char_id: u32,
    skill_id: u32,
) -> Option<GuildSkillUpRequested> {
    if ui.pending.is_some() {
        ui.feedback = Some("A guild action is already pending.".to_string());
        ui.feedback_is_error = true;
        return None;
    }
    if requester_char_id == 0 || requester_char_id != info.master_char_id {
        ui.feedback =
            Some(guild_action_error_text("skill_up", GuildErrorKind::NoPermission).to_string());
        ui.feedback_is_error = true;
        return None;
    }
    let Some(skill) = info.skills.iter().find(|skill| skill.skill_id == skill_id) else {
        ui.feedback = Some("Guild skill is unavailable.".to_string());
        ui.feedback_is_error = true;
        return None;
    };
    if info.skill_points == 0 {
        ui.feedback =
            Some(guild_action_error_text("skill_up", GuildErrorKind::NoSkillPoints).to_string());
        ui.feedback_is_error = true;
        return None;
    }
    if skill.max_level == 0 || skill.level >= skill.max_level {
        ui.feedback =
            Some(guild_action_error_text("skill_up", GuildErrorKind::SkillMaxed).to_string());
        ui.feedback_is_error = true;
        return None;
    }
    ui.pending = Some(PendingGuildMutation {
        action: "skill_up",
        generation,
    });
    ui.feedback = Some("Upgrading guild skill…".to_string());
    ui.feedback_is_error = false;
    Some(GuildSkillUpRequested { skill_id })
}

fn on_skill_up(
    mut click: On<Pointer<Click>>,
    buttons: Query<&GuildSkillUpgrade>,
    mut context: GuildMutationContext,
    mut writer: MessageWriter<GuildSkillUpRequested>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    click.propagate(false);
    let Some(info) = context.guild.info() else {
        return;
    };
    if let Some(command) = request_skill_up(
        &mut context.ui,
        *context.generation,
        info,
        context.session.char_id,
        button.0,
    ) {
        writer.write(command);
    }
}

/// Secondary-click on a cell opens the info modal for that guild skill.
fn on_cell_click(
    click: On<Pointer<Click>>,
    cells: Query<&GuildSkillCell>,
    mut writer: MessageWriter<ShowInfoModal>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Secondary {
        return;
    }
    writer.write(ShowInfoModal {
        target: InfoTarget::GuildSkill(cell.0),
    });
}

pub(crate) fn refresh_skills(
    mut commands: Commands,
    guild: Res<GuildState>,
    session: Res<ZoneSession>,
    catalog: Option<Res<SkillCatalog>>,
    container: Query<(Entity, Option<&Children>), With<GuildSkillsList>>,
    mut rendered: Local<Option<SkillRenderSignature>>,
) {
    let Ok((container, children)) = container.single() else {
        return;
    };
    let signature = SkillRenderSignature {
        guild_id: guild.info().map(|info| info.guild_id),
        requester_char_id: session.char_id,
        skill_points: guild.info().map(|info| info.skill_points).unwrap_or(0),
        rows: guild
            .info()
            .map(|info| project_rows(info, session.char_id, catalog.as_deref()))
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
    if signature.guild_id.is_some() {
        commands
            .spawn_scene(skill_grid(signature.rows.clone(), signature.skill_points))
            .insert(ChildOf(container));
    }
    *rendered = Some(signature);
}

pub(crate) fn skill_grid(rows: Vec<SkillRow>, skill_points: u32) -> impl Scene {
    let cells: Vec<_> = rows.into_iter().map(skill_cell).collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                }
                ignore_picking()
                Children [
                    chrome_text("Guild skills".to_string(), 13.0, theme::TEXT),
                    chrome_text(format!("{skill_points} points"), 10.5, theme::GOLD),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(10),
                    row_gap: px(10),
                }
                ignore_picking()
                Children [ {cells} ]
            ),
        ]
    }
}

fn skill_cell(row: SkillRow) -> impl Scene {
    let icon = row.icon_path.map(|path| EntityScene(skill_icon(path)));
    let upgrade = row
        .can_upgrade
        .then(|| EntityScene(upgrade_button(row.skill_id)));
    let level_color = if row.max_level > 0 && row.level >= row.max_level {
        theme::GOLD
    } else if row.level > 0 {
        theme::EMERALD_BRI
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        template_value(GuildSkillCell(row.skill_id))
        Node {
            width: px(62),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(4),
            padding: {UiRect::vertical(px(3))},
            border_radius: BorderRadius::all(px(8)),
        }
        Pickable
        on(on_cell_click)
        Children [
            (
                Node {
                    width: px(44), height: px(44),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(10)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(theme::STROKE)
                ignore_picking()
                Children [ {icon} ]
            ),
            chrome_text(row.name, 8.5, theme::TEXT_FAINT),
            (
                Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(3) }
                ignore_picking()
                Children [
                    chrome_text(format!("{}/{}", row.level, row.max_level), 9.0, level_color),
                    {upgrade},
                ]
            ),
        ]
    }
}

fn upgrade_button(skill_id: u32) -> impl Scene {
    bsn! {
        template_value(GuildSkillUpgrade(skill_id))
        GuildMutationControl
        Node {
            width: px(14), height: px(14),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor(theme::EMERALD)
        Pickable
        on(on_skill_up)
        Children [ chrome_text("+".to_string(), 10.0, theme::EMERALD_INK) ]
    }
}

fn skill_icon(path: String) -> impl Scene {
    bsn! {
        ImageNode { image: {path} }
        Node { width: px(30), height: px(30) }
        ignore_picking()
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::ScenePlugin;
    use game_engine::domain::guild::{GuildPlugin, GuildState, GuildSystems};
    use game_engine::infrastructure::skill::SkillCatalog;
    use lifthrasir_data::{SkillData, SkillMeta};
    use net_contract::dto::{GuildActionResult, GuildErrorKind, GuildInfo, GuildSkillInfo};
    use net_contract::events::{GuildIngress, GuildIngressPayload, ZoneDisconnected};
    use net_contract::state::{ZoneSession, ZoneSessionGeneration};

    use super::*;
    use crate::widgets::guild_window::{GuildUi, GuildUiSession, emblem, feedback};

    fn guild() -> GuildInfo {
        GuildInfo {
            guild_id: 7,
            name: "Vikings".into(),
            master_char_id: 42,
            emblem_id: 0,
            notice_subject: String::new(),
            notice_body: String::new(),
            positions: vec![],
            members: vec![],
            level: 3,
            exp: 500,
            next_exp: 1_000,
            skill_points: 2,
            skills: vec![GuildSkillInfo {
                skill_id: 10_000,
                level: 1,
                max_level: 5,
            }],
            relations: vec![],
        }
    }

    fn catalog() -> SkillCatalog {
        let mut data = SkillData::default();
        data.skills.insert(
            10_000,
            SkillMeta {
                name: "GD_GLORYGUILD".into(),
                display_name: "Guild Glory".into(),
                description: vec!["Raises ^00ff00guild prestige^000000.".into()],
                max_level: 99,
                sp_cost: vec![],
                attack_range: vec![],
            },
        );
        SkillCatalog::from_skill_data(data)
    }

    #[test]
    fn server_rows_include_zero_level_unknown_and_maximum_zero_skills() {
        let mut info = guild();
        info.skills = vec![
            GuildSkillInfo {
                skill_id: 10_000,
                level: 0,
                max_level: 5,
            },
            GuildSkillInfo {
                skill_id: 99_999,
                level: 0,
                max_level: 2,
            },
            GuildSkillInfo {
                skill_id: 99_998,
                level: 0,
                max_level: 0,
            },
        ];

        let rows = project_rows(&info, 42, Some(&catalog()));

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, "Guild Glory");
        assert_eq!(
            rows[0].icon_path.as_deref(),
            Some("ro://data/texture/유저인터페이스/item/gd_gloryguild.bmp")
        );
        assert_eq!(rows[0].level, 0);
        assert_eq!(rows[0].max_level, 5);
        assert!(rows[0].can_upgrade);
        assert_eq!(rows[1].name, "Skill 99999");
        assert_eq!(rows[1].max_level, 2);
        assert!(rows[1].can_upgrade);
        assert_eq!(rows[2].name, "Skill 99998");
        assert_eq!(rows[2].max_level, 0);
        assert!(!rows[2].can_upgrade);
    }

    #[test]
    fn unknown_metadata_scene_keeps_upgradeable_id_row_and_zero_maximum_read_only() {
        let mut info = guild();
        info.skills = vec![
            GuildSkillInfo {
                skill_id: 99_999,
                level: 0,
                max_level: 2,
            },
            GuildSkillInfo {
                skill_id: 99_998,
                level: 0,
                max_level: 0,
            },
        ];
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.world_mut()
            .spawn_scene(skill_grid(project_rows(&info, 42, None), 2))
            .unwrap();

        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains(&"Skill 99999".to_string()));
        assert!(texts.contains(&"Skill 99998".to_string()));
        assert!(texts.contains(&"2 points".to_string()));
        let controls: Vec<_> = app
            .world_mut()
            .query::<&GuildSkillUpgrade>()
            .iter(app.world())
            .map(|control| control.0)
            .collect();
        assert_eq!(controls, vec![99_999]);
    }

    fn click_event(target: Entity, button: PointerButton) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        use std::time::Duration;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(target)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button,
                hit: HitData::new(target, 0.0, None, None),
                duration: Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    #[test]
    fn secondary_cell_click_opens_the_guild_skill_info_modal() {
        let mut app = skill_app();
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild()),
        });
        app.update();
        let cell = app
            .world_mut()
            .query_filtered::<Entity, With<GuildSkillCell>>()
            .single(app.world())
            .unwrap();

        app.world_mut()
            .trigger(click_event(cell, PointerButton::Secondary));

        let requests: Vec<_> = app
            .world()
            .resource::<Messages<ShowInfoModal>>()
            .iter_current_update_messages()
            .map(|request| request.target)
            .collect();
        assert_eq!(requests, vec![InfoTarget::GuildSkill(10_000)]);
        assert!(
            app.world()
                .resource::<Messages<GuildSkillUpRequested>>()
                .is_empty()
        );
    }

    fn skill_app() -> App {
        let generation = ZoneSessionGeneration(9);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<GuildIngress>()
            .add_message::<ZoneDisconnected>()
            .add_message::<GuildSkillUpRequested>()
            .add_message::<ShowInfoModal>()
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
            .init_resource::<emblem::GuildEmblemPreview>()
            .insert_resource(catalog())
            .add_plugins(GuildPlugin);
        app.world_mut().spawn(GuildSkillsList);
        app.add_systems(
            Update,
            (feedback::apply_guild_results, refresh_skills)
                .chain()
                .in_set(GuildSystems::UiSync),
        );
        app
    }

    #[test]
    fn clicking_upgrade_twice_emits_one_command_and_waits_for_snapshot() {
        let mut app = skill_app();
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild()),
        });
        app.update();
        let button = app
            .world_mut()
            .query_filtered::<Entity, With<GuildSkillUpgrade>>()
            .single(app.world())
            .unwrap();

        app.world_mut()
            .trigger(click_event(button, PointerButton::Primary));
        app.world_mut()
            .trigger(click_event(button, PointerButton::Primary));

        let commands = app
            .world()
            .resource::<Messages<GuildSkillUpRequested>>()
            .len();
        assert_eq!(commands, 1);
        let info = app.world().resource::<GuildState>().info().unwrap();
        assert_eq!(info.skill_points, 2);
        assert_eq!(info.skills[0].level, 1);
    }

    #[test]
    fn successful_result_does_not_change_skill_state_before_snapshot() {
        let mut app = skill_app();
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild()),
        });
        app.update();
        let button = app
            .world_mut()
            .query_filtered::<Entity, With<GuildSkillUpgrade>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .trigger(click_event(button, PointerButton::Primary));
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "skill_up".into(),
                success: true,
                error: GuildErrorKind::None,
            }),
        });

        app.update();

        let ui = app.world().resource::<GuildUi>();
        assert!(ui.pending.is_none());
        let info = app.world().resource::<GuildState>().info().unwrap();
        assert_eq!(info.skill_points, 2);
        assert_eq!(info.skills[0].level, 1);
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "1/5")
        );
    }

    #[test]
    fn authoritative_snapshot_rebuilds_skill_level_and_point_guard() {
        let mut app = skill_app();
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild()),
        });
        app.update();

        let mut updated = guild();
        updated.skills[0].level = 2;
        updated.skill_points = 0;
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(updated),
        });
        app.update();

        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.contains(&"2/5".to_string()));
        assert!(texts.contains(&"0 points".to_string()));
        let upgrades = app
            .world_mut()
            .query::<&GuildSkillUpgrade>()
            .iter(app.world())
            .count();
        assert_eq!(upgrades, 0);
    }

    #[test]
    fn rejected_upgrade_uses_shared_feedback_and_releases_pending() {
        let mut app = skill_app();
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::Info(guild()),
        });
        app.update();
        let button = app
            .world_mut()
            .query_filtered::<Entity, With<GuildSkillUpgrade>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .trigger(click_event(button, PointerButton::Primary));
        app.world_mut().write_message(GuildIngress {
            generation: ZoneSessionGeneration(9),
            payload: GuildIngressPayload::ActionResult(GuildActionResult {
                action: "skill_up".into(),
                success: false,
                error: GuildErrorKind::SkillRequirement,
            }),
        });

        app.update();

        let ui = app.world().resource::<GuildUi>();
        assert!(ui.pending.is_none());
        assert_eq!(
            ui.feedback.as_deref(),
            Some("Guild skill requirements are not met")
        );
    }

    #[test]
    fn member_cannot_request_a_guild_skill_upgrade() {
        let mut ui = GuildUi::default();

        assert!(
            request_skill_up(&mut ui, ZoneSessionGeneration(9), &guild(), 43, 10_000).is_none()
        );
        assert_eq!(
            ui.feedback.as_deref(),
            Some("Only the guild master can do that")
        );
        assert!(ui.pending.is_none());
    }

    #[test]
    fn pending_mutation_prevents_another_guild_skill_command() {
        let generation = ZoneSessionGeneration(9);
        let mut ui = GuildUi {
            pending: Some(PendingGuildMutation {
                action: "notice_edit",
                generation,
            }),
            ..Default::default()
        };

        assert!(request_skill_up(&mut ui, generation, &guild(), 42, 10_000).is_none());
        assert_eq!(ui.pending.as_ref().unwrap().action, "notice_edit");
    }

    #[test]
    fn no_points_prevents_a_guild_skill_command() {
        let mut info = guild();
        info.skill_points = 0;
        let mut ui = GuildUi::default();

        assert!(request_skill_up(&mut ui, ZoneSessionGeneration(9), &info, 42, 10_000).is_none());
        assert_eq!(
            ui.feedback.as_deref(),
            Some("No guild skill points available")
        );
        assert!(ui.pending.is_none());
    }

    #[test]
    fn maximum_zero_and_legacy_maxed_skills_are_read_only() {
        for (level, max_level) in [(0, 0), (5, 5), (7, 5)] {
            let mut info = guild();
            info.skills[0].level = level;
            info.skills[0].max_level = max_level;
            let mut ui = GuildUi::default();

            assert!(
                request_skill_up(&mut ui, ZoneSessionGeneration(9), &info, 42, 10_000).is_none()
            );
            assert_eq!(
                ui.feedback.as_deref(),
                Some("Guild skill is already at maximum level")
            );
            assert!(ui.pending.is_none());
        }
    }

    #[test]
    fn skill_id_absent_from_snapshot_cannot_be_requested() {
        let mut ui = GuildUi::default();

        assert!(
            request_skill_up(&mut ui, ZoneSessionGeneration(9), &guild(), 42, 99_999).is_none()
        );
        assert_eq!(ui.feedback.as_deref(), Some("Guild skill is unavailable."));
        assert!(ui.pending.is_none());
    }

    #[test]
    fn valid_master_upgrade_emits_one_point_command_without_mutating_snapshot() {
        let info = guild();
        let mut ui = GuildUi::default();
        let generation = ZoneSessionGeneration(9);

        let command = request_skill_up(&mut ui, generation, &info, 42, 10_000).unwrap();

        assert_eq!(command.skill_id, 10_000);
        assert_eq!(ui.pending.as_ref().unwrap().action, "skill_up");
        assert_eq!(info.skill_points, 2);
        assert_eq!(info.skills[0].level, 1);
    }
}
