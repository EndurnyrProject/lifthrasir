//! Idiomatic BSN chrome for the party roster window (mirrors the inventory window).
//!
//! [`window`] builds the persistent chrome — root, draggable titlebar, an empty
//! [`PartyWindowBody`] region, and a persistent footer with a real `@FeathersButton`
//! "Leave Party" — as one `bsn!` tree. [`body`] projects the live roster view-model
//! (header band + one row per member, or the partyless empty state) and is respawned
//! by [`refresh_roster`](super::refresh_roster) each frame the window is visible.
//!
//! Scenes own their data, so every view-model ([`RosterHeader`], [`RosterRow`]) is
//! prepared as owned values in the system before entering a `bsn!` block. The window
//! spawns as a single scene, so the titlebar drag is an inline `on(...)` observer that
//! resolves the root by marker (no captured entity, no `make_draggable`).

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::FeathersButton;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor, ThemedText};
use net_contract::commands::PartyLeaveRequested;

use crate::theme;
use crate::theme::feathers_theme::{
    TOKEN_TEXT, TOKEN_TITLEBAR_BG, TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER,
};
use crate::widgets::draggable::px_or_zero;

use super::{
    MemberPresence, PartyFooter, PartyTitlebar, PartyWindowBody, PartyWindowRoot, PARTY_MAX,
};

const WINDOW_LEFT: f32 = 300.0;
const WINDOW_TOP: f32 = 90.0;
const WINDOW_WIDTH: f32 = 320.0;

/// The owned header view-model: party name, leader name, and the member/active counts.
pub(crate) struct RosterHeader {
    pub name: String,
    pub leader_name: String,
    pub members: usize,
    pub active: usize,
}

/// One roster row's owned view-model. `presence` already encodes the HP join result,
/// so the scene never touches `EntityRegistry`/`CharacterStatus`.
pub(crate) struct RosterRow {
    pub name: String,
    pub level: u32,
    pub map: String,
    pub online: bool,
    pub leader: bool,
    pub presence: MemberPresence,
}

/// Spawn the whole window as one scene and parent it under `parent` with a single
/// insert.
pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

fn window() -> impl Scene {
    bsn! {
        PartyWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [ titlebar(), body_container(), footer() ]
    }
}

fn titlebar() -> impl Scene {
    bsn! {
        PartyTitlebar
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        on(on_titlebar_drag)
        Children [
            glyph_icon("members", 16.0, theme::GOLD),
            (
                Text("Party")
                TextFont {
                    font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
                    font_size: {FontSize::Px(15.0)},
                }
                ThemeTextColor({TOKEN_TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_DIM) } }
                Node { width: px(22), height: px(22) }
                on(on_close)
            ),
        ]
    }
}

/// The (initially empty) body region; [`body`] fills it via `refresh_roster`.
fn body_container() -> impl Scene {
    bsn! {
        PartyWindowBody
        Node {
            flex_direction: FlexDirection::Column,
            padding: {UiRect { left: Val::Px(14.0), right: Val::Px(14.0), top: Val::Px(12.0), bottom: Val::Px(10.0) }},
        }
        ignore_picking()
    }
}

/// The persistent footer: a single `@FeathersButton` whose `on(Activate)` requests a
/// party leave. Hidden while partyless (toggled by `refresh_roster`) so it is never a
/// dead control; kept out of the swappable body so its observer is not respawned.
fn footer() -> impl Scene {
    bsn! {
        PartyFooter
        Node {
            flex_direction: FlexDirection::Row,
            padding: {UiRect { left: Val::Px(14.0), right: Val::Px(14.0), bottom: Val::Px(14.0), ..default() }},
        }
        Visibility::Hidden
        ignore_picking()
        Children [
            (
                @FeathersButton { @caption: bsn! { button_label("Leave Party") } }
                Node { flex_grow: 1.0, height: px(32) }
                on(on_leave)
            ),
        ]
    }
}

/// The whole swappable body: the header band plus one row per member, or the partyless
/// empty state. `header` is `None` exactly when partyless.
pub(crate) fn body(header: Option<RosterHeader>, rows: Vec<RosterRow>) -> impl Scene {
    let partyless = header.is_none();
    let header_scene = header.map(|header| EntityScene(header_band(header)));
    let empty = partyless.then(|| EntityScene(empty_state()));
    let row_scenes: Vec<_> = rows.into_iter().map(member_row).collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [ {header_scene}, {empty}, {row_scenes} ]
    }
}

