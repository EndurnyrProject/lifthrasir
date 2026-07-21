use bevy::prelude::*;
use bevy_auto_plugin::prelude::{auto_add_message, auto_add_system};
use leafwing_input_manager::prelude::ActionState;

use crate::core::state::GameState;
use crate::domain::entities::markers::LocalPlayer;
use crate::domain::hotbar::model::{Hotbar, HotbarSlot};
use crate::domain::input::{HOTBAR_ACTIONS, PlayerAction, ui_unfocused};
use crate::domain::inventory::{Inventory, UseItemRequested};
use crate::domain::skill::SkillCastRequested;
use net_contract::events::ChatHeard;

#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin)]
pub struct HotbarSlotActivated {
    pub index: usize,
}

#[auto_add_system(
    plugin = crate::app::input_plugin::InputPlugin,
    schedule = Update,
    config(run_if = in_state(GameState::InGame).and_then(ui_unfocused))
)]
pub fn activate_from_keys(
    player: Query<&ActionState<PlayerAction>, With<LocalPlayer>>,
    mut activated: MessageWriter<HotbarSlotActivated>,
) {
    let Ok(state) = player.single() else {
        return;
    };
    for (i, action) in HOTBAR_ACTIONS.iter().enumerate() {
        if state.just_pressed(action) {
            activated.write(HotbarSlotActivated { index: i });
        }
    }
}

#[auto_add_system(
    plugin = crate::app::zone_domain_plugin::ZoneDomainAutoPlugin,
    schedule = Update
)]
pub fn dispatch_hotbar_activation(
    mut activations: MessageReader<HotbarSlotActivated>,
    hotbar: Res<Hotbar>,
    inventory: Res<Inventory>,
    mut cast: MessageWriter<SkillCastRequested>,
    mut use_item: MessageWriter<UseItemRequested>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for activation in activations.read() {
        let Some(slot) = hotbar.get(activation.index) else {
            continue;
        };
        match slot {
            HotbarSlot::Skill(id) => {
                cast.write(SkillCastRequested { skill_id: id });
            }
            HotbarSlot::Item(item_id) => match inventory.iter().find(|it| it.item_id == item_id) {
                Some(item) => {
                    use_item.write(UseItemRequested {
                        index: item.index as u32,
                    });
                }
                None => {
                    chat.write(ChatHeard {
                        gid: 0,
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
            .add_message::<UseItemRequested>()
            .add_message::<ChatHeard>()
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

    fn chats(app: &App) -> Vec<ChatHeard> {
        app.world()
            .resource::<Messages<ChatHeard>>()
            .iter_current_update_messages()
            .cloned()
            .collect()
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
        assert_eq!(chat[0].gid, 0);
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

    #[test]
    fn no_player_entity_does_not_panic() {
        let mut app = App::new();
        app.add_message::<HotbarSlotActivated>()
            .add_systems(Update, activate_from_keys);
        app.update();

        let msgs: Vec<HotbarSlotActivated> = app
            .world()
            .resource::<Messages<HotbarSlotActivated>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert!(msgs.is_empty());
    }

    #[test]
    fn slot3_just_pressed_emits_activated_index_2() {
        let mut app = App::new();
        app.add_message::<HotbarSlotActivated>()
            .add_systems(Update, activate_from_keys);
        let mut state = ActionState::<PlayerAction>::default();
        state.press(&PlayerAction::Slot3);
        app.world_mut().spawn((LocalPlayer, state));
        app.update();

        let msgs: Vec<HotbarSlotActivated> = app
            .world()
            .resource::<Messages<HotbarSlotActivated>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].index, 2);
    }
}
