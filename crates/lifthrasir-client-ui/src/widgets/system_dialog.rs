//! System dialog: a reusable, message-driven modal notice, after the `System Dialogs`
//! artboard.
//!
//! Any system summons it by writing a [`ShowSystemDialog`] message. The dialog dims
//! the screen and shows a window whose title bar carries the kicker in the severity
//! tone, over a severity badge, the title, the message, an optional code chip and one
//! or two buttons. The primary button, or Enter, dismisses it and, when the request
//! carried a `confirm_state`, navigates there.
//!
//! Only one dialog lives at a time: a second request while one is open is ignored,
//! which absorbs the duplicate lost/failed events quinnet can emit on a drop.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::presentation::ui::events::{
    DialogSeverity, ShowSystemDialog, SystemDialogChoice, SystemDialogKind,
};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, glyph_icon, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, CONTENT_INSET, WINDOW_SHADOW, diamond, display_text, window_titled,
};

use crate::screens::shared::{fit_button, glow, key_cap, modal_layer, overline};

/// Sits just below the fade transition so the modal renders over every screen.
/// `pub` so sibling modals (e.g. the death dialog) can anchor strictly below it.
pub const DIALOG_Z: i32 = i32::MAX - 2;
const DIALOG_WIDTH: f32 = 420.0;

pub struct SystemDialogPlugin;

