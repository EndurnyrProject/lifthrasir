//! Create-party modal (BSN + Feathers), reusing `system_dialog`'s glass-card look.
//!
//! Opened from the roster window's partyless empty state, it centers a click-eating
//! backdrop behind a glass card matching the system dialog's chrome: a title, an
//! [`EditableText`] name field (the same primitive the chat input uses), a Primary
//! "Create" `@FeathersButton`, and a "Cancel" button. Submitting a non-empty (trimmed)
//! name writes exactly one [`PartyCreateRequested`] and closes the modal; Cancel or an
//! empty name closes without emitting.
//!
//! It layers strictly below the system dialog (a static assert pins `DIALOG_Z` under
//! `system_dialog::DIALOG_Z`), so the critical disconnect notice always stacks above.
//! A single-open guard keeps two create dialogs from stacking; focus is handed to the
//! name field on spawn so the user can type immediately.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, FontSize, FontSourceTemplate};
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;
use net_contract::commands::PartyCreateRequested;

use crate::theme;
use crate::widgets::chrome::ignore_picking;
use crate::widgets::system_dialog;

/// Its own tier strictly below both the death dialog (`MAX - 3`) and the system dialog
/// (`MAX - 2`): ordering is create (`MAX - 4`) < death (`MAX - 3`) < system (`MAX - 2`).
/// A distinct lower tier means a death or disconnect modal opening over the create form
/// always renders above and stays clickable (equal-z UI peers have no guaranteed
/// stacking order). `death_dialog::DIALOG_Z` is private, so the assert pins the one
/// reachable neighbour and the numeric value keeps create below the death tier.
const DIALOG_Z: i32 = i32::MAX - 4;

const _: () = assert!(
    DIALOG_Z < system_dialog::DIALOG_Z,
    "create-party dialog must stack under the system dialog so the disconnect modal stays clickable"
);

const DIALOG_WIDTH: f32 = 360.0;
const NAME_MAX_CHARS: usize = 24;

/// The modal root; the single-open guard and the Create/Cancel observers resolve it by
/// this marker.
#[derive(Component, Default, Clone)]
pub struct CreatePartyDialogRoot;

/// The name input field. The Create observer reads its value by this marker; the focus
/// system grabs it on spawn.
#[derive(Component, Default, Clone)]
pub struct CreatePartyNameField;

/// Decide the party name to submit: trimmed and non-empty, else `None` (Cancel/empty
/// closes without emitting). Split out so the submit decision is unit-testable without a
/// live `EditableText`.
pub fn submit_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Open the create dialog, guarding against a second one. Attached to the roster
/// empty-state "Create a party" button via `on(...)`.
pub fn open_create_dialog(
    _: On<Activate>,
    existing: Query<(), With<CreatePartyDialogRoot>>,
    mut commands: Commands,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn_scene(create_dialog());
}

/// Hand keyboard focus to the freshly spawned name field so the user can type at once,
/// mirroring how `chat_input_control` focuses the chat input.
pub fn focus_new_name_field(
    field: Query<Entity, Added<CreatePartyNameField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    let Ok(entity) = field.single() else {
        return;
    };
    input_focus.set(entity, FocusCause::Navigated);
}

/// The whole modal as one scene: a dimmed, click-eating backdrop centering the glass card.
fn create_dialog() -> impl Scene {
    bsn! {
        CreatePartyDialogRoot
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
        Children [ card() ]
    }
}

/// The glass card: title, name field, divider, and the Create/Cancel button row. Reuses
/// `system_dialog`'s card chrome (glass fill, hairline stroke, rounded corners) so the
/// party dialogs read as the same family.
fn card() -> impl Scene {
    bsn! {
        Node {
            width: px(DIALOG_WIDTH),
            padding: {UiRect::axes(px(30), px(26))},
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(16)),
        }
        BackgroundColor({theme::GLASS})
        BorderColor::all(theme::STROKE)
        Children [
            title(),
            name_field(),
            divider(),
            button_row(),
        ]
    }
}

/// The display-font card title.
fn title() -> impl Scene {
    bsn! {
        Text("Create Party")
        TextFont {
            font: FontSourceTemplate::Handle(theme::FONT_TITLE),
            font_size: {FontSize::Px(23.0)},
        }
        TextColor({theme::DISPLAY_GOLD})
        Node { margin: {UiRect::bottom(px(4))} }
        ignore_picking()
    }
}

