use crate::envelope::Body;
use crate::proto::aesir::net;
use net_contract::commands::PickupRequested;
use net_contract::events::{ItemOnGround, ItemVanished, PickupOutcome, PickupResult, VanishReason};

pub fn item_on_ground(i: net::ItemOnGround) -> ItemOnGround {
    ItemOnGround {
        ground_id: i.ground_id,
        nameid: i.nameid,
        amount: i.amount,
        x: i.x as u16,
        y: i.y as u16,
        identified: i.identified,
        is_falling: i.is_falling,
        sub_x: i.sub_x as u8,
        sub_y: i.sub_y as u8,
    }
}

pub fn item_vanished(v: net::ItemVanished) -> ItemVanished {
    let reason = match net::ItemVanishReason::try_from(v.reason) {
        Ok(net::ItemVanishReason::PickedUp) => VanishReason::PickedUp,
        Ok(net::ItemVanishReason::Expired) | Err(_) => VanishReason::Expired,
    };
    ItemVanished {
        ground_id: v.ground_id,
        reason,
    }
}

pub fn pickup_result(r: net::PickupResult) -> PickupResult {
    let outcome = match net::PickupResultCode::try_from(r.result) {
        Ok(net::PickupResultCode::Ok) => PickupOutcome::Ok,
        Ok(net::PickupResultCode::TooFar) => PickupOutcome::TooFar,
        Ok(net::PickupResultCode::Overweight) => PickupOutcome::Overweight,
        Ok(net::PickupResultCode::InventoryFull) => PickupOutcome::InventoryFull,
        Ok(net::PickupResultCode::Gone) => PickupOutcome::Gone,
        Ok(net::PickupResultCode::Failed) | Err(_) => PickupOutcome::Failed,
    };
    PickupResult {
        ground_id: r.ground_id,
        outcome,
    }
}

pub fn pickup_body(c: &PickupRequested) -> Body {
    Body::PickupItemRequest(net::PickupItemRequest {
        ground_id: c.ground_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_on_ground_maps_and_casts_fields() {
        let mapped = item_on_ground(net::ItemOnGround {
            ground_id: 7,
            nameid: 501,
            amount: 3,
            x: 150,
            y: 120,
            identified: true,
            is_falling: true,
            sub_x: 9,
            sub_y: 12,
        });

        assert_eq!(mapped.ground_id, 7);
        assert_eq!(mapped.nameid, 501);
        assert_eq!(mapped.amount, 3);
        assert_eq!(mapped.x, 150u16);
        assert_eq!(mapped.y, 120u16);
        assert!(mapped.identified);
        assert!(mapped.is_falling);
        assert_eq!(mapped.sub_x, 9u8);
        assert_eq!(mapped.sub_y, 12u8);
    }

    #[test]
    fn item_vanished_maps_picked_up() {
        let mapped = item_vanished(net::ItemVanished {
            ground_id: 7,
            reason: net::ItemVanishReason::PickedUp as i32,
        });

        assert_eq!(mapped.ground_id, 7);
        assert_eq!(mapped.reason, VanishReason::PickedUp);
    }

    #[test]
    fn item_vanished_maps_expired() {
        let mapped = item_vanished(net::ItemVanished {
            ground_id: 7,
            reason: net::ItemVanishReason::Expired as i32,
        });

        assert_eq!(mapped.reason, VanishReason::Expired);
    }

    #[test]
    fn item_vanished_defaults_unknown_to_expired() {
        let mapped = item_vanished(net::ItemVanished {
            ground_id: 7,
            reason: 99,
        });

        assert_eq!(mapped.reason, VanishReason::Expired);
    }

    #[test]
    fn pickup_result_maps_ok() {
        let mapped = pickup_result(net::PickupResult {
            ground_id: 7,
            result: net::PickupResultCode::Ok as i32,
        });

        assert_eq!(mapped.ground_id, 7);
        assert_eq!(mapped.outcome, PickupOutcome::Ok);
    }

    #[test]
    fn pickup_result_maps_too_far() {
        let mapped = pickup_result(net::PickupResult {
            ground_id: 7,
            result: net::PickupResultCode::TooFar as i32,
        });

        assert_eq!(mapped.outcome, PickupOutcome::TooFar);
    }

    #[test]
    fn pickup_result_maps_inventory_full() {
        let mapped = pickup_result(net::PickupResult {
            ground_id: 7,
            result: net::PickupResultCode::InventoryFull as i32,
        });

        assert_eq!(mapped.outcome, PickupOutcome::InventoryFull);
    }

    #[test]
    fn pickup_result_defaults_unknown_to_failed() {
        let mapped = pickup_result(net::PickupResult {
            ground_id: 7,
            result: 99,
        });

        assert_eq!(mapped.outcome, PickupOutcome::Failed);
    }

    #[test]
    fn pickup_body_carries_ground_id() {
        let body = pickup_body(&PickupRequested { ground_id: 42 });
        match body {
            Body::PickupItemRequest(net::PickupItemRequest { ground_id }) => {
                assert_eq!(ground_id, 42u64)
            }
            other => panic!("expected Body::PickupItemRequest, got {other:?}"),
        }
    }
}
