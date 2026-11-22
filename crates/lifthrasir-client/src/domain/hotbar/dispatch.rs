use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_add_system};
use leafwing_input_manager::prelude::ActionState;

use crate::core::state::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::homunculus::HomunculusCastIntent;
use crate::domain::hotbar::model::{Hotbar, HotbarSlot};
use crate::domain::input::{HOTBAR_ACTIONS, PlayerAction, ui_unfocused};
use crate::domain::inventory::{Inventory, UseItemRequested};
use crate::domain::skill::SkillCastRequested;
use crate::domain::vending::{VENDING_SKILL_ID, VendingIntent};
use net_contract::events::ChatNotice;

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin)]
pub struct HotbarSlotActivated {
    pub index: usize,
}

#[auto_add_system(
    plugin = crate::domain::input::plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame).and_then(ui_unfocused))
)]
pub fn activate_from_keys(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    hotbar: Res<Hotbar>,
    keys: Res<ButtonInput<KeyCode>>,
    mut activated: MessageWriter<HotbarSlotActivated>,
) {
    let Ok(state) = player.single() else {
        return;
    };
    let row = second_row_held(&hotbar, &keys);
    for (i, action) in HOTBAR_ACTIONS.iter().enumerate() {
        if state.just_pressed(action) {
            activated.write(HotbarSlotActivated {
                index: hotbar.index(row, i),
            });
        }
    }
}

/// The row a slot key fires: the second one while Shift is held and it is on show.
// NOTE: Shift is read raw, so the second row cannot be rebound and a slot bound to a
// Shift chord always fires it. Give the row its own `PlayerAction`s if that matters.
fn second_row_held(hotbar: &Hotbar, keys: &ButtonInput<KeyCode>) -> usize {
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    usize::from(hotbar.two_rows && shift)
}

