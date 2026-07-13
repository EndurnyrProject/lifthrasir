use bevy_auto_plugin::prelude::*;

pub mod commands;
pub mod dto;
pub mod events;
pub mod state;

#[derive(AutoPlugin)]
#[auto_plugin(impl_plugin_trait)]
pub struct NetContractPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn net_contract_plugin_registers_moved_messages() {
        let mut app = App::new();
        app.add_plugins(NetContractPlugin);

        assert!(app
            .world()
            .contains_resource::<Messages<events::SelfMoved>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::CharacterServerConnected>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::LoginAccepted>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::NpcDialogReceived>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::TalkToNpc>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::RespondToNpc>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::ShopOpened>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::BuyFromShop>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::CartLoaded>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::CartItemAdded>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::CartItemRemoved>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::CartMountResult>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::MountCart>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::MoveToCart>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::MoveFromCart>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::StorageOpened>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::StorageItemAdded>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::StorageItemRemoved>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::StorageResult>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::DepositStorageItem>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::WithdrawStorageItem>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::CloseStorage>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::PartyInfoReceived>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::PartyInviteNotified>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::PartyActionResulted>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::PartyDisbanded>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::PartyCreateRequested>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::PartyInviteRequested>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::PartyInviteResponded>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::PartyLeaveRequested>>());
        assert!(app
            .world()
            .contains_resource::<Messages<events::EmoteShown>>());
        assert!(app
            .world()
            .contains_resource::<Messages<commands::EmoteSent>>());
    }
}
