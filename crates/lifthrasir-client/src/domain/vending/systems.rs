//! Vending systems: session reset, ordered ingress, intents, and projection.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use net_contract::commands::{
    CloseVendingRequested, ListVendingRequested, OpenVendingRequested, PurchaseVendingRequested,
};
use net_contract::events::{VendingIngress, ZoneDisconnected};
use net_contract::state::{ZoneSession, ZoneSessionGeneration};

use super::state::{
    RevealVendingSeller, VendingIntent, VendingNotice, VendingRefresh, VendingState,
};
use crate::domain::skill::SkillCastRequested;

/// The vending skill, used only as the ordinary activation command.
pub const VENDING_SKILL_ID: u32 = 41;

#[derive(SystemParam)]
pub struct VendingIntentWriters<'w> {
    skill_casts: MessageWriter<'w, SkillCastRequested>,
    reveals: MessageWriter<'w, RevealVendingSeller>,
    opens: MessageWriter<'w, OpenVendingRequested>,
    lists: MessageWriter<'w, ListVendingRequested>,
    purchases: MessageWriter<'w, PurchaseVendingRequested>,
    closes: MessageWriter<'w, CloseVendingRequested>,
}

/// Clears every session-owned value when the zone session or character changes.
pub(super) fn reset_vending_session(
    generation: Res<ZoneSessionGeneration>,
    session: Option<Res<ZoneSession>>,
    mut disconnected: MessageReader<ZoneDisconnected>,
    mut state: ResMut<VendingState>,
) {
    let char_id = session.as_deref().map_or(0, |session| session.char_id);
    let (seen_generation, seen_char_id) = state.session_gate();
    if seen_generation != *generation || seen_char_id != char_id {
        state.reset_session();
        state.set_session_gate(*generation, char_id);
    }
    if disconnected.read().count() != 0 {
        state.reset_session();
    }
}

/// Applies ordered authority before any local intent is validated.
pub(super) fn apply_vending_ingress(
    generation: Res<ZoneSessionGeneration>,
    mut ingress: MessageReader<VendingIngress>,
    mut state: ResMut<VendingState>,
) {
    for incoming in ingress.read() {
        if incoming.generation != *generation {
            continue;
        }
        state.apply(incoming);
    }
}

pub(super) fn mark_unconfirmed_mutations(time: Res<Time>, mut state: ResMut<VendingState>) {
    state.mark_unconfirmed(time.elapsed_secs());
}

pub(super) fn handle_vending_intents(
    time: Res<Time>,
    mut intents: MessageReader<VendingIntent>,
    mut state: ResMut<VendingState>,
    mut out: VendingIntentWriters,
) {
    let now = time.elapsed_secs();
    for intent in intents.read() {
        match intent {
            VendingIntent::EnterSeller => {
                if state.setup().is_some() || state.active_shop().is_some() {
                    out.reveals.write(RevealVendingSeller);
                } else {
                    out.skill_casts.write(SkillCastRequested {
                        skill_id: VENDING_SKILL_ID,
                    });
                }
            }
            VendingIntent::Browse {
                vendor_unit_id,
                shop_id,
            } => {
                if let Some(request_id) = state.begin_browse(*vendor_unit_id) {
                    out.lists.write(ListVendingRequested {
                        request_id,
                        vendor_unit_id: *vendor_unit_id,
                        expected_shop_id: Some(shop_id.clone()),
                    });
                }
            }
            VendingIntent::Open {
                setup_id,
                title,
                entries,
            } => {
                if let Some(request_id) = state.begin_open(setup_id, now) {
                    out.opens.write(OpenVendingRequested {
                        request_id,
                        setup_id: setup_id.clone(),
                        title: title.clone(),
                        entries: entries.clone(),
                    });
                } else {
                    debug!("ignoring vending open intent without a live setup grant");
                }
            }
            VendingIntent::Close { target } => {
                if let Some(request_id) = state.begin_close(target, now) {
                    out.closes.write(CloseVendingRequested {
                        request_id,
                        target: target.clone(),
                    });
                }
            }
            VendingIntent::Purchase {
                vendor_unit_id,
                shop_id,
                items,
            } => {
                if let Some(request_id) = state.begin_purchase(*vendor_unit_id, shop_id, now) {
                    out.purchases.write(PurchaseVendingRequested {
                        request_id,
                        vendor_unit_id: *vendor_unit_id,
                        shop_id: shop_id.clone(),
                        items: items.clone(),
                    });
                }
            }
            VendingIntent::Refresh => {
                if let Some(refresh) = state.begin_refresh() {
                    write_refresh(&mut out.lists, refresh);
                }
            }
        }
    }
}

/// Sends the read-only reload queued by a settled checkout.
pub(super) fn send_queued_refresh(
    mut state: ResMut<VendingState>,
    mut lists: MessageWriter<ListVendingRequested>,
) {
    if let Some(refresh) = state.take_refresh() {
        write_refresh(&mut lists, refresh);
    }
}

pub(super) fn project_vending_notices(
    mut state: ResMut<VendingState>,
    mut notices: MessageWriter<VendingNotice>,
) {
    for notice in state.take_notices() {
        notices.write(notice);
    }
}

fn write_refresh(lists: &mut MessageWriter<ListVendingRequested>, refresh: VendingRefresh) {
    lists.write(ListVendingRequested {
        request_id: refresh.request_id,
        vendor_unit_id: refresh.vendor_unit_id,
        expected_shop_id: Some(refresh.expected_shop_id),
    });
}
