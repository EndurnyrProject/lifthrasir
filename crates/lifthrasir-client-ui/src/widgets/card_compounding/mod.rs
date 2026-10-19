//! A server-driven modal for selecting the equipment that receives a card.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::paths::item_icon_path;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::inventory::Inventory;
use net_contract::commands::ComposeCardRequested;
use net_contract::events::{
    CardComposeOutcome, CardCompositionFinished, CardTargetsOffered, ChatHeard,
};

use crate::widgets::info_modal::InfoModalRoot;

mod scene;

pub struct CardCompoundingPlugin;

#[derive(Component, Default, Clone)]
struct CardPickerRoot;

#[derive(Component, Clone, Copy, Default)]
struct ConfirmEnabled(bool);

#[derive(Component, Clone, Copy, Default, Debug, PartialEq, Eq)]
enum CardPickerAction {
    #[default]
    Cancel,
    Confirm,
    Select(u32),
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveCardPicker {
    card_index: u32,
    offered_indices: Vec<u32>,
    selected_index: Option<u32>,
    pending_pair: Option<(u32, u32)>,
}

impl Plugin for CardCompoundingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                open_offer,
                handle_result,
                sync_window,
                apply_confirm_disabled,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(Update, cancel_on_escape.run_if(in_state(GameState::InGame)));
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveCardPicker>();
        });
    }
}

fn open_offer(
    mut offers: MessageReader<CardTargetsOffered>,
    existing_info: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
) {
    let Some(offer) = offers.read().last() else {
        return;
    };
    for entity in &existing_info {
        commands.entity(entity).despawn();
    }
    commands.insert_resource(ActiveCardPicker {
        card_index: offer.card_index,
        offered_indices: offer.equipment_indices.clone(),
        selected_index: None,
        pending_pair: None,
    });
}

fn row_views(
    picker: &ActiveCardPicker,
    inventory: &Inventory,
    item_db: Option<&ItemDb>,
) -> Vec<scene::CardRowView> {
    picker
        .offered_indices
        .iter()
        .filter_map(|&index| {
            let item = inventory.get(u16::try_from(index).ok()?)?;
            let name = item_db
                .and_then(|db| db.name(item.item_id, item.identified))
                .map(str::to_string)
                .unwrap_or_else(|| format!("Item #{}", item.item_id));
            let icon_path = item_db
                .and_then(|db| db.icon_resource(item.item_id, item.identified))
                .map(item_icon_path);
            Some(scene::CardRowView {
                index,
                label: if item.refine > 0 {
                    format!("+{} {name}", item.refine)
                } else {
                    name
                },
                icon_path,
                selected: picker.selected_index == Some(index),
            })
        })
        .collect()
}

