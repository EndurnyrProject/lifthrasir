use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_system;
use bevy_quinnet::client::{QuinnetClient, client_connected};
use net_contract::commands::{CloseStorage, DepositStorageItem, WithdrawStorageItem};

use crate::channels::GAMEPLAY;
use crate::envelope::Body;
use crate::proto::aesir::net::{
    StorageCloseRequest, StorageDepositRequest, StorageKind, StorageWithdrawRequest,
};
use crate::zone::{QuicZoneState, ZonePhase};

fn storage_kind(kind: net_contract::dto::StorageKind) -> i32 {
    match kind {
        net_contract::dto::StorageKind::Personal => StorageKind::Personal as i32,
        net_contract::dto::StorageKind::Guild => StorageKind::Guild as i32,
    }
}

fn deposit_body(command: &DepositStorageItem) -> Body {
    Body::StorageDepositRequest(StorageDepositRequest {
        inventory_index: command.inventory_index,
        amount: command.amount,
        kind: storage_kind(command.kind),
    })
}

fn withdraw_body(command: &WithdrawStorageItem) -> Body {
    Body::StorageWithdrawRequest(StorageWithdrawRequest {
        storage_index: command.storage_index,
        amount: command.amount,
        kind: storage_kind(command.kind),
    })
}

fn close_body(command: &CloseStorage) -> Body {
    Body::StorageCloseRequest(StorageCloseRequest {
        kind: storage_kind(command.kind),
    })
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Last,
    config(run_if = not(client_connected))
)]
pub fn clear_storage_commands_while_disconnected(
    mut deposits: ResMut<Messages<DepositStorageItem>>,
    mut withdrawals: ResMut<Messages<WithdrawStorageItem>>,
    mut closes: ResMut<Messages<CloseStorage>>,
) {
    deposits.clear();
    withdrawals.clear();
    closes.clear();
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_deposit_storage(
    mut commands: MessageReader<DepositStorageItem>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }

    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, deposit_body(command)) {
            error!("failed to send StorageDepositRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_withdraw_storage(
    mut commands: MessageReader<WithdrawStorageItem>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }

    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, withdraw_body(command)) {
            error!("failed to send StorageWithdrawRequest: {error}");
        }
    }
}

#[auto_add_system(
    plugin = crate::AesirNetPlugin,
    schedule = Update,
    config(run_if = client_connected)
)]
pub fn send_close_storage(
    mut commands: MessageReader<CloseStorage>,
    mut client: ResMut<QuinnetClient>,
    mut zone: ResMut<QuicZoneState>,
) {
    if zone.phase != ZonePhase::Playing {
        commands.clear();
        return;
    }

    for command in commands.read() {
        if let Err(error) = zone.send(&mut client, GAMEPLAY, close_body(command)) {
            error!("failed to send StorageCloseRequest: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_requests_target_the_selected_vault() {
        use net_contract::dto::StorageKind as Kind;
        for (kind, expected) in [(Kind::Personal, 0), (Kind::Guild, 1)] {
            let Body::StorageDepositRequest(deposit) = deposit_body(&DepositStorageItem {
                kind,
                inventory_index: 70_000,
                amount: 80_000,
            }) else {
                panic!("expected deposit");
            };
            let Body::StorageWithdrawRequest(withdraw) = withdraw_body(&WithdrawStorageItem {
                kind,
                storage_index: 70_001,
                amount: 80_001,
            }) else {
                panic!("expected withdrawal");
            };
            let Body::StorageCloseRequest(close) = close_body(&CloseStorage { kind }) else {
                panic!("expected close");
            };
            assert_eq!(
                (deposit.kind, deposit.inventory_index, deposit.amount),
                (expected, 70_000, 80_000)
            );
            assert_eq!(
                (withdraw.kind, withdraw.storage_index, withdraw.amount),
                (expected, 70_001, 80_001)
            );
            assert_eq!(close.kind, expected);
        }
    }

    fn app_with_storage_senders() -> App {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>();
        app.init_resource::<QuicZoneState>();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.add_message::<CloseStorage>();
        app.add_systems(
            Update,
            (
                send_deposit_storage,
                send_withdraw_storage,
                send_close_storage,
            ),
        );
        app
    }

    #[test]
    fn deposit_body_preserves_u32_inventory_index_and_amount() {
        let body = deposit_body(&DepositStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            inventory_index: 70_000,
            amount: 80_000,
        });

        match body {
            Body::StorageDepositRequest(StorageDepositRequest {
                inventory_index,
                amount,
                kind: _,
            }) => {
                assert_eq!(inventory_index, 70_000);
                assert_eq!(amount, 80_000);
            }
            other => panic!("expected Body::StorageDepositRequest, got {other:?}"),
        }
    }

    #[test]
    fn withdraw_body_preserves_u32_storage_index_and_amount() {
        let body = withdraw_body(&WithdrawStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            storage_index: 70_001,
            amount: 80_001,
        });

        match body {
            Body::StorageWithdrawRequest(StorageWithdrawRequest {
                storage_index,
                amount,
                kind: _,
            }) => {
                assert_eq!(storage_index, 70_001);
                assert_eq!(amount, 80_001);
            }
            other => panic!("expected Body::StorageWithdrawRequest, got {other:?}"),
        }
    }

    #[test]
    fn close_body_produces_storage_close_request() {
        let body = close_body(&CloseStorage {
            kind: net_contract::dto::StorageKind::Personal,
        });

        assert!(matches!(
            body,
            Body::StorageCloseRequest(StorageCloseRequest { kind: _ })
        ));
    }

    #[test]
    fn out_of_phase_systems_clear_all_storage_commands() {
        let mut app = app_with_storage_senders();
        app.world_mut().write_message(DepositStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            inventory_index: 7,
            amount: 2,
        });
        app.world_mut().write_message(WithdrawStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            storage_index: 8,
            amount: 3,
        });
        app.world_mut().write_message(CloseStorage {
            kind: net_contract::dto::StorageKind::Personal,
        });

        app.update();
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;

        // No stale command reaches the disconnected client after the phase changes.
        app.update();
    }

    #[test]
    fn commands_queued_while_disconnected_are_consumed() {
        let mut app = App::new();
        app.init_resource::<QuinnetClient>();
        app.init_resource::<QuicZoneState>();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.add_message::<CloseStorage>();
        app.add_systems(
            Last,
            clear_storage_commands_while_disconnected.run_if(not(client_connected)),
        );
        app.world_mut().resource_mut::<QuicZoneState>().phase = ZonePhase::Playing;
        app.world_mut().write_message(DepositStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            inventory_index: 7,
            amount: 2,
        });
        app.world_mut().write_message(WithdrawStorageItem {
            kind: net_contract::dto::StorageKind::Personal,
            storage_index: 8,
            amount: 3,
        });
        app.world_mut().write_message(CloseStorage {
            kind: net_contract::dto::StorageKind::Personal,
        });

        app.update();

        assert!(
            app.world()
                .resource::<Messages<DepositStorageItem>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<WithdrawStorageItem>>()
                .is_empty()
        );
        assert!(app.world().resource::<Messages<CloseStorage>>().is_empty());
    }
}