impl Plugin for SystemDialogPlugin {
    fn build(&self, app: &mut App) {
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

/// The tone of a severity: the kicker, the badge and the code chip take it.
fn severity_tone(severity: DialogSeverity) -> Color {
    match severity {
        DialogSeverity::Error => ro_dark::TONE_ERROR,
        DialogSeverity::Warn => ro_dark::RARE_RING,
        DialogSeverity::Info => ro_dark::FROST_GLOW,
        DialogSeverity::Ok => ro_dark::TONE_OK,
    }
}

/// The line-icon name (in `assets/data/ui/icons/`) for a severity.
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

/// The whole modal as one scene: a dimmed, click-eating layer centring the window.
fn system_dialog(request: &ShowSystemDialog) -> impl Scene + use<> {
    let confirm_state = request.confirm_state.clone();
    let kind = request.kind;
    let correlation = request.correlation;
    bsn! {
        @modal_layer(DIALOG_Z)
        SystemDialogRoot::new(confirm_state, kind, correlation)
        BackgroundColor({ro_dark::WINDOW_OUTLINE.with_alpha(0.6)})
        Children [ @window(request) ]
    }
}

fn window(request: &ShowSystemDialog) -> impl Scene + use<> {
    let tone = severity_tone(request.severity);
    let code = (!request.code.is_empty()).then(|| EntityScene(code_chip(&request.code, tone)));
    let secondary = (!request.secondary_label.is_empty())
        .then(|| EntityScene(secondary_button(request.secondary_label.clone())));
    bsn! {
        @window_titled()
        Node {
            width: px(DIALOG_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Children [
            @kicker_bar(request.kicker.to_uppercase(), tone)
            --
            Node { column_gap: px(14), margin: {UiRect::top(px(8))} }
            @ignore_picking()
            Children [
                @badge(severity_icon(request.severity), tone)
                --
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexStart,
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    row_gap: px(4),
                }
                @ignore_picking()
                Children [
                    @display_text(request.title.clone(), 18.0, ro_dark::HERO)
                    --
                    @chrome_text(request.message.clone(), 13.0, ro_dark::LABEL)
                    --
                    {code}
                ]
            ]
            --
            Node {
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(8),
                margin: {UiRect::top(px(18))},
            }
            @ignore_picking()
            Children [
                {secondary}
                --
                @primary_button(request.button_label.clone())
            ]
        ]
    }
}

/// The window's title row, holding the kicker instead of a title.
fn kicker_bar(kicker: String, tone: Color) -> impl Scene {
    let halo = glow(tone);
    bsn! {
        Node {
            height: px(40),
            align_items: AlignItems::Center,
            column_gap: px(10),
            padding: {UiRect::left(px(46.0 - CONTENT_INSET))},
            margin: {UiRect::bottom(px(6))},
        }
        @ignore_picking()
        Children [
            @diamond(8.0, tone, tone)
            halo
            --
            @overline(kicker, tone)
        ]
    }
}

/// The round severity badge holding the tinted line glyph.
fn badge(icon: &'static str, tone: Color) -> impl Scene {
    let halo = glow(tone.with_alpha(0.4));
    bsn! {
        Node {
            width: px(40),
            height: px(40),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor({tone.with_alpha(0.16)})
        BorderColor::all(tone)
        halo
        @ignore_picking()
        Children [ @glyph_icon(icon, 18.0, tone) ]
    }
}

/// The error code under the message.
fn code_chip(code: &str, tone: Color) -> impl Scene + use<> {
    let code = code.to_string();
    bsn! {
        Node {
            height: px(20),
            align_items: AlignItems::Center,
            padding: {UiRect::horizontal(px(7))},
            margin: {UiRect::top(px(6))},
            border: px(1),
            border_radius: BorderRadius::all(px(3)),
        }
        BackgroundColor({tone.with_alpha(0.14)})
        BorderColor::all(tone.with_alpha(0.4))
        @ignore_picking()
        Children [ @chrome_text(code, 11.0, tone) ]
    }
}

/// The primary action, captioned with the request label and the key that triggers it.
fn primary_button(label: String) -> impl Scene {
    let caption = bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @chrome_text(label, 13.0, ro_dark::STRONG)
            --
            @key_cap("Enter")
        ]
    };
    bsn! {
        @fit_button(ButtonArt::PRIMARY, caption)
        on(confirm_on_click)
    }
}

/// The lesser action. It dismisses the dialog and reports a non-primary choice, and
/// never navigates `confirm_state`.
fn secondary_button(label: String) -> impl Scene {
    bsn! {
        @fit_button(ButtonArt::NORMAL, chrome_text(label, 13.0, ro_dark::BUTTON_LABEL))
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
/// the primary path it never navigates `confirm_state`: a decline just closes.
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
    use bevy::scene::ScenePlugin;
    use bevy::ui_widgets::Button;

    #[test]
    fn each_severity_has_its_own_tone_and_glyph() {
        let severities = [
            DialogSeverity::Error,
            DialogSeverity::Warn,
            DialogSeverity::Info,
            DialogSeverity::Ok,
        ];
        for (i, a) in severities.iter().enumerate() {
            for b in &severities[i + 1..] {
                assert_ne!(severity_tone(*a), severity_tone(*b));
                assert_ne!(severity_icon(*a), severity_icon(*b));
            }
        }
    }

    fn request(code: &str, secondary: &str) -> ShowSystemDialog {
        ShowSystemDialog {
            severity: DialogSeverity::Error,
            kind: SystemDialogKind::Generic,
            kicker: "Connection".into(),
            title: "Disconnected".into(),
            message: "The server stopped answering.".into(),
            code: code.into(),
            button_label: "Back to Sign In".into(),
            secondary_label: secondary.into(),
            confirm_state: None,
            correlation: None,
        }
    }

    /// The texts and the number of buttons of the dialog a request spawns.
    fn spawned(request: ShowSystemDialog) -> (Vec<String>, usize) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<ShowSystemDialog>();
        app.add_systems(Update, show_system_dialog);
        app.world_mut().write_message(request);
        app.update();
        let world = app.world_mut();
        let texts = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        let buttons = world.query::<&Button>().iter(world).count();
        (texts, buttons)
    }

    #[test]
    fn a_request_spawns_its_kicker_title_message_and_primary_button() {
        let (texts, buttons) = spawned(request("", ""));
        for expected in [
            "CONNECTION",
            "Disconnected",
            "The server stopped answering.",
            "Back to Sign In",
            "Enter",
        ] {
            assert!(texts.iter().any(|text| text == expected), "{expected}");
        }
        assert_eq!(buttons, 1, "no secondary button without a label");
    }

    #[test]
    fn the_code_and_the_secondary_button_show_only_when_asked_for() {
        let (texts, buttons) = spawned(request("NET-03", "Try Again"));
        assert!(texts.iter().any(|text| text == "NET-03"));
        assert!(texts.iter().any(|text| text == "Try Again"));
        assert_eq!(buttons, 2);
    }

    #[test]
    fn a_second_request_does_not_stack_a_second_dialog() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<ShowSystemDialog>();
        app.add_systems(Update, show_system_dialog);
        app.world_mut().write_message(request("", ""));
        app.update();
        app.world_mut().write_message(request("", ""));
        app.update();
        let world = app.world_mut();
        let roots = world.query::<&SystemDialogRoot>().iter(world).count();
        assert_eq!(roots, 1);
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
