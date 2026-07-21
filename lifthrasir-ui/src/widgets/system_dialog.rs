//! System dialog: a reusable, message-driven modal notice (BSN + Feathers).
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
//! The card chrome is one declarative `bsn!` tree; the primary button is a Feathers
//! `@FeathersButton` (Primary variant) whose `on(Activate)` observer runs the shared
//! dismiss + navigate logic. The severity accent tints the badge border/glyph and the
//! code chip; Feathers owns the button fill from its own tokens, so the severity
//! colour is carried by the badge rather than the button (see the note on `card`).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};

use crate::theme;
use crate::theme::feathers_theme::install_norse_theme;
use crate::widgets::chrome::ignore_picking;

/// Sits just below the fade transition so the modal renders over every screen.
/// `pub` so sibling modals (e.g. the death dialog) can anchor strictly below it.
pub const DIALOG_Z: i32 = i32::MAX - 2;
const DIALOG_WIDTH: f32 = 384.0;

pub struct SystemDialogPlugin;

impl Plugin for SystemDialogPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.add_systems(Update, (show_system_dialog, confirm_on_enter));
    }
}

/// The modal root. Carries navigation and ownership data echoed onto every choice.
#[derive(Component, Clone, Default)]
pub struct SystemDialogRoot {
    confirm_state: Option<GameState>,
    kind: SystemDialogKind,
    correlation: Option<u64>,
}

impl SystemDialogRoot {
    pub(crate) fn new(
        confirm_state: Option<GameState>,
        kind: SystemDialogKind,
        correlation: Option<u64>,
    ) -> Self {
        Self {
            confirm_state,
            kind,
            correlation,
        }
    }

    /// Whether this root belongs to one exact dialog-producing operation.
    pub fn matches(&self, kind: SystemDialogKind, correlation: Option<u64>) -> bool {
        self.kind == kind && self.correlation == correlation
    }
}

/// Accent colour for a severity — drives the badge border, glyph, and code chip.
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
pub(crate) fn show_system_dialog(
    mut requests: MessageReader<ShowSystemDialog>,
    existing: Query<(), With<SystemDialogRoot>>,
    mut commands: Commands,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    if !existing.is_empty() {
        return;
    }
    commands.spawn_scene(system_dialog(request));
}

/// The whole modal as one scene: a dimmed, click-eating backdrop centering the glass card.
fn system_dialog(request: &ShowSystemDialog) -> impl Scene + use<> {
    let confirm_state = request.confirm_state.clone();
    let kind = request.kind;
    let correlation = request.correlation;
    bsn! {
        template_value(SystemDialogRoot::new(confirm_state, kind, correlation))
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        BackgroundColor({Color::srgba(0.012, 0.027, 0.024, 0.55)})
        GlobalZIndex({DIALOG_Z})
        Pickable
        Children [ card(request) ]
    }
}

/// The glass card. The severity accent tints the badge border/glyph and the code chip;
/// the primary button is a Feathers Primary `@FeathersButton`, which drives its own fill
/// from `BUTTON_PRIMARY_*` tokens — a per-severity button tint is not expressible through
/// Feathers, so the severity cue lives on the badge, not the button.
fn card(request: &ShowSystemDialog) -> impl Scene + use<> {
    let accent = severity_accent(request.severity);
    let code_display = if request.code.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    let code_text = format!("CODE  {}", request.code);
    bsn! {
        Node {
            width: px(DIALOG_WIDTH),
            padding: {UiRect::axes(px(30), px(26))},
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(16)),
        }
        BackgroundColor({theme::GLASS})
        BorderColor::all(theme::STROKE)
        Children [
            badge(severity_icon(request.severity), accent),
            card_text(request.kicker.to_uppercase(), theme::FONT_BODY, 10.0, theme::GOLD, 16.0),
            card_text(request.title.clone(), theme::FONT_TITLE, 23.0, theme::DISPLAY_GOLD, 7.0),
            message(request.message.clone()),
            code_chip(code_text, accent, code_display),
            divider(),
            button_row(request),
        ]
    }
}

/// Decide whether the secondary button participates in layout: collapsed to
/// `Display::None` (removed from flow, so the primary keeps full width) when the
/// request carried no secondary label.
fn secondary_display(label: &str) -> Display {
    if label.is_empty() {
        Display::None
    } else {
        Display::Flex
    }
}