/// The name input well: a field-tinted rounded box holding the `EditableText` the user
/// types into. `EditableText` carries a `PlainEditor`, not a patchable literal, so it is
/// passed via `template_value`.
fn name_field() -> impl Scene {
    let editable = EditableText {
        max_characters: Some(NAME_MAX_CHARS),
        ..default()
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            height: px(40),
            padding: {UiRect::horizontal(px(12))},
            margin: {UiRect::top(px(16))},
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(theme::STROKE)
        Children [
            (
                CreatePartyNameField
                template_value(editable)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(14.0)},
                }
                TextColor({theme::TEXT})
                Node { flex_grow: 1.0, height: px(18) }
            ),
        ]
    }
}

/// Hairline separator between the field and the button row.
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

/// Create (Primary) + Cancel, side by side.
fn button_row() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: px(10),
        }
        ignore_picking()
        Children [
            (
                @FeathersButton { @caption: bsn! { (Text("Cancel") ThemedText) } }
                Node { flex_grow: 1.0, height: px(44), border_radius: BorderRadius::all(px(11)) }
                on(on_cancel)
            ),
            (
                @FeathersButton {
                    @caption: bsn! { (Text("Create") ThemedText) },
                    @variant: ButtonVariant::Primary,
                }
                Node { flex_grow: 1.0, height: px(44), border_radius: BorderRadius::all(px(11)) }
                on(on_create)
            ),
        ]
    }
}

fn on_create(
    _: On<Activate>,
    field: Query<&EditableText, With<CreatePartyNameField>>,
    root: Query<Entity, With<CreatePartyDialogRoot>>,
    mut writer: MessageWriter<PartyCreateRequested>,
    mut commands: Commands,
) {
    if let Ok(field) = field.single()
        && let Some(name) = submit_name(&field.value().to_string()) {
            writer.write(PartyCreateRequested { name });
        }
    close_dialog(&root, &mut commands);
}

fn on_cancel(
    _: On<Activate>,
    root: Query<Entity, With<CreatePartyDialogRoot>>,
    mut commands: Commands,
) {
    close_dialog(&root, &mut commands);
}

fn close_dialog(root: &Query<Entity, With<CreatePartyDialogRoot>>, commands: &mut Commands) {
    if let Ok(root) = root.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_name_keeps_trimmed_non_empty() {
        assert_eq!(submit_name("  Wolfpack  "), Some("Wolfpack".to_string()));
    }

    #[test]
    fn submit_name_rejects_empty_and_whitespace() {
        assert_eq!(submit_name(""), None);
        assert_eq!(submit_name("   "), None);
    }

    #[test]
    fn create_writes_command_and_despawns_root() {
        let mut app = App::new();
        app.add_message::<PartyCreateRequested>();

        let root = app.world_mut().spawn(CreatePartyDialogRoot).id();
        app.world_mut()
            .spawn((CreatePartyNameField, EditableText::new("Wolfpack")));
        let button = app.world_mut().spawn_empty().observe(on_create).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyCreateRequested>>();
        let mut cursor = messages.get_cursor();
        let written: Vec<_> = cursor.read(messages).collect();
        assert_eq!(written.len(), 1, "exactly one create command");
        assert_eq!(written[0].name, "Wolfpack");
        assert!(
            app.world().get_entity(root).is_err(),
            "creating despawns the dialog root"
        );
    }

    #[test]
    fn empty_name_despawns_without_command() {
        let mut app = App::new();
        app.add_message::<PartyCreateRequested>();

        let root = app.world_mut().spawn(CreatePartyDialogRoot).id();
        app.world_mut()
            .spawn((CreatePartyNameField, EditableText::new("   ")));
        let button = app.world_mut().spawn_empty().observe(on_create).id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let messages = app.world().resource::<Messages<PartyCreateRequested>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(
            cursor.read(messages).count(),
            0,
            "empty name emits no command"
        );
        assert!(
            app.world().get_entity(root).is_err(),
            "empty submit still closes the dialog"
        );
    }
}