#[auto_add_system(
    plugin = crate::domain::world::plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn dispatch_hotbar_activation(
    mut activations: MessageReader<HotbarSlotActivated>,
    hotbar: Res<Hotbar>,
    inventory: Res<Inventory>,
    (mut cast, mut companion): (
        MessageWriter<SkillCastRequested>,
        Option<ResMut<Messages<HomunculusCastIntent>>>,
    ),
    mut vending: MessageWriter<VendingIntent>,
    mut use_item: MessageWriter<UseItemRequested>,
    mut chat: MessageWriter<ChatNotice>,
) {
    for activation in activations.read() {
        let Some(slot) = hotbar.get(activation.index) else {
            continue;
        };
        match slot {
            HotbarSlot::Skill(id) if id == VENDING_SKILL_ID => {
                vending.write(VendingIntent::EnterSeller);
            }
            HotbarSlot::Skill(id) => {
                cast.write(SkillCastRequested { skill_id: id });
            }
            HotbarSlot::HomunculusSkill(skill_id) => {
                if let Some(out) = companion.as_deref_mut() {
                    out.write(HomunculusCastIntent {
                        skill_id,
                        level: None,
                    });
                }
            }
            HotbarSlot::Item(item_id) => match inventory.iter().find(|it| it.item_id == item_id) {
                Some(item) => {
                    use_item.write(UseItemRequested {
                        index: item.index as u32,
                    });
                }
                None => {
                    chat.write(ChatNotice {
                        error: false,
                        message: "You don't have that item.".to_string(),
                    });
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::inventory::item::Item;

    fn dispatch_app() -> App {
        let mut app = App::new();
        app.add_message::<HotbarSlotActivated>()
            .add_message::<SkillCastRequested>()
            .add_message::<VendingIntent>()
            .add_message::<UseItemRequested>()
            .add_message::<ChatNotice>()
            .init_resource::<Hotbar>()
            .init_resource::<Inventory>()
            .add_systems(Update, dispatch_hotbar_activation);
        app
    }

    fn activate(app: &mut App, index: usize) {
        app.world_mut()
            .resource_mut::<Messages<HotbarSlotActivated>>()
            .write(HotbarSlotActivated { index });
        app.update();
    }

    fn casts(app: &App) -> Vec<SkillCastRequested> {
        app.world()
            .resource::<Messages<SkillCastRequested>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn uses(app: &App) -> Vec<UseItemRequested> {
        app.world()
            .resource::<Messages<UseItemRequested>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    fn chats(app: &App) -> Vec<ChatNotice> {
        app.world()
            .resource::<Messages<ChatNotice>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
    }

    #[test]
    fn legacy_hotbar_loads_and_companion_slot_dispatches_only_companion_cast() {
        let mut app = dispatch_app();
        app.add_message::<HomunculusCastIntent>();
        let legacy: HotbarSlot = ron::from_str("Skill(8009)").unwrap();
        let companion = HotbarSlot::HomunculusSkill(8009);
        assert_eq!(legacy, HotbarSlot::Skill(8009));
        assert_eq!(
            ron::from_str::<HotbarSlot>(&ron::to_string(&companion).unwrap()).unwrap(),
            companion
        );
        app.world_mut()
            .resource_mut::<Hotbar>()
            .assign(0, companion);
        activate(&mut app, 0);
        assert!(casts(&app).is_empty());
        let messages = app
            .world()
            .resource::<Messages<HomunculusCastIntent>>()
            .iter_current_update_messages()
            .collect::<Vec<_>>();
        assert!(matches!(
            messages.as_slice(),
            [HomunculusCastIntent {
                skill_id: 8009,
                level: None
            }]
        ));
    }

    #[test]
    fn skill_slot_emits_skill_cast_requested() {
        let mut app = dispatch_app();
        app.world_mut()
            .resource_mut::<Hotbar>()
            .assign(0, HotbarSlot::Skill(42));
        activate(&mut app, 0);

        let msgs = casts(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].skill_id, 42);
        assert!(uses(&app).is_empty());
        assert!(chats(&app).is_empty());
    }

    #[test]
    fn item_in_inventory_emits_use_item_requested() {
        let mut app = dispatch_app();
        app.world_mut()
            .resource_mut::<Hotbar>()
            .assign(1, HotbarSlot::Item(501));
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 3,
            item_id: 501,
            amount: 5,
            ..Default::default()
        });
        activate(&mut app, 1);

        let msgs = uses(&app);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].index, 3);
        assert!(casts(&app).is_empty());
        assert!(chats(&app).is_empty());
    }

    #[test]
    fn item_absent_from_inventory_emits_chat_heard() {
        let mut app = dispatch_app();
        app.world_mut()
            .resource_mut::<Hotbar>()
            .assign(2, HotbarSlot::Item(999));
        activate(&mut app, 2);

        let chat = chats(&app);
        assert_eq!(chat.len(), 1);
        assert!(!chat[0].error);
        assert!(uses(&app).is_empty());
    }

    #[test]
    fn empty_slot_emits_nothing() {
        let mut app = dispatch_app();
        activate(&mut app, 5);

        assert!(casts(&app).is_empty());
        assert!(uses(&app).is_empty());
        assert!(chats(&app).is_empty());
    }

    fn keys_app() -> App {
        let mut app = App::new();
        app.add_message::<HotbarSlotActivated>()
            .init_resource::<Hotbar>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, activate_from_keys);
        app
    }

    fn press_slot3(app: &mut App) {
        let mut state = ActionState::<PlayerAction>::default();
        state.press(&PlayerAction::Slot3);
        app.world_mut().spawn((LocalPlayer, state));
        app.update();
    }

    fn activated(app: &App) -> Vec<usize> {
        app.world()
            .resource::<Messages<HotbarSlotActivated>>()
            .iter_current_update_messages()
            .map(|message| message.index)
            .collect()
    }

    #[test]
    fn no_player_entity_does_not_panic() {
        let mut app = keys_app();
        app.update();
        assert!(activated(&app).is_empty());
    }

    #[test]
    fn slot3_just_pressed_emits_activated_index_2() {
        let mut app = keys_app();
        press_slot3(&mut app);
        assert_eq!(activated(&app), vec![2]);
    }

    #[test]
    fn slot_keys_fire_the_page_on_show() {
        let mut app = keys_app();
        app.world_mut().resource_mut::<Hotbar>().page = 1;
        press_slot3(&mut app);
        assert_eq!(activated(&app), vec![14]);
    }

    #[test]
    fn shift_fires_the_second_row_only_while_it_is_on_show() {
        for (two_rows, expected) in [(true, 14), (false, 2)] {
            let mut app = keys_app();
            app.world_mut().resource_mut::<Hotbar>().two_rows = two_rows;
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ShiftLeft);
            press_slot3(&mut app);
            assert_eq!(activated(&app), vec![expected]);
        }
    }
}
