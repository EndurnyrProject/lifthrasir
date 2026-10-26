//! Vending drag-and-drop: what a drag carries, where it may land, and the one
//! code path both a drop and a double-click go through.
//!
//! Follows the hotbar's pattern: the payload lives in [`VendingDrag`] from
//! `PointerDragStart` until the button is released, so a server update that
//! redraws the source panel mid-drag cannot lose the drop. Every action is
//! re-validated against live state when applied.

use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::VendingState;

use super::buyer::BuyerBasket;
use super::cells::item_icon;
use super::{SellerDraft, VendingUi};
use crate::worldspace::viewport_to_ui;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);
const GHOST_SIZE: f32 = 34.0;
const GHOST_Z: i32 = 2000;
const GHOST_ALPHA: f32 = 0.85;
/// Drop-zone border while a compatible drag is live, and at rest.
const ZONE_ACTIVE: Color = lifthrasir_ui::theme::EMERALD;
const ZONE_IDLE: Color = lifthrasir_ui::theme::GOLD_FAINT;

/// Which surface an item is dragged from.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    /// A sellable cart slot in the seller's stock panel.
    #[default]
    Stock,
    /// A drafted listing row in the setup editor.
    Draft,
    /// A listing in the vendor panel.
    Listing,
    /// A staged line in the buyer's basket.
    Basket,
    /// A listing in the seller's own open shop; inspect-only, never dragged.
    Shop,
}

/// An item on a vending surface: its surface and index (a neutral cart index
/// for stock/draft, the opaque listing index for listing/basket/shop). Drag
/// sources and the right-click inspector both resolve items through it.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DragSource {
    pub kind: DragKind,
    pub index: u32,
}

/// The live drag payload, if any.
#[derive(Resource, Default, Debug)]
pub struct VendingDrag {
    pub payload: Option<DragSource>,
}

/// A panel that accepts drops.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    #[default]
    SetupListings,
    CartStock,
    Basket,
    VendorListing,
}

/// The local change a drop or double-click asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropAction {
    List(u32),
    Unlist(u32),
    AddToBasket(u32),
    RemoveFromBasket(u32),
}

pub fn drop_action(zone: DropZone, payload: DragSource) -> Option<DropAction> {
    match (zone, payload.kind) {
        (DropZone::SetupListings, DragKind::Stock) => Some(DropAction::List(payload.index)),
        (DropZone::CartStock, DragKind::Draft) => Some(DropAction::Unlist(payload.index)),
        (DropZone::Basket, DragKind::Listing) => Some(DropAction::AddToBasket(payload.index)),
        (DropZone::VendorListing, DragKind::Basket) => {
            Some(DropAction::RemoveFromBasket(payload.index))
        }
        _ => None,
    }
}

/// The double-click equivalent of dragging a grid cell to its natural target.
/// Rows hold typed fields and buttons, so they have no double-click action.
pub fn shortcut_action(source: DragSource) -> Option<DropAction> {
    match source.kind {
        DragKind::Stock => Some(DropAction::List(source.index)),
        DragKind::Listing => Some(DropAction::AddToBasket(source.index)),
        DragKind::Draft | DragKind::Basket | DragKind::Shop => None,
    }
}

/// Everything a drop may change. Only local staging: nothing here reaches the
/// server, which still validates the submitted shop or purchase.
#[derive(SystemParam)]
pub struct DropTargets<'w> {
    state: Res<'w, VendingState>,
    cart: Res<'w, Cart>,
    draft: ResMut<'w, SellerDraft>,
    basket: ResMut<'w, BuyerBasket>,
    ui: ResMut<'w, VendingUi>,
}

impl DropTargets<'_> {
    /// Applies one action against live state; a refusal lands on the error line.
    pub fn apply(&mut self, action: DropAction) {
        match self.try_apply(action) {
            Ok(()) => self.ui.set_error(None),
            Err(error) => self.ui.set_error(Some(error)),
        }
    }

    fn try_apply(&mut self, action: DropAction) -> Result<(), String> {
        match action {
            DropAction::List(cart_index) => {
                let setup = self.state.setup().ok_or("The shop setup has ended.")?;
                let stock = u16::try_from(cart_index)
                    .ok()
                    .and_then(|slot| self.cart.get(slot))
                    .map(|item| item.amount)
                    .ok_or("That cart slot no longer holds an item.")?;
                if self.draft.add(cart_index, stock, setup)? {
                    self.ui.focus_price = Some(cart_index);
                }
            }
            DropAction::Unlist(cart_index) => {
                self.draft.remove(cart_index);
            }
            DropAction::AddToBasket(index) => {
                if self.state.purchase_pending() {
                    return Ok(());
                }
                let shop = self.state.browse().ok_or("That shop is no longer open.")?;
                self.basket.step(shop, index, true);
            }
            DropAction::RemoveFromBasket(index) => {
                if self.state.purchase_pending() {
                    return Ok(());
                }
                if let Some(shop) = self.state.browse() {
                    self.basket.set_quantity(shop, index, 0);
                }
            }
        }
        Ok(())
    }
}