/// The action row: an optional lesser secondary button beside the primary button.
/// Both grow to share the row; when the secondary is collapsed the primary spans
/// the full width, preserving the single-button look.
fn button_row(request: &ShowSystemDialog) -> impl Scene + use<> {
    let secondary = secondary_button(
        request.secondary_label.clone(),
        secondary_display(&request.secondary_label),
    );
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: px(10),
        }
        ignore_picking()
        Children [
            secondary,
            primary_button(request.button_label.clone()),
        ]
    }
}

/// Round-cornered severity badge holding the tinted line glyph.
fn badge(icon_name: &'static str, accent: Color) -> impl Scene {
    bsn! {
        Node {
            width: px(60),
            height: px(60),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(16)),
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(accent.with_alpha(0.4))
        Children [ severity_glyph(icon_name, accent) ]
    }
}

/// A square white SVG glyph tinted with the severity accent. `ImageNode` has no theme
/// token, so its colour stays a raw palette value.
fn severity_glyph(name: &'static str, color: Color) -> impl Scene {
    bsn! {
        ImageNode {
            image: {format!("{}{}.svg", theme::ICON_DIR, name)},
            color: color,
        }
        Node { width: px(30), height: px(30) }
        ignore_picking()
    }
}

/// Standalone card line: Feathers has no font token and this text sits outside a Feathers
/// ancestor, so font and colour are set explicitly. `margin_top` spaces it from the line above.
fn card_text(
    text: String,
    font: &'static str,
    size: f32,
    color: Color,
    margin_top: f32,
) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(font),
            font_size: {FontSize::Px(size)},
        }
        TextColor(color)
        Node { margin: {UiRect::top(px(margin_top))} }
        ignore_picking()
    }
}

/// The wrapped body message; capped width so long copy stays inside the card.
fn message(text: String) -> impl Scene {
    bsn! {
        Text(text)
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_BODY),
            font_size: {FontSize::Px(13.5)},
        }
        TextColor({theme::TEXT_DIM})
        Node {
            max_width: px(DIALOG_WIDTH - 80.0),
            margin: {UiRect::top(px(11))},
        }
        ignore_picking()
    }
}

/// Optional error-code chip. Rendered always but collapsed to `Display::None` (removing it
/// and its top margin from layout) when the request carried no code.
fn code_chip(text: String, accent: Color, display: Display) -> impl Scene {
    bsn! {
        Node {
            display: {display},
            padding: {UiRect::axes(px(12), px(5))},
            margin: {UiRect::top(px(16))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.30)})
        BorderColor::all(theme::STROKE)
        Children [
            (
                Text(text)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(11.0)},
                }
                TextColor(accent)
                ignore_picking()
            ),
        ]
    }
}

/// Hairline separator between the copy and the primary button.
fn divider() -> impl Scene {
    bsn! {
        Node {
            width: percent(100),
            height: px(1),
            margin: {UiRect::vertical(px(20))},
        }
        BackgroundColor({theme::STROKE_STRONG})
        ignore_picking()
    }
}

/// The primary action: a Feathers Primary button captioned with the request label and an
/// "Enter" hint, wired to the shared confirm handler via `on(Activate)`.
fn primary_button(label: String) -> impl Scene {
    bsn! {
        @FeathersButton {
            @caption: bsn! {
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(9),
                }
                ignore_picking()
                Children [
                    (Text(label) ThemedText),
                    (
                        Text("Enter")
                        TextFont {
                            font: FontSourceTemplate::Handle(theme::FONT_BODY),
                            font_size: {FontSize::Px(9.5)},
                        }
                        TextColor({Color::WHITE.with_alpha(0.55)})
                        ignore_picking()
                    ),
                ]
            },
            @variant: ButtonVariant::Primary,
        }
        Node {
            flex_grow: 1.0,
            height: px(46),
            border_radius: BorderRadius::all(px(11)),
        }
        on(confirm_on_click)
    }
}