/// Partyless empty state: a hint plus a "Create a party" `@FeathersButton` that opens
/// the create-party modal ([`create_dialog`](super::create_dialog)).
fn empty_state() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(8) }
        ignore_picking()
        Children [
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(2) }
                ignore_picking()
                Children [
                    title_text("Create a party".to_string(), 15.0, theme::GOLD),
                    chrome_text("You are not in a party yet.".to_string(), 11.5, theme::TEXT_DIM),
                ]
            ),
            (
                @FeathersButton { @caption: bsn! { button_label("Create a party") } }
                Node { height: px(32) }
                on(super::create_dialog::open_create_dialog)
            ),
        ]
    }
}

/// Header band: party name, leader line (crown + name), and the `members/12` + active
/// counts.
fn header_band(header: RosterHeader) -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: {UiRect::axes(px(10), px(8))},
            border_radius: BorderRadius::all(px(8)),
            margin: {UiRect::bottom(px(4))},
        }
        BackgroundColor(theme::GLASS_2)
        ignore_picking()
        Children [
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(3) }
                ignore_picking()
                Children [
                    title_text(header.name, 15.0, theme::TEXT),
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(4) }
                        ignore_picking()
                        Children [
                            glyph_icon("crown", 12.0, theme::GOLD),
                            chrome_text(format!("Leader {}", header.leader_name), 11.5, theme::TEXT_DIM),
                        ]
                    ),
                ]
            ),
            (
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(3) }
                ignore_picking()
                Children [
                    chrome_text(format!("{}/{PARTY_MAX} members", header.members), 11.5, theme::TEXT),
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(5) }
                        ignore_picking()
                        Children [
                            online_dot(true),
                            chrome_text(format!("{} active", header.active), 11.5, theme::EMERALD_BRI),
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// One roster row: online dot, name (+ leader crown), `Lv`/map meta, and the presence
/// cell (HP bar + "on screen", or "Elsewhere").
fn member_row(row: RosterRow) -> impl Scene {
    let crown = row
        .leader
        .then(|| EntityScene(glyph_icon("crown", 12.0, theme::GOLD)));
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(8), px(7))},
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            online_dot(row.online),
            (
                Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: px(2) }
                ignore_picking()
                Children [
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(4) }
                        ignore_picking()
                        Children [ chrome_text(row.name, 13.0, theme::TEXT), {crown} ]
                    ),
                    (
                        Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(4) }
                        ignore_picking()
                        Children [
                            chrome_text(format!("Lv {}", row.level), 11.0, theme::GOLD),
                            glyph_icon("pin", 11.0, theme::TEXT_FAINT),
                            chrome_text(row.map, 11.0, theme::TEXT_DIM),
                        ]
                    ),
                ]
            ),
            presence_cell(row.presence),
        ]
    }
}

/// Right-hand presence cell. `OnScreen` shows the HP bar, `hp / max`, and an "on
/// screen" chip; `Elsewhere` shows a plain tag with no bar and no fabricated HP.
fn presence_cell(presence: MemberPresence) -> impl Scene {
    let (bar, hp_text, chip, elsewhere) = match presence {
        MemberPresence::OnScreen { hp, max_hp } => {
            let fraction = if max_hp == 0 {
                0.0
            } else {
                (hp as f32 / max_hp as f32).clamp(0.0, 1.0)
            };
            let color = if fraction < 0.25 {
                theme::HEALTH_RED
            } else {
                theme::EMERALD
            };
            (
                Some(EntityScene(hp_bar(fraction * 100.0, color))),
                Some(EntityScene(chrome_text(
                    format!("{hp} / {max_hp}"),
                    10.5,
                    theme::TEXT_DIM,
                ))),
                Some(EntityScene(chrome_text(
                    "on screen".to_string(),
                    9.5,
                    theme::EMERALD_BRI,
                ))),
                None,
            )
        }
        MemberPresence::Elsewhere => (
            None,
            None,
            None,
            Some(EntityScene(chrome_text(
                "Elsewhere".to_string(),
                11.0,
                theme::TEXT_FAINT,
            ))),
        ),
    };
    bsn! {
        Node {
            width: px(96),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(3),
        }
        ignore_picking()
        Children [ {bar}, {hp_text}, {chip}, {elsewhere} ]
    }
}

