use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::{ButtonVariant, FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemedText};

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::{chrome_text, ignore_picking, titlebar};

use super::members::MemberRow;
use super::*;

const WINDOW_LEFT: f32 = 250.0;
const WINDOW_TOP: f32 = 70.0;
pub(crate) const GUILD_WINDOW_WIDTH: f32 = 690.0;

pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        GuildWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(GUILD_WINDOW_WIDTH),
            max_height: px(650),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [
            titlebar::<GuildTitlebar, GuildWindowRoot>("members", "Guild"),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: {UiRect::all(px(16))},
                }
                ignore_picking()
                Children [ guild_panel() ]
            ),
        ]
    }
}

fn guild_panel() -> impl Scene {
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(10) }
        ignore_picking()
        Children [ header(), tabs(), content(), feedback_banner(), leave_control() ]
    }
}

fn leave_control() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            padding: {UiRect::top(px(5))},
        }
        Pickable
        Children [
            (
                GuildLeaveButton
                GuildMutationControl
                @FeathersButton {
                    @caption: bsn! { (Text("Leave Guild") ThemedText) },
                    @variant: ButtonVariant::Normal,
                }
                Node { width: px(170), height: px(36) }
                on(super::dialogs::on_leave)
            ),
        ]
    }
}

fn header() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(14),
            padding: {UiRect::all(px(12))},
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor(theme::GLASS_2)
        ignore_picking()
        Children [
            (
                Node {
                    width: px(58), height: px(58),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: px(1),
                    border_radius: BorderRadius::all(px(10)),
                }
                BackgroundColor(theme::FIELD)
                BorderColor::all(theme::GOLD_FAINT)
                ignore_picking()
                Children [
                    (GuildHeaderEmblemFallback title_text("G".to_string(), 22.0, theme::GOLD)),
                    (
                        GuildHeaderEmblemImage
                        ImageNode {}
                        Node { width: px(48), height: px(48) }
                        Visibility::Hidden
                    ),
                ]
            ),
            (
                Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(4) }
                ignore_picking()
                Children [
                    (GuildNameText title_text(String::new(), 21.0, theme::TEXT)),
                    (GuildMasterText chrome_text(String::new(), 11.5, theme::TEXT_DIM)),
                    (GuildNoticeText chrome_text(String::new(), 11.0, theme::TEXT_FAINT)),
                    (
                        Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween }
                        ignore_picking()
                        Children [
                            (GuildLevelText chrome_text(String::new(), 10.5, theme::GOLD)),
                            (GuildExpText chrome_text(String::new(), 10.5, theme::TEXT_DIM)),
                        ]
                    ),
                    (
                        Node {
                            width: percent(100), height: px(7),
                            border_radius: BorderRadius::all(px(4)),
                            overflow: {Overflow::clip()},
                        }
                        BackgroundColor(theme::FIELD)
                        ignore_picking()
                        Children [
                            (
                                GuildExpFill
                                Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(4)) }
                                BackgroundColor(theme::GOLD)
                                ignore_picking()
                            ),
                        ]
                    ),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(4) }
                ignore_picking()
                Children [
                    (GuildMemberCountText chrome_text(String::new(), 12.0, theme::TEXT)),
                    (GuildOnlineCountText chrome_text(String::new(), 11.5, theme::EMERALD_BRI)),
                    (GuildSkillPointsText chrome_text(String::new(), 11.0, theme::GOLD)),
                    (
                        GuildEmblemUploadButton
                        GuildMutationControl
                        @FeathersButton {
                            @caption: bsn! { (Text("Change Emblem") ThemedText) },
                            @variant: ButtonVariant::Normal,
                        }
                        Node { width: px(138), height: px(30) }
                        Visibility::Hidden
                        on(super::emblem::on_select_emblem)
                    ),
                ]
            ),
        ]
    }
}