/// Arms the drag with the pressed item.
pub fn on_drag_start(
    mut drag: On<PointerDragStart>,
    sources: Query<&DragSource>,
    mut state: ResMut<VendingDrag>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Ok(source) = sources.get(drag.entity) else {
        return;
    };
    drag.propagate(false);
    state.payload = Some(*source);
}

/// Applies a compatible drop. Drops bubble up from the hit row or cell, so this
/// acts on the zone it is attached to and stops there.
pub fn on_zone_drop(
    mut drop: On<PointerDragDrop>,
    zones: Query<&DropZone>,
    mut state: ResMut<VendingDrag>,
    mut targets: DropTargets,
) {
    let Ok(zone) = zones.get(drop.entity) else {
        return;
    };
    drop.propagate(false);
    let Some(action) = state.payload.take().and_then(|p| drop_action(*zone, p)) else {
        return;
    };
    targets.apply(action);
}

/// Last primary click on a vending cell, for double-click detection.
#[derive(Resource, Default)]
pub struct LastVendingClick {
    source: Option<DragSource>,
    at: Duration,
}

/// A double-click on a grid cell does what dragging it to its target would.
pub fn on_cell_click(
    click: On<PointerClick>,
    sources: Query<&DragSource>,
    time: Res<Time>,
    mut last: ResMut<LastVendingClick>,
    mut targets: DropTargets,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(source) = sources.get(click.entity) else {
        return;
    };
    let now = time.elapsed();
    let double = last.source == Some(*source) && now.saturating_sub(last.at) <= DOUBLE_CLICK;
    if !double {
        *last = LastVendingClick {
            source: Some(*source),
            at: now,
        };
        return;
    }
    last.source = None;
    if let Some(action) = shortcut_action(*source) {
        targets.apply(action);
    }
}

/// Ends the drag once the primary button is up. Picking dispatches drops in
/// `PreUpdate`, so this also covers a drag whose source was despawned mid-drag
/// and never saw its own `PointerDragEnd`.
pub fn end_drag_on_release(
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    mut state: ResMut<VendingDrag>,
) {
    let released = buttons.is_none_or(|buttons| !buttons.pressed(MouseButton::Left));
    if state.payload.is_some() && released {
        state.payload = None;
    }
}

/// Lights up every zone the live drag could land in.
pub fn highlight_drop_zones(
    state: Res<VendingDrag>,
    mut zones: Query<(&DropZone, &mut BorderColor)>,
) {
    for (zone, mut border) in &mut zones {
        let active = state
            .payload
            .is_some_and(|payload| drop_action(*zone, payload).is_some());
        let target = BorderColor::all(if active { ZONE_ACTIVE } else { ZONE_IDLE });
        if *border != target {
            *border = target;
        }
    }
}

/// The icon a dragged item shows on the cursor.
pub fn payload_icon(
    payload: DragSource,
    state: &VendingState,
    cart: &Cart,
    item_db: &ItemDb,
) -> Option<String> {
    match payload.kind {
        DragKind::Stock | DragKind::Draft => {
            let item = cart.get(u16::try_from(payload.index).ok()?)?;
            item_icon(item_db, item.nameid, item.identified)
        }
        DragKind::Shop => None,
        DragKind::Listing | DragKind::Basket => {
            let listing = state
                .browse()?
                .items
                .iter()
                .find(|listing| listing.index == payload.index)?;
            item_icon(item_db, listing.nameid, listing.identified)
        }
    }
}

#[derive(Component)]
pub struct VendingDragGhost;

#[derive(SystemParam)]
pub struct GhostLookup<'w> {
    state: Res<'w, VendingState>,
    cart: Res<'w, Cart>,
    item_db: Res<'w, ItemDb>,
    asset_server: Res<'w, AssetServer>,
}

/// Keeps a translucent icon under the cursor while a drag is live.
pub fn update_vending_ghost(
    mut commands: Commands,
    state: Res<VendingDrag>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    lookup: GhostLookup,
    mut ghosts: Query<(Entity, &mut Node), With<VendingDragGhost>>,
) {
    let ghost = ghosts.single_mut().ok();
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let icon = state
        .payload
        .and_then(|payload| payload_icon(payload, &lookup.state, &lookup.cart, &lookup.item_db));
    let (Some(cursor), Some(icon)) = (cursor, icon) else {
        if let Some((entity, _)) = ghost {
            commands.entity(entity).despawn();
        }
        return;
    };
    let cursor = viewport_to_ui(cursor, &ui_scale);
    let left = Val::Px(cursor.x - GHOST_SIZE / 2.0);
    let top = Val::Px(cursor.y - GHOST_SIZE / 2.0);
    if let Some((_, mut node)) = ghost {
        node.left = left;
        node.top = top;
        return;
    }
    commands.spawn((
        VendingDragGhost,
        ImageNode {
            image: lookup.asset_server.load(icon),
            color: Color::WHITE.with_alpha(GHOST_ALPHA),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left,
            top,
            width: Val::Px(GHOST_SIZE),
            height: Val::Px(GHOST_SIZE),
            ..default()
        },
        GlobalZIndex(GHOST_Z),
        Pickable::IGNORE,
        DespawnOnExit(GameState::InGame),
    ));
}
