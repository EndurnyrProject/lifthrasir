//! System dialog: a reusable, message-driven modal notice (raw `bevy_ui`).
//!
//! Ported from the `designs/Endurnir Project` System Dialog mockup. Any system can
//! summon it by writing a [`ShowSystemDialog`] message; the widget spawns a dimmed,
//! click-eating backdrop centering a glass card (severity badge, kicker, title,
//! message, optional code chip, primary button). The primary button — or Enter —
//! dismisses the card and, when the request carried a `confirm_state`, navigates
//! there via `NextState<GameState>`.
//!
//! Only one dialog lives at a time: a second request while one is open is ignored,
//! which absorbs the duplicate lost/failed events quinnet can emit on a drop.
//!
//! The severity badge renders the mockup's per-preset line icon as an SVG glyph
//! (via [`theme::icon`]), tinted with the severity accent.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::presentation::ui::events::{DialogSeverity, ShowSystemDialog};

use crate::theme;

/// Sits just below the fade transition so the modal renders over every screen.
const DIALOG_Z: i32 = i32::MAX - 2;
const DIALOG_WIDTH: f32 = 384.0;

pub struct SystemDialogPlugin;

impl Plugin for SystemDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (show_system_dialog, confirm_on_enter));
    }
}

/// The modal root. Carries the screen the primary button navigates to.
#[derive(Component)]
pub struct SystemDialogRoot {
    confirm_state: Option<GameState>,
}

/// Accent colour for a severity — drives the badge border, glyph, and primary button.
pub fn severity_accent(severity: DialogSeverity) -> Color {
    match severity {
        DialogSeverity::Error => theme::BAD,
        DialogSeverity::Warn => theme::WARN,
        DialogSeverity::Info => theme::MANA_BLUE,
        DialogSeverity::Ok => theme::EMERALD,
    }
}

/// The line-icon name (in `assets/ui/icons/`) for a severity.
fn severity_icon(severity: DialogSeverity) -> &'static str {
    match severity {
        DialogSeverity::Error => "bang",
        DialogSeverity::Warn => "triangle",
        DialogSeverity::Info => "info",
        DialogSeverity::Ok => "ok",
    }
}

/// Spawns the modal for the latest request, unless one is already open.
fn show_system_dialog(
    mut requests: MessageReader<ShowSystemDialog>,
    existing: Query<(), With<SystemDialogRoot>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    spawn_dialog(&mut commands, &asset_server, request);
}

fn spawn_dialog(commands: &mut Commands, asset_server: &AssetServer, request: &ShowSystemDialog) {
    let body = asset_server.load(theme::FONT_BODY);
    let title_font = asset_server.load(theme::FONT_TITLE);
    let accent = severity_accent(request.severity);

    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.012, 0.027, 0.024, 0.55)),
            GlobalZIndex(DIALOG_Z),
            Pickable::default(),
            SystemDialogRoot {
                confirm_state: request.confirm_state.clone(),
            },
        ))
        .id();

    let card = commands
        .spawn((
            Node {
                width: Val::Px(DIALOG_WIDTH),
                padding: UiRect::axes(Val::Px(30.0), Val::Px(26.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::GLASS),
            BorderColor::all(theme::STROKE),
            ChildOf(backdrop),
        ))
        .id();

    commands
        .spawn((
            Node {
                width: Val::Px(60.0),
                height: Val::Px(60.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(theme::FIELD),
            BorderColor::all(accent.with_alpha(0.4)),
            ChildOf(card),
        ))
        .with_child(theme::icon(
            asset_server,
            severity_icon(request.severity),
            30.0,
            accent,
        ));

    commands.spawn((
        theme::label(
            request.kicker.to_uppercase(),
            body.clone(),
            10.0,
            theme::GOLD,
        ),
        Node {
            margin: UiRect::top(Val::Px(16.0)),
            ..default()
        },
        ChildOf(card),
    ));

    commands.spawn((
        theme::label(request.title.clone(), title_font, 23.0, theme::DISPLAY_GOLD),
        Node {
            margin: UiRect::top(Val::Px(7.0)),
            ..default()
        },
        ChildOf(card),
    ));

    commands.spawn((
        theme::label(request.message.clone(), body.clone(), 13.5, theme::TEXT_DIM),
        Node {
            max_width: Val::Px(DIALOG_WIDTH - 80.0),
            margin: UiRect::top(Val::Px(11.0)),
            ..default()
        },
        ChildOf(card),
    ));

    if !request.code.is_empty() {
        commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                    margin: UiRect::top(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.30)),
                BorderColor::all(theme::STROKE),
                ChildOf(card),
            ))
            .with_child(theme::label(
                format!("CODE  {}", request.code),
                body.clone(),
                11.0,
                accent,
            ));
    }

    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(theme::STROKE_STRONG),
        Pickable::IGNORE,
        ChildOf(card),
    ));

    let button = commands
        .spawn((
            Pickable::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                border_radius: BorderRadius::all(Val::Px(11.0)),
                ..default()
            },
            BackgroundColor(accent),
            ChildOf(card),
        ))
        .id();
    commands.spawn((
        theme::label(
            request.button_label.clone(),
            body.clone(),
            14.5,
            Color::srgba(0.0, 0.0, 0.0, 0.85),
        ),
        ChildOf(button),
    ));
    commands.spawn((
        theme::label("Enter", body, 9.5, Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ChildOf(button),
    ));
    commands.entity(button).observe(confirm_on_click);
}

/// Despawns the open dialog and, if it carried a target, navigates there.
fn confirm_dialog(
    root: Entity,
    confirm_state: &Option<GameState>,
    commands: &mut Commands,
    next_state: &mut NextState<GameState>,
) {
    commands.entity(root).despawn();
    if let Some(state) = confirm_state {
        next_state.set(state.clone());
    }
}

fn confirm_on_click(
    _click: On<Pointer<Click>>,
    dialog: Single<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let (root, dialog) = dialog.into_inner();
    confirm_dialog(root, &dialog.confirm_state, &mut commands, &mut next_state);
}

fn confirm_on_enter(
    mut keys: MessageReader<KeyboardInput>,
    dialog: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok((root, dialog)) = dialog.single() else {
        keys.clear();
        return;
    };
    let confirmed = keys
        .read()
        .any(|event| event.state.is_pressed() && event.logical_key == Key::Enter);
    if confirmed {
        confirm_dialog(root, &dialog.confirm_state, &mut commands, &mut next_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_accent_maps_each_variant() {
        assert_eq!(severity_accent(DialogSeverity::Error), theme::BAD);
        assert_eq!(severity_accent(DialogSeverity::Warn), theme::WARN);
        assert_eq!(severity_accent(DialogSeverity::Info), theme::MANA_BLUE);
        assert_eq!(severity_accent(DialogSeverity::Ok), theme::EMERALD);
    }

    #[test]
    fn severity_icon_distinguishes_tone() {
        assert_eq!(severity_icon(DialogSeverity::Error), "bang");
        assert_eq!(severity_icon(DialogSeverity::Warn), "triangle");
        assert_eq!(severity_icon(DialogSeverity::Info), "info");
        assert_eq!(severity_icon(DialogSeverity::Ok), "ok");
    }
}