fn tabs() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, column_gap: px(6) }
        ignore_picking()
        Children [
            (
                MembersTabButton
                GuildTabButton
                @FeathersButton { @caption: bsn! { (Text("Members") ThemedText) } }
                Node { flex_grow: 1.0, height: px(34) }
                on(super::select_members)
            ),
            (
                PositionsTabButton
                GuildTabButton
                @FeathersButton { @caption: bsn! { (Text("Positions") ThemedText) } }
                Node { flex_grow: 1.0, height: px(34) }
                on(super::select_positions)
            ),
            (
                NoticeTabButton
                GuildTabButton
                @FeathersButton { @caption: bsn! { (Text("Notice") ThemedText) } }
                Node { flex_grow: 1.0, height: px(34) }
                on(super::select_notice)
            ),
            (
                SkillsTabButton
                GuildTabButton
                @FeathersButton { @caption: bsn! { (Text("Skills") ThemedText) } }
                Node { flex_grow: 1.0, height: px(34) }
                on(super::select_skills)
            ),
            (
                RelationsTabButton
                GuildTabButton
                @FeathersButton { @caption: bsn! { (Text("Relations") ThemedText) } }
                Node { flex_grow: 1.0, height: px(34) }
                on(super::select_relations)
            ),
        ]
    }
}

fn content() -> impl Scene {
    bsn! {
        Node { width: percent(100), height: px(350), flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch }
        ignore_picking()
        Children [
            (
                GuildMembersPanel
                GuildTabPage
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    column_gap: px(4),
                }
                ignore_picking()
                Children [
                    (
                        #members_scroll
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            height: percent(100),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Column,
                            row_gap: px(5),
                            padding: {UiRect::right(px(6))},
                        }
                        ScrollArea
                        Pickable
                        Children [
                            invite_controls(),
                            roster_heading(),
                            (GuildMembersList Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(5) } ignore_picking()),
                        ]
                    ),
                    @FeathersScrollbar { @target: #members_scroll, @orientation: {ControlOrientation::Vertical} }
                    Node { width: px(6), height: percent(100) }
                ]
            ),
            (
                GuildPositionsPanel
                GuildTabPage
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::None,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    column_gap: px(4),
                    padding: {UiRect::vertical(px(10))},
                }
                Visibility::Hidden
                ignore_picking()
                Children [
                    (
                        #positions_scroll
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0),
                            height: percent(100),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Column,
                            padding: {UiRect::right(px(6))},
                        }
                        ScrollArea
                        Pickable
                        Children [ (GuildPositionsList Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8) } ignore_picking()) ]
                    ),
                    @FeathersScrollbar { @target: #positions_scroll, @orientation: {ControlOrientation::Vertical} }
                    Node { width: px(6), height: percent(100) }
                ]
            ),
            (
                GuildNoticePanel
                GuildTabPage
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    padding: {UiRect::vertical(px(10))},
                }
                Visibility::Hidden
                ignore_picking()
                Children [ (GuildNoticeContent Node { width: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Stretch, row_gap: px(10) } ignore_picking()) ]
            ),
            (
                GuildSkillsPanel
                GuildTabPage
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::None,
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Column,
                    padding: {UiRect::vertical(px(10))},
                }
                Visibility::Hidden
                Pickable
                Children [ (GuildSkillsList Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8) } ignore_picking()) ]
            ),
            (
                GuildRelationsPanel
                GuildTabPage
                Node {
                    width: percent(100),
                    height: percent(100),
                    display: Display::None,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10),
                    padding: {UiRect::vertical(px(10))},
                }
                Visibility::Hidden
                Pickable
                Children [
                    super::relations::relation_controls(),
                    (
                        super::relations::GuildRelationsList
                        Node {
                            width: percent(100),
                            flex_grow: 1.0,
                            min_height: px(0),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Column,
                        }
                        Pickable
                    ),
                ]
            ),
        ]
    }
}