/// The lesser secondary action: a Normal-variant Feathers button. Rendered but
/// collapsed to `Display::None` when the request carried no secondary label. Its
/// `on(Activate)` dismisses the dialog and reports a non-primary choice — it never
/// navigates `confirm_state`.
fn secondary_button(label: String, display: Display) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { (Text(label) ThemedText) } }
        Node {
            display: {display},
            flex_grow: 1.0,
            height: px(46),
            border_radius: BorderRadius::all(px(11)),
        }
        on(dismiss_on_click)
    }
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
    _activate: On<Activate>,
    dialog: Single<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut choice: MessageWriter<SystemDialogChoice>,
) {
    let (root, dialog) = dialog.into_inner();
    choice.write(SystemDialogChoice {
        primary: true,
        kind: dialog.kind,
        correlation: dialog.correlation,
    });
    confirm_dialog(root, &dialog.confirm_state, &mut commands, &mut next_state);
}

fn confirm_on_enter(
    mut keys: MessageReader<KeyboardInput>,
    dialog: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut choice: MessageWriter<SystemDialogChoice>,
) {
    let Ok((root, dialog)) = dialog.single() else {
        keys.clear();
        return;
    };
    let confirmed = keys
        .read()
        .any(|event| event.state.is_pressed() && event.logical_key == Key::Enter);
    if confirmed {
        choice.write(SystemDialogChoice {
            primary: true,
            kind: dialog.kind,
            correlation: dialog.correlation,
        });
        confirm_dialog(root, &dialog.confirm_state, &mut commands, &mut next_state);
    }
}

/// Secondary button: report a non-primary choice and dismiss the dialog. Unlike
/// the primary path it never navigates `confirm_state` — a decline just closes.
fn dismiss_on_click(
    _activate: On<Activate>,
    dialog: Query<(Entity, &SystemDialogRoot)>,
    mut commands: Commands,
    mut choice: MessageWriter<SystemDialogChoice>,
) {
    let Ok((root, dialog)) = dialog.single() else {
        return;
    };
    choice.write(SystemDialogChoice {
        primary: false,
        kind: dialog.kind,
        correlation: dialog.correlation,
    });
    commands.entity(root).despawn();
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

    fn choices(app: &App) -> Vec<(bool, SystemDialogKind, Option<u64>)> {
        let messages = app.world().resource::<Messages<SystemDialogChoice>>();
        let mut cursor = messages.get_cursor();
        cursor
            .read(messages)
            .map(|choice| (choice.primary, choice.kind, choice.correlation))
            .collect()
    }

    #[test]
    fn secondary_display_collapses_when_label_empty() {
        assert_eq!(secondary_display(""), Display::None);
        assert_eq!(secondary_display("Decline"), Display::Flex);
    }

    #[test]
    fn confirm_on_click_despawns_dialog_and_navigates() {
        let mut app = App::new();
        app.init_resource::<NextState<GameState>>();
        app.add_message::<SystemDialogChoice>();
        let target = GameState::CharacterSelection;
        let root = app
            .world_mut()
            .spawn(SystemDialogRoot {
                confirm_state: Some(target.clone()),
                kind: SystemDialogKind::PartyInvite,
                correlation: Some(42),
            })
            .observe(confirm_on_click)
            .id();
        app.world_mut().trigger(Activate { entity: root });
        app.world_mut().flush();

        assert!(
            app.world().get_entity(root).is_err(),
            "confirming despawns the dialog root"
        );
        match app.world().resource::<NextState<GameState>>() {
            NextState::Pending(state) => assert_eq!(*state, target),
            _ => panic!("confirm_state should have been queued as the next game state"),
        }
        assert_eq!(
            choices(&app),
            vec![(true, SystemDialogKind::PartyInvite, Some(42))],
            "primary press echoes the dialog kind and correlation"
        );
    }

    #[test]
    fn dismiss_on_click_despawns_and_reports_non_primary() {
        let mut app = App::new();
        app.add_message::<SystemDialogChoice>();
        let root = app.world_mut().spawn(SystemDialogRoot::default()).id();
        let button = app.world_mut().spawn_empty().observe(dismiss_on_click).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        assert!(
            app.world().get_entity(root).is_err(),
            "dismissing despawns the dialog root"
        );
        assert_eq!(
            choices(&app),
            vec![(false, SystemDialogKind::Generic, None)],
            "secondary press reports non-primary and the default dialog identity"
        );
    }
}
