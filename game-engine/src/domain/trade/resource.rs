use bevy::prelude::*;
use net_contract::events::{TradeOfferUpdated, zone::ZoneInventoryItem};

pub const MAX_OFFER_SLOTS: usize = 10;

/// The open trade as last reported by the server.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenTrade {
    pub partner_char_id: u32,
    pub partner_name: String,
    pub own: Vec<ZoneInventoryItem>,
    pub partner: Vec<ZoneInventoryItem>,
    pub own_zeny: u64,
    pub partner_zeny: u64,
    pub own_locked: bool,
    pub partner_locked: bool,
    pub confirm_sent: bool,
}

/// The current server-authoritative player trade, if any.
#[derive(Resource, Default)]
pub struct TradeSession {
    open: Option<OpenTrade>,
}

impl TradeSession {
    pub fn open(&mut self, partner_char_id: u32, partner_name: String) {
        self.open = Some(OpenTrade {
            partner_char_id,
            partner_name,
            own: vec![],
            partner: vec![],
            own_zeny: 0,
            partner_zeny: 0,
            own_locked: false,
            partner_locked: false,
            confirm_sent: false,
        });
    }

    /// Apply an authoritative offer snapshot, ignoring updates while closed.
    pub fn apply_offer(&mut self, update: &TradeOfferUpdated) {
        let Some(open) = &mut self.open else {
            warn!("ignoring trade offer update while trade is closed");
            return;
        };
        open.own.clone_from(&update.own);
        open.partner.clone_from(&update.partner);
        open.own_zeny = update.own_zeny;
        open.partner_zeny = update.partner_zeny;
        open.own_locked = update.own_locked;
        open.partner_locked = update.partner_locked;
    }

    pub fn mark_confirm_sent(&mut self) {
        if let Some(open) = &mut self.open {
            open.confirm_sent = true;
        }
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    pub fn reset(&mut self) {
        self.close();
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn current(&self) -> Option<&OpenTrade> {
        self.open.as_ref()
    }

    pub fn can_confirm(&self) -> bool {
        self.current()
            .is_some_and(|open| open.own_locked && open.partner_locked && !open.confirm_sent)
    }

    pub fn is_offered(&self, index: u32) -> bool {
        self.current()
            .is_some_and(|open| open.own.iter().any(|item| item.index == index))
    }

    pub fn offer_full(&self) -> bool {
        self.current()
            .is_some_and(|open| open.own.len() >= MAX_OFFER_SLOTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(index: u32) -> ZoneInventoryItem {
        ZoneInventoryItem {
            index,
            nameid: 501,
            type_: 0,
            amount: 2,
            location: 0,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            expire_time: 0,
            bound: 0,
            favorite: false,
            look: 0,
        }
    }

    #[test]
    fn trade_opens_empty_and_only_confirms_after_both_locks() {
        let mut session = TradeSession::default();
        session.open(42, "Alice".into());
        let open = session.current().unwrap();
        assert_eq!(
            (open.partner_char_id, open.partner_name.as_str()),
            (42, "Alice")
        );
        assert!(open.own.is_empty() && open.partner.is_empty());
        assert_eq!((open.own_zeny, open.partner_zeny), (0, 0));
        assert!(!open.own_locked && !open.partner_locked && !open.confirm_sent);
        assert!(!session.can_confirm());
        session.apply_offer(&TradeOfferUpdated {
            own: vec![item(7)],
            partner: vec![item(0)],
            own_zeny: 5,
            partner_zeny: 10,
            own_locked: true,
            partner_locked: false,
        });
        assert!(!session.can_confirm());
        session.apply_offer(&TradeOfferUpdated {
            own: vec![item(7)],
            partner: vec![item(0)],
            own_zeny: 5,
            partner_zeny: 10,
            own_locked: true,
            partner_locked: true,
        });
        assert!(session.can_confirm());
        let open = session.current().unwrap();
        assert_eq!(
            (open.own.clone(), open.partner.clone()),
            (vec![item(7)], vec![item(0)])
        );
        assert_eq!((open.own_zeny, open.partner_zeny), (5, 10));
        session.mark_confirm_sent();
        assert!(!session.can_confirm());
        session.close();
        assert!(!session.is_open());
        assert!(session.current().is_none());
    }

    #[test]
    fn closed_offers_are_ignored_and_slots_cap_at_ten() {
        let mut session = TradeSession::default();
        let update = TradeOfferUpdated {
            own: (1..=9).map(item).collect(),
            partner: vec![],
            own_zeny: 0,
            partner_zeny: 0,
            own_locked: false,
            partner_locked: false,
        };
        session.apply_offer(&update);
        assert!(!session.is_open());
        session.open(1, "B".into());
        session.apply_offer(&update);
        assert!(session.is_offered(9));
        assert!(!session.is_offered(10));
        assert!(!session.offer_full());
        let mut full = update;
        full.own.push(item(10));
        session.apply_offer(&full);
        assert!(session.offer_full());
        session.reset();
        assert!(!session.is_open());
        assert!(!session.offer_full());
    }

    #[test]
    fn confirmation_requires_both_locked_and_unsent() {
        for own_locked in [false, true] {
            for partner_locked in [false, true] {
                for confirm_sent in [false, true] {
                    let mut session = TradeSession::default();
                    session.open(1, "B".into());
                    session.apply_offer(&TradeOfferUpdated {
                        own: vec![],
                        partner: vec![],
                        own_zeny: 0,
                        partner_zeny: 0,
                        own_locked,
                        partner_locked,
                    });
                    if confirm_sent {
                        session.mark_confirm_sent();
                    }
                    assert_eq!(
                        session.can_confirm(),
                        own_locked && partner_locked && !confirm_sent
                    );
                }
            }
        }
    }
}