fn sync_window(
    picker: Option<Res<ActiveCardPicker>>,
    roots: Query<Entity, With<CardPickerRoot>>,
    inventory: Res<Inventory>,
    item_db: Option<Res<ItemDb>>,
    mut commands: Commands,
) {
    let Some(picker) = picker else {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    };
    if !roots.is_empty()
        && !picker.is_changed()
        && !inventory.is_changed()
        && !item_db.as_ref().is_some_and(|db| db.is_changed())
    {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let card_name = u16::try_from(picker.card_index)
        .ok()
        .and_then(|index| inventory.get(index))
        .map(|item| {
            item_db
                .as_deref()
                .and_then(|db| db.name(item.item_id, item.identified))
                .map(str::to_string)
                .unwrap_or_else(|| format!("Item #{}", item.item_id))
        })
        .unwrap_or_else(|| format!("card #{}", picker.card_index));
    let rows = row_views(&picker, &inventory, item_db.as_deref());
    let can_confirm = picker.pending_pair.is_none()
        && picker
            .selected_index
            .is_some_and(|index| rows.iter().any(|row| row.index == index))
        && u16::try_from(picker.card_index)
            .ok()
            .is_some_and(|index| inventory.get(index).is_some());
    commands
        .spawn_scene(scene::window(
            card_name,
            rows,
            picker.pending_pair.is_some(),
            can_confirm,
        ))
        .insert(DespawnOnExit(GameState::InGame));
}

fn apply_confirm_disabled(
    buttons: Query<(Entity, &ConfirmEnabled), Added<ConfirmEnabled>>,
    mut commands: Commands,
) {
    for (entity, enabled) in &buttons {
        if !enabled.0 {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
}

fn on_action(
    activate: On<Activate>,
    actions: Query<&CardPickerAction>,
    picker: Option<ResMut<ActiveCardPicker>>,
    inventory: Res<Inventory>,
    mut commands: Commands,
    mut compose: MessageWriter<ComposeCardRequested>,
) {
    let (Ok(action), Some(mut picker)) = (actions.get(activate.entity), picker) else {
        return;
    };
    match *action {
        CardPickerAction::Cancel => {
            commands.remove_resource::<ActiveCardPicker>();
        }
        CardPickerAction::Select(index)
            if picker.pending_pair.is_none()
                && picker.offered_indices.contains(&index)
                && u16::try_from(index)
                    .ok()
                    .is_some_and(|i| inventory.get(i).is_some()) =>
        {
            picker.selected_index = Some(index);
        }
        CardPickerAction::Confirm if picker.pending_pair.is_none() => {
            let Some(index) = picker
                .selected_index
                .filter(|i| picker.offered_indices.contains(i))
            else {
                return;
            };
            let (Ok(card), Ok(target)) = (u16::try_from(picker.card_index), u16::try_from(index))
            else {
                return;
            };
            if inventory.get(card).is_none() || inventory.get(target).is_none() {
                return;
            }
            compose.write(ComposeCardRequested {
                card_index: picker.card_index,
                equipment_index: index,
            });
            picker.pending_pair = Some((picker.card_index, index));
        }
        _ => {}
    }
}

fn on_backdrop_click(_: On<Pointer<Click>>, mut commands: Commands) {
    commands.remove_resource::<ActiveCardPicker>();
}

fn cancel_on_escape(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::Escape) {
        commands.remove_resource::<ActiveCardPicker>();
    }
}

fn result_message(outcome: CardComposeOutcome) -> String {
    let reason = match outcome {
        CardComposeOutcome::Success => return "Card compounded successfully.".into(),
        CardComposeOutcome::CardNotFound => "Card not found",
        CardComposeOutcome::NotACard => "Item is not a card",
        CardComposeOutcome::SourceEquipped => "Card is equipped",
        CardComposeOutcome::TargetNotFound => "Equipment not found",
        CardComposeOutcome::SameInventorySlot => "Card and equipment share a slot",
        CardComposeOutcome::NotEquipment => "Target is not equipment",
        CardComposeOutcome::TargetUnidentified => "Equipment is unidentified",
        CardComposeOutcome::TargetEquipped => "Equipment is equipped",
        CardComposeOutcome::LocationMismatch => "Card does not fit the equipment",
        CardComposeOutcome::NoFreeSocket => "No free socket",
        CardComposeOutcome::ItemUseDisabled => "Item use is disabled",
        CardComposeOutcome::PersistenceFailed => "Could not save the composition",
        CardComposeOutcome::Unknown(_) => "Unknown rejection",
    };
    format!("Card compounding failed: {reason}.")
}

fn handle_result(
    mut results: MessageReader<CardCompositionFinished>,
    picker: Option<Res<ActiveCardPicker>>,
    mut chat: MessageWriter<ChatHeard>,
    mut commands: Commands,
) {
    let mut matched = false;
    for result in results.read() {
        if matched
            || picker.as_deref().and_then(|p| p.pending_pair)
                != Some((result.card_index, result.equipment_index))
        {
            continue;
        }
        chat.write(ChatHeard {
            gid: 0,
            message: result_message(result.outcome),
        });
        commands.remove_resource::<ActiveCardPicker>();
        matched = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use bevy::state::app::StatesPlugin;
    use lifthrasir_assets::catalogs::item::registry::ItemDb;
    use lifthrasir_client::domain::inventory::{Inventory, Item};
    use lifthrasir_data::{ItemData, ItemInfo};

    fn picker_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ScenePlugin,
            StatesPlugin,
        ));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.init_asset::<Shader>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_state::<GameState>();
        app.add_message::<CardTargetsOffered>();
        app.add_message::<CardCompositionFinished>();
        app.add_message::<ComposeCardRequested>();
        app.add_message::<ChatHeard>();
        app.insert_resource(Inventory::default());
        crate::install_test_ui_primitives(&mut app);
        app.add_systems(
            Update,
            (
                open_offer,
                handle_result,
                sync_window,
                apply_confirm_disabled,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(Update, cancel_on_escape.run_if(in_state(GameState::InGame)));
        app.add_systems(OnExit(GameState::InGame), |mut commands: Commands| {
            commands.remove_resource::<ActiveCardPicker>()
        });
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app
    }

    #[test]
    fn card_offer_renders_named_targets_and_omits_missing_items() {
        let mut app = picker_app();
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 9,
            item_id: 2104,
            identified: true,
            ..Default::default()
        });
        let mut data = ItemData::default();
        data.items.insert(
            2104,
            ItemInfo {
                identified_name: "Buckler".into(),
                identified_resource: "BUCKLER".into(),
                ..Default::default()
            },
        );
        app.insert_resource(ItemDb::from_item_data(data));
        app.world_mut().write_message(CardTargetsOffered {
            card_index: 7,
            equipment_indices: vec![9, 10],
        });
        app.update();
        let labels: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            labels.iter().any(|label| label.contains("Buckler")),
            "{labels:?}"
        );
        assert!(
            !labels.iter().any(|label| label.contains("#10")),
            "{labels:?}"
        );
        assert_eq!(
            app.world_mut()
                .query::<&ImageNode>()
                .iter(app.world())
                .count(),
            1
        );
    }

    fn offer_with_equipment(app: &mut App, indices: Vec<u32>) {
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            item_id: 4001,
            amount: 2,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 9,
            item_id: 2104,
            amount: 1,
            cards: [0; 4],
            ..Default::default()
        });
        app.world_mut().write_message(CardTargetsOffered {
            card_index: 7,
            equipment_indices: indices,
        });
        app.update();
    }

    fn activate(app: &mut App, action: CardPickerAction) {
        let entity = app
            .world_mut()
            .query::<(Entity, &CardPickerAction)>()
            .iter(app.world())
            .find_map(|(entity, candidate)| (*candidate == action).then_some(entity))
            .expect("action button exists");
        app.world_mut().trigger(Activate { entity });
    }

    #[test]
    fn confirming_offered_equipment_sends_one_request_and_locks() {
        let mut app = picker_app();
        offer_with_equipment(&mut app, vec![9]);
        activate(&mut app, CardPickerAction::Select(9));
        app.update();
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "✓ Item #2104")
        );
        activate(&mut app, CardPickerAction::Confirm);
        activate(&mut app, CardPickerAction::Confirm);
        let messages = app.world().resource::<Messages<ComposeCardRequested>>();
        let requests: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            (requests[0].card_index, requests[0].equipment_index),
            (7, 9)
        );
        assert_eq!(
            app.world().resource::<Inventory>().get(9).unwrap().cards,
            [0; 4]
        );
    }

    #[test]
    fn matching_result_closes_picker_once_and_unrelated_result_does_not() {
        let mut app = picker_app();
        offer_with_equipment(&mut app, vec![9]);
        activate(&mut app, CardPickerAction::Select(9));
        app.update();
        activate(&mut app, CardPickerAction::Confirm);
        let result = |equipment_index, outcome| CardCompositionFinished {
            card_index: 7,
            equipment_index,
            outcome,
            cards: vec![],
        };
        app.world_mut()
            .write_message(result(10, CardComposeOutcome::Success));
        app.update();
        assert_eq!(
            app.world().resource::<ActiveCardPicker>().pending_pair,
            Some((7, 9))
        );
        assert_eq!(
            app.world()
                .resource::<Messages<ChatHeard>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        app.world_mut()
            .write_message(result(9, CardComposeOutcome::NoFreeSocket));
        app.world_mut()
            .write_message(result(9, CardComposeOutcome::NoFreeSocket));
        app.update();
        assert!(!app.world().contains_resource::<ActiveCardPicker>());
        let messages = app.world().resource::<Messages<ChatHeard>>();
        let feedback: Vec<_> = messages.iter_current_update_messages().collect();
        assert_eq!(feedback.len(), 1);
        assert!(feedback[0].message.contains("No free socket"));
        assert_eq!(
            app.world().resource::<Inventory>().get(9).unwrap().cards,
            [0; 4]
        );
    }

    #[test]
    fn matching_success_and_unknown_results_report_feedback_without_mutating_inventory() {
        for (outcome, expected) in [
            (CardComposeOutcome::Success, "Card compounded successfully."),
            (
                CardComposeOutcome::Unknown(99),
                "Card compounding failed: Unknown rejection.",
            ),
        ] {
            let mut app = picker_app();
            offer_with_equipment(&mut app, vec![9]);
            activate(&mut app, CardPickerAction::Select(9));
            app.update();
            activate(&mut app, CardPickerAction::Confirm);
            app.world_mut().write_message(CardCompositionFinished {
                card_index: 7,
                equipment_index: 9,
                outcome,
                cards: vec![401, 0, 0, 0],
            });
            app.update();
            assert!(!app.world().contains_resource::<ActiveCardPicker>());
            let chat = app.world().resource::<Messages<ChatHeard>>();
            let feedback: Vec<_> = chat.iter_current_update_messages().collect();
            assert_eq!(feedback.len(), 1);
            assert_eq!(feedback[0].message, expected);
            assert_eq!(
                app.world().resource::<Inventory>().get(9).unwrap().cards,
                [0; 4]
            );
        }
    }

    #[test]
    fn empty_and_unoffered_targets_cannot_be_confirmed() {
        let mut app = picker_app();
        offer_with_equipment(&mut app, vec![]);
        let labels: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(labels.contains(&"No compatible equipment available.".to_string()));
        assert!(
            app.world_mut()
                .query::<(&CardPickerAction, &InteractionDisabled)>()
                .iter(app.world())
                .any(|(action, _)| *action == CardPickerAction::Confirm)
        );
        activate(&mut app, CardPickerAction::Confirm);
        assert_eq!(
            app.world()
                .resource::<Messages<ComposeCardRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );

        app.world_mut().write_message(CardTargetsOffered {
            card_index: 7,
            equipment_indices: vec![9],
        });
        app.update();
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 10,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<ActiveCardPicker>()
            .selected_index = Some(10);
        activate(&mut app, CardPickerAction::Confirm);
        assert_eq!(
            app.world()
                .resource::<Messages<ComposeCardRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    #[test]
    fn missing_item_db_and_stale_slots_never_force_a_request() {
        let mut app = picker_app();
        offer_with_equipment(&mut app, vec![9, 10]);
        let labels: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(labels.contains(&"Item #2104".to_string()));
        activate(&mut app, CardPickerAction::Select(9));
        app.update();
        app.world_mut()
            .resource_mut::<Inventory>()
            .remove_amount(9, 1);
        activate(&mut app, CardPickerAction::Confirm);
        assert_eq!(
            app.world()
                .resource::<Messages<ComposeCardRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        activate(&mut app, CardPickerAction::Cancel);
        app.update();
        assert!(!app.world().contains_resource::<ActiveCardPicker>());

        offer_with_equipment(&mut app, vec![9]);
        activate(&mut app, CardPickerAction::Select(9));
        app.update();
        app.world_mut()
            .resource_mut::<Inventory>()
            .remove_amount(7, 2);
        activate(&mut app, CardPickerAction::Confirm);
        assert_eq!(
            app.world()
                .resource::<Messages<ComposeCardRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    #[test]
    fn escape_cancels_and_exit_clears_pending_picker() {
        let mut app = picker_app();
        offer_with_equipment(&mut app, vec![9]);
        app.world_mut()
            .write_message(bevy::input::keyboard::KeyboardInput {
                key_code: KeyCode::Escape,
                logical_key: bevy::input::keyboard::Key::Escape,
                state: bevy::input::ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        app.update();
        assert!(!app.world().contains_resource::<ActiveCardPicker>());
        assert_eq!(
            app.world()
                .resource::<Messages<ComposeCardRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        offer_with_equipment(&mut app, vec![9]);
        activate(&mut app, CardPickerAction::Select(9));
        app.update();
        activate(&mut app, CardPickerAction::Confirm);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::CharacterSelection);
        app.update();
        assert!(!app.world().contains_resource::<ActiveCardPicker>());
        assert!(
            app.world_mut()
                .query::<&CardPickerRoot>()
                .iter(app.world())
                .next()
                .is_none()
        );
    }

    #[test]
    fn card_result_feedback_is_distinct_and_unknown_is_not_success() {
        let outcomes = [
            CardComposeOutcome::Success,
            CardComposeOutcome::CardNotFound,
            CardComposeOutcome::NotACard,
            CardComposeOutcome::SourceEquipped,
            CardComposeOutcome::TargetNotFound,
            CardComposeOutcome::SameInventorySlot,
            CardComposeOutcome::NotEquipment,
            CardComposeOutcome::TargetUnidentified,
            CardComposeOutcome::TargetEquipped,
            CardComposeOutcome::LocationMismatch,
            CardComposeOutcome::NoFreeSocket,
            CardComposeOutcome::ItemUseDisabled,
            CardComposeOutcome::PersistenceFailed,
            CardComposeOutcome::Unknown(99),
        ];
        let messages: Vec<_> = outcomes.into_iter().map(result_message).collect();
        assert_eq!(
            messages.len(),
            messages
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
        assert_eq!(messages[0], "Card compounded successfully.");
        assert!(messages.last().unwrap().contains("Unknown rejection"));
    }

    #[test]
    fn card_offer_replaces_an_open_item_info_modal() {
        let mut app = picker_app();
        let info = app.world_mut().spawn(InfoModalRoot).id();
        offer_with_equipment(&mut app, vec![9]);
        assert!(app.world().get_entity(info).is_err());
        assert_eq!(
            app.world_mut()
                .query::<&CardPickerRoot>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn server_offer_opens_card_picker() {
        let mut app = picker_app();

        app.world_mut().write_message(CardTargetsOffered {
            card_index: 7,
            equipment_indices: vec![9],
        });
        app.update();

        let picker = app
            .world()
            .get_resource::<ActiveCardPicker>()
            .expect("open picker");
        assert_eq!(picker.card_index, 7);
        assert_eq!(picker.offered_indices, vec![9]);
    }
}