fn invite_controls() -> impl Scene {
    let editable = EditableText {
        max_characters: Some(24),
        ..default()
    };
    bsn! {
        GuildInviteControls
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::bottom(px(7))},
        }
        Pickable
        Children [
            (
                GuildInviteNameField
                Pickable
                template_value(editable)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(13.0)},
                }
                TextColor(theme::TEXT)
                BackgroundColor(theme::FIELD)
                Node {
                    flex_grow: 1.0,
                    height: px(36),
                    padding: {UiRect::axes(px(10), px(8))},
                    border: px(1),
                    border_radius: BorderRadius::all(px(8)),
                }
                BorderColor::all(theme::STROKE)
            ),
            (
                GuildInviteButton
                @FeathersButton {
                    @caption: bsn! { (Text("Invite by Name") ThemedText) },
                    @variant: ButtonVariant::Primary,
                }
                Node { width: px(150), height: px(36) }
                on(super::members::on_invite_by_name)
            ),
        ]
    }
}

fn roster_heading() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Row, padding: {UiRect::horizontal(px(10))} }
        ignore_picking()
        Children [
            (Node { flex_grow: 1.0 } chrome_text("Member / Status".to_string(), 9.5, theme::TEXT_FAINT)),
            (Node { width: px(210) } chrome_text("Resources".to_string(), 9.5, theme::TEXT_FAINT)),
        ]
    }
}

pub(crate) fn member_rows(rows: Vec<MemberRow>) -> impl Scene {
    let rows: Vec<_> = rows.into_iter().map(member_row).collect();
    bsn! { Node { flex_direction: FlexDirection::Column, row_gap: px(5) } ignore_picking() Children [ {rows} ] }
}

fn member_row(row: MemberRow) -> impl Scene {
    let status = if row.online { "Online" } else { "Offline" };
    let status_color = if row.online {
        theme::EMERALD_BRI
    } else {
        theme::TEXT_FAINT
    };
    let ap = row
        .ap
        .map(|(current, max)| format!("  AP {current}/{max}"))
        .unwrap_or_default();
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(10), px(7))},
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            (
                Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(2) }
                ignore_picking()
                Children [
                    chrome_text(row.name, 13.0, theme::TEXT),
                    chrome_text(format!("{} · {} · Lv {}", row.position, row.job, row.level), 10.5, theme::TEXT_DIM),
                    chrome_text(format!("{status} · {}", row.map), 10.0, status_color),
                ]
            ),
            (
                Node { width: px(210) }
                chrome_text(
                    format!("HP {}/{}  SP {}/{}{}", row.hp.0, row.hp.1, row.sp.0, row.sp.1, ap),
                    10.5,
                    theme::TEXT_DIM,
                )
            ),
            (
                template_value(super::members::GuildExpelControl(row.char_id))
                Node { width: px(190), flex_direction: FlexDirection::Row, column_gap: px(5) }
                Pickable
                Children [
                    (
                        template_value(super::members::GuildExpelReasonField(row.char_id))
                        Pickable
                        template_value(EditableText::new(""))
                        TextFont {
                            font: FontSourceTemplate::Handle(theme::FONT_BODY),
                            font_size: {FontSize::Px(11.0)},
                        }
                        TextColor(theme::TEXT)
                        BackgroundColor(theme::FIELD)
                        Node { flex_grow: 1.0, height: px(30), padding: {UiRect::horizontal(px(6))} }
                        BorderColor::all(theme::STROKE)
                    ),
                    (
                        template_value(super::members::GuildExpelButton(row.char_id))
                        GuildMutationControl
                        @FeathersButton {
                            @caption: bsn! { (Text("Expel") ThemedText) },
                            @variant: ButtonVariant::Normal,
                        }
                        Node { width: px(68), height: px(30) }
                        on(super::members::on_expel)
                    ),
                ]
            ),
        ]
    }
}