/// An HP track with a fill sized to `percent_full` (0..=100) and tinted `color`.
fn hp_bar(percent_full: f32, color: Color) -> impl Scene {
    bsn! {
        Node { width: percent(100), height: px(6), border_radius: BorderRadius::all(px(3)) }
        BackgroundColor(theme::FIELD)
        ignore_picking()
        Children [
            (
                Node { width: percent(percent_full), height: percent(100), border_radius: BorderRadius::all(px(3)) }
                BackgroundColor(color)
                ignore_picking()
            ),
        ]
    }
}

/// A small round status dot: emerald when online, faint when offline.
fn online_dot(online: bool) -> impl Scene {
    let color = if online {
        theme::EMERALD
    } else {
        theme::TEXT_FAINT
    };
    bsn! {
        Node { width: px(8), height: px(8), border_radius: BorderRadius::all(px(4)) }
        BackgroundColor(color)
        ignore_picking()
    }
}

/// Button caption: `ThemedText` inherits font + color from the Feathers button ancestor.
fn button_label(text: &'static str) -> impl Scene {
    bsn! {
        Text(text)
        ThemedText
    }
}

/// A display-font (cinzel) label, colored explicitly (Feathers has no font token and
/// this text sits outside a Feathers ancestor).
fn title_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/cinzel.ttf"),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        ignore_picking()
    }
}

/// A plain colored text label with the body font.
fn chrome_text(text: String, size: f32, color: Color) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle("fonts/manrope.ttf"),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        ignore_picking()
    }
}

/// A square white SVG glyph tinted with `color`. `ImageNode` has no theme-token tint,
/// so glyph colors stay raw palette values.
fn glyph_icon(name: &'static str, size: f32, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(size), height: px(size) }
        ignore_picking()
    }
}

/// `Pickable::IGNORE` as a scene, so non-interactive nodes don't swallow clicks.
fn ignore_picking() -> impl Scene {
    bsn! {
        Pickable { should_block_lower: false, is_hoverable: false }
    }
}

fn on_close(_: On<Activate>, mut window: Query<&mut Visibility, With<PartyWindowRoot>>) {
    if let Ok(mut visibility) = window.single_mut() {
        *visibility = Visibility::Hidden;
    }
}

fn on_leave(_: On<Activate>, mut writer: MessageWriter<PartyLeaveRequested>) {
    writer.write(PartyLeaveRequested);
}

/// Drag the single party window by its titlebar; mirrors `make_draggable` but resolves
/// the root from its marker instead of a captured entity, so the whole window spawns as
/// one scene. Only the titlebar itself moves the window: `Pointer<Drag>` bubbles up from
/// the close button, so a drag targeting it is ignored.
fn on_titlebar_drag(
    drag: On<Pointer<Drag>>,
    titlebars: Query<(), With<PartyTitlebar>>,
    mut roots: Query<&mut Node, With<PartyWindowRoot>>,
) {
    if titlebars.get(drag.entity).is_err() {
        return;
    }
    let Ok(mut node) = roots.single_mut() else {
        return;
    };
    node.left = Val::Px(px_or_zero(node.left) + drag.delta.x);
    node.top = Val::Px(px_or_zero(node.top) + drag.delta.y);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    #[test]
    fn window_scene_spawns_root_titlebar_body_and_footer() {
        let mut app = scene_app();
        app.world_mut().spawn_scene(window()).unwrap();

        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<(), With<PartyWindowRoot>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<(), With<PartyWindowBody>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<(), With<PartyFooter>>()
                .iter(world)
                .count(),
            1
        );
    }

    #[test]
    fn body_scene_spawns_a_row_per_member() {
        let mut app = scene_app();
        let header = RosterHeader {
            name: "Wolfpack".to_string(),
            leader_name: "Solveig".to_string(),
            members: 2,
            active: 2,
        };
        let rows = vec![
            RosterRow {
                name: "Solveig".to_string(),
                level: 99,
                map: "prontera".to_string(),
                online: true,
                leader: true,
                presence: MemberPresence::OnScreen {
                    hp: 100,
                    max_hp: 200,
                },
            },
            RosterRow {
                name: "Brynjar".to_string(),
                level: 88,
                map: "payon".to_string(),
                online: true,
                leader: false,
                presence: MemberPresence::Elsewhere,
            },
        ];
        assert!(app
            .world_mut()
            .spawn_scene(body(Some(header), rows))
            .is_ok());
    }
}
