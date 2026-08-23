use crate::proto::aesir::net;
use net_contract::events::ProductionResult;

pub fn production_result(r: net::ProductionResult) -> ProductionResult {
    ProductionResult {
        success: r.success,
        item_id: r.item_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_outcomes() {
        assert_eq!(
            production_result(net::ProductionResult {
                success: true,
                item_id: 1201,
            }),
            ProductionResult {
                success: true,
                item_id: 1201,
            }
        );
        assert_eq!(
            production_result(net::ProductionResult {
                success: false,
                item_id: 1201,
            }),
            ProductionResult {
                success: false,
                item_id: 1201,
            }
        );
    }
}