fn feedback_banner() -> impl Scene {
    bsn! {
        GuildFeedbackBanner
        Node {
            width: percent(100),
            min_height: px(34),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: {UiRect::axes(px(12), px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
        }
        BackgroundColor(Color::NONE)
        BorderColor::all(Color::NONE)
        Visibility::Hidden
        ignore_picking()
        Children [ (GuildFeedbackText chrome_text(String::new(), 11.5, theme::TEXT)) ]
    }
}

fn title_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        ignore_picking()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    #[test]
    fn window_has_supported_tabs_without_creation_controls() {
        let mut app = app();
        app.world_mut().spawn_scene(window()).unwrap();
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();

        for expected in ["Members", "Positions", "Notice", "Skills", "Relations"] {
            assert!(texts.contains(&expected.to_string()), "missing {expected}");
        }
        for omitted in [
            "Create a Guild",
            "Create",
            "Requires 1 Emperium",
            "Territories",
            "Expelled",
            "Guild Funds",
            "Contrib",
            "Edit",
            "Expel",
            "Assign",
        ] {
            assert!(
                !texts.contains(&omitted.to_string()),
                "rendered unsupported {omitted}"
            );
        }
    }

    #[test]
    fn tab_buttons_and_pages_match_with_only_one_page_in_layout() {
        let mut app = app();
        app.world_mut().spawn_scene(window()).unwrap();

        let tab_count = app
            .world_mut()
            .query_filtered::<Entity, With<GuildTabButton>>()
            .iter(app.world())
            .count();
        let pages: Vec<_> = app
            .world_mut()
            .query_filtered::<&Node, With<GuildTabPage>>()
            .iter(app.world())
            .map(|node| node.display)
            .collect();

        assert_eq!(pages.len(), tab_count);
        assert_eq!(
            pages
                .iter()
                .filter(|display| **display == Display::Flex)
                .count(),
            1
        );
        assert!(
            pages
                .iter()
                .filter(|display| **display != Display::Flex)
                .all(|display| *display == Display::None)
        );
    }

    #[test]
    fn guilded_view_contains_shared_progression_header_structure() {
        let mut app = app();
        app.world_mut().spawn_scene(window()).unwrap();

        for count in [
            app.world_mut()
                .query_filtered::<Entity, With<GuildLevelText>>()
                .iter(app.world())
                .count(),
            app.world_mut()
                .query_filtered::<Entity, With<GuildExpText>>()
                .iter(app.world())
                .count(),
            app.world_mut()
                .query_filtered::<Entity, With<GuildSkillPointsText>>()
                .iter(app.world())
                .count(),
            app.world_mut()
                .query_filtered::<Entity, With<GuildExpFill>>()
                .iter(app.world())
                .count(),
        ] {
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn management_window_keeps_invite_field_click_focusable() {
        let mut app = app();
        app.add_plugins(crate::focus::UiFocusMirrorPlugin);
        app.world_mut().spawn_scene(window()).unwrap();

        let root = app
            .world_mut()
            .query_filtered::<&Node, With<GuildWindowRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(root.width, px(GUILD_WINDOW_WIDTH));
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<GuildInviteNameField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Pickable>(entity),
            Some(&Pickable::default())
        );
        assert_eq!(
            app.world()
                .get::<bevy::input_focus::tab_navigation::TabIndex>(entity),
            Some(&bevy::input_focus::tab_navigation::TabIndex(0))
        );
    }

    #[test]
    fn clicking_invite_name_field_accepts_keyboard_input() {
        use bevy::camera::RenderTarget;
        use bevy::input::ButtonState;
        use bevy::input::keyboard::{Key, KeyboardInput};
        use bevy::input_focus::tab_navigation::TabNavigationPlugin;
        use bevy::input_focus::{InputDispatchPlugin, InputFocusPlugin};
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerButton, PointerId};
        use bevy::window::{PrimaryWindow, WindowRef};

        let mut app = app();
        app.add_plugins((
            bevy::input::InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
            TabNavigationPlugin,
            bevy::text::TextPlugin,
            bevy::ui_widgets::EditableTextInputPlugin,
            crate::focus::UiFocusMirrorPlugin,
        ));
        app.init_resource::<game_engine::domain::input::UiFocus>();
        app.init_resource::<bevy::ui::UiScale>();
        app.add_message::<bevy::window::Ime>();
        app.add_message::<Pointer<Release>>();
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.world_mut().spawn_scene(super::window()).unwrap();
        let name_box = app
            .world_mut()
            .query_filtered::<Entity, With<GuildInviteNameField>>()
            .single(app.world())
            .unwrap();

        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            Location {
                target: RenderTarget::Window(WindowRef::Entity(window))
                    .normalize(Some(window))
                    .unwrap(),
                position: Vec2::new(4.0, 4.0),
            },
            Press {
                button: PointerButton::Primary,
                hit: HitData::new(window, 0.0, None, None),
                count: 1,
            },
            name_box,
        ));
        app.world_mut().flush();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyV,
            logical_key: Key::Character("V".into()),
            state: ButtonState::Pressed,
            text: Some("V".into()),
            repeat: false,
            window,
        });
        app.update();

        let field = app
            .world_mut()
            .query_filtered::<&EditableText, With<GuildInviteNameField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(field.value(), "V");
    }

    #[test]
    fn relation_name_fields_are_static_click_focusable_controls() {
        let mut app = app();
        app.add_plugins(crate::focus::UiFocusMirrorPlugin);
        app.world_mut().spawn_scene(window()).unwrap();

        for entity in [
            app.world_mut()
                .query_filtered::<Entity, With<super::relations::GuildAllianceNameField>>()
                .single(app.world())
                .unwrap(),
            app.world_mut()
                .query_filtered::<Entity, With<super::relations::GuildAntagonistNameField>>()
                .single(app.world())
                .unwrap(),
        ] {
            assert_eq!(
                app.world().get::<Pickable>(entity),
                Some(&Pickable::default())
            );
            assert_eq!(
                app.world()
                    .get::<bevy::input_focus::tab_navigation::TabIndex>(entity),
                Some(&bevy::input_focus::tab_navigation::TabIndex(0))
            );
        }
    }

    #[test]
    fn roster_row_renders_online_map_position_job_level_and_resources() {
        let mut app = app();
        app.world_mut()
            .spawn_scene(member_rows(vec![MemberRow {
                char_id: 42,
                name: "Odin".into(),
                position: "Master".into(),
                job: "Rune Knight".into(),
                level: 99,
                online: true,
                map: "prontera".into(),
                hp: (90, 100),
                sp: (40, 50),
                ap: Some((8, 10)),
            }]))
            .unwrap();
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        assert!(texts.contains(&"Master · Rune Knight · Lv 99".to_string()));
        assert!(texts.contains(&"Online · prontera".to_string()));
        assert!(texts.contains(&"HP 90/100  SP 40/50  AP 8/10".to_string()));
        let reason_field = app
            .world_mut()
            .query_filtered::<Entity, With<super::members::GuildExpelReasonField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world().get::<Pickable>(reason_field),
            Some(&Pickable::default())
        );
    }

    #[test]
    fn offline_row_keeps_last_known_hp_sp_and_ap() {
        let mut app = app();
        app.world_mut()
            .spawn_scene(member_rows(vec![MemberRow {
                char_id: 43,
                name: "Thor".into(),
                position: "Member".into(),
                job: "Blacksmith".into(),
                level: 80,
                online: false,
                map: "geffen".into(),
                hp: (70, 100),
                sp: (30, 50),
                ap: Some((5, 10)),
            }]))
            .unwrap();
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();

        assert!(texts.contains(&"Offline · geffen".to_string()));
        assert!(texts.contains(&"HP 70/100  SP 30/50  AP 5/10".to_string()));
    }

    #[test]
    fn task7_shell_is_read_only_for_non_master_members() {
        let mut app = app();
        app.world_mut().spawn_scene(window()).unwrap();
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect();

        for management_control in ["Edit", "Expel", "Assign", "Upload Emblem"] {
            assert!(!texts.contains(&management_control));
        }
        assert!(texts.contains(&"Leave Guild"));
    }
}
