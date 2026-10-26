//! Vending window: the seller's setup editor, its read-only monitor, and the
//! shared vending feedback line.
//!
//! Authority stays in [`VendingState`] and the server: this module only projects
//! that state into a window and turns affordances into [`VendingIntent`]s. It
//! never charges SP, moves items, or edits inventory/zeny.
//!
//! The chrome and the content host are spawned once. [`rebuild_content`] refills
//! the host only when its *structure* changes (mode, setup/shop identity, or the
//! set of drafted cart slots), so a focused quantity/price/title field survives
//! typing and every unrelated inventory, zeny, or sale update.

use std::collections::HashSet;

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::{
    RevealVendingSeller, VendingIntent, VendingNotice, VendingState,
};
use net_contract::dto::{VendingCloseTarget, VendingOffer, VendingSetup};
use net_contract::events::ChatHeard;

pub mod feedback;
pub mod scene;

#[cfg(test)]
mod tests;

use feedback::vending_notice_text;

/// Marks the window root so visibility, drag, and close act on this window only.
#[derive(Component, Default, Clone)]
pub struct VendingWindowRoot;

/// The draggable titlebar (the drag handle).
#[derive(Component, Default, Clone)]
pub struct VendingWindowTitlebar;

/// The padded body region under the titlebar.
#[derive(Component, Default, Clone)]
pub struct VendingWindowBody;

/// The titlebar label, updated in place as the mode changes.
#[derive(Component, Default, Clone)]
pub struct VendingTitle;

/// The region the rebuild system refills; typed fields live here.
#[derive(Component, Default, Clone)]
pub struct VendingContentHost;

/// The editor/monitor status line (SP already paid, or earned total).
#[derive(Component, Default, Clone)]
pub struct VendingModeNote;

/// The validation/rejection line under the content.
#[derive(Component, Default, Clone)]
pub struct VendingErrorLine;

/// The shop title field; read on submit, never mirrored per keystroke.
#[derive(Component, Default, Clone)]
pub struct VendingTitleField;

/// Which typed value a row field holds.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftFieldKind {
    #[default]
    Amount,
    Price,
}

/// One typed quantity or price field of a drafted listing.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendingDraftField {
    pub cart_index: u32,
    pub kind: DraftFieldKind,
}

/// The remove affordance of a drafted listing.
#[derive(Component, Default, Clone, Copy)]
pub struct VendingRowRemove {
    pub cart_index: u32,
}

/// One selectable sellable cart slot.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendingStockCell {
    pub cart_index: u32,
}

/// The submit affordance of the editor.
#[derive(Component, Default, Clone)]
pub struct VendingOpenButton;

/// The cancel affordance of the editor.
#[derive(Component, Default, Clone)]
pub struct VendingCancelButton;

/// One listed cart slot with its drafted quantity and price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftRow {
    pub cart_index: u32,
    pub amount: u32,
    pub price: u32,
}

/// The seller's unsent listings. Holds no authoritative inventory or zeny.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct SellerDraft {
    pub title: String,
    pub rows: Vec<DraftRow>,
}

impl SellerDraft {
    /// The listings as they will be submitted, in draft order.
    pub fn offers(&self) -> Vec<VendingOffer> {
        self.rows
            .iter()
            .map(|row| VendingOffer {
                cart_index: row.cart_index,
                amount: row.amount,
                price: row.price,
            })
            .collect()
    }

    pub fn row(&self, cart_index: u32) -> Option<&DraftRow> {
        self.rows.iter().find(|row| row.cart_index == cart_index)
    }

    /// Adds one sellable slot, or removes it when already drafted.
    pub fn toggle(&mut self, cart_index: u32, setup: &VendingSetup) {
        if let Some(position) = self
            .rows
            .iter()
            .position(|row| row.cart_index == cart_index)
        {
            self.rows.remove(position);
            return;
        }
        if self.rows.len() >= setup.max_slots as usize {
            return;
        }
        self.rows.push(DraftRow {
            cart_index,
            amount: 1,
            price: setup.min_unit_price,
        });
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Local pre-flight only: the server validates again at open time.
    pub fn validate(&self, setup: &VendingSetup, cart: &Cart) -> Result<(), String> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err("Enter a shop title.".to_string());
        }
        if title.chars().any(char::is_control) {
            return Err("The shop title cannot contain control characters.".to_string());
        }
        if title.len() > setup.max_title_bytes as usize {
            return Err(format!(
                "The shop title must fit in {} bytes.",
                setup.max_title_bytes
            ));
        }
        if self.rows.is_empty() {
            return Err("List at least one item.".to_string());
        }
        if self.rows.len() > setup.max_slots as usize {
            return Err(format!(
                "A shop can list at most {} items.",
                setup.max_slots
            ));
        }

        let mut seen = HashSet::new();
        for row in &self.rows {
            if !seen.insert(row.cart_index) {
                return Err("Each cart slot can be listed only once.".to_string());
            }
            if !setup.sellable_cart_indices.contains(&row.cart_index) {
                return Err("That cart slot cannot be sold.".to_string());
            }
            let Ok(index) = u16::try_from(row.cart_index) else {
                return Err("That cart slot does not exist.".to_string());
            };
            let Some(item) = cart.get(index) else {
                return Err("That cart slot no longer holds an item.".to_string());
            };
            if row.amount == 0 || row.amount > item.amount {
                return Err(format!("Quantity must be between 1 and {}.", item.amount));
            }
            if row.price < setup.min_unit_price {
                return Err(format!("Price must be at least {}.", setup.min_unit_price));
            }
            if row.price > setup.max_unit_price {
                return Err(format!("Price must be at most {}.", setup.max_unit_price));
            }
        }
        Ok(())
    }
}

/// Truncates to `max_bytes` on a character boundary, so the submitted title is
/// always valid UTF-8 and never longer than the negotiated limit.
pub fn clamp_title(title: &str, max_bytes: u32) -> String {
    let max_bytes = max_bytes as usize;
    if title.len() <= max_bytes {
        return title.to_string();
    }
    let mut end = 0;
    for (index, ch) in title.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    title[..end].to_string()
}

/// Which body the window is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Setup,
    Shop,
}

/// What the window currently shows, and whether the player wants it open.
#[derive(Resource, Debug, Default)]
pub struct VendingUi {
    visible: bool,
    mode: Option<Mode>,
    /// The structural identity of the rendered body, so unrelated updates do not
    /// respawn (and thereby unfocus) typed fields.
    structure: Option<(Mode, String, Vec<u32>)>,
    error: Option<String>,
    pending_close: bool,
}

impl VendingUi {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }
}

pub struct VendingWindowPlugin;

impl Plugin for VendingWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SellerDraft>()
            .init_resource::<VendingUi>()
            .add_systems(
                Update,
                (
                    reveal_on_intent,
                    sync_window_state,
                    rebuild_content,
                    sync_texts,
                    deliver_vending_feedback,
                    cancel_on_escape,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset_vending_ui);
    }
}

/// Spawns the vending window under the in-game HUD root.
pub fn spawn_vending_window(commands: &mut Commands, parent: Entity) {
    commands
        .spawn_scene(scene::window())
        .insert(DespawnOnExit(GameState::InGame))
        .insert(ChildOf(parent));
}

fn reset_vending_ui(mut draft: ResMut<SellerDraft>, mut ui: ResMut<VendingUi>) {
    draft.reset();
    *ui = VendingUi::default();
}

fn reveal_on_intent(mut reveals: MessageReader<RevealVendingSeller>, mut ui: ResMut<VendingUi>) {
    if reveals.read().count() != 0 {
        ui.visible = true;
    }
}

fn sync_window_state(
    state: Res<VendingState>,
    mut ui: ResMut<VendingUi>,
    mut draft: ResMut<SellerDraft>,
    mut roots: Query<(&mut Visibility, &Children), With<VendingWindowRoot>>,
    mut titles: Query<&mut Text, With<VendingTitle>>,
) {
    // A close that the domain no longer tracks has settled (or was rejected), so
    // the affordance becomes usable again instead of staying latched.
    if ui.pending_close && !state.close_pending() {
        ui.pending_close = false;
    }

    let mode = if state.setup().is_some() {
        Some(Mode::Setup)
    } else if state.active_shop().is_some() {
        Some(Mode::Shop)
    } else {
        None
    };

    // A new authoritative setup always opens the editor; leaving vending hides it.
    if ui.mode != mode {
        if mode == Some(Mode::Setup) {
            ui.visible = true;
            draft.reset();
        }
        ui.mode = mode;
        ui.error = None;
        ui.pending_close = false;
    }

    let Ok((mut visibility, _)) = roots.single_mut() else {
        return;
    };
    *visibility = if ui.visible && mode.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let label = match mode {
        Some(Mode::Setup) => "Set Up Shop",
        Some(Mode::Shop) => "My Shop",
        None => "Vending",
    };
    for mut title in &mut titles {
        if title.0 != label {
            title.0 = label.to_string();
        }
    }
}

fn state_identity(state: &VendingState) -> (Option<Mode>, String) {
    if let Some(setup) = state.setup() {
        return (Some(Mode::Setup), setup.setup_id.clone());
    }
    if let Some(shop) = state.active_shop() {
        return (Some(Mode::Shop), shop.shop_id.clone());
    }
    (None, String::new())
}

#[expect(
    clippy::too_many_arguments,
    reason = "one rebuild reads the state, draft, item names, typed fields and hosts"
)]
fn rebuild_content(
    mut commands: Commands,
    state: Res<VendingState>,
    cart: Res<Cart>,
    item_db: Res<ItemDb>,
    mut ui: ResMut<VendingUi>,
    mut draft: ResMut<SellerDraft>,
    fields: Query<(&VendingDraftField, &EditableText)>,
    title_fields: Query<&EditableText, With<VendingTitleField>>,
    hosts: Query<(Entity, Option<&Children>), With<VendingContentHost>>,
    new_hosts: Query<(), Added<VendingContentHost>>,
) {
    let (mode, identity) = state_identity(&state);
    let membership: Vec<u32> = draft.rows.iter().map(|row| row.cart_index).collect();
    let structure = mode.map(|mode| (mode, identity, membership));
    if structure == ui.structure && new_hosts.is_empty() {
        return;
    }
    ui.structure = structure;
    ui.pending_close = false;

    absorb_typed_fields(&fields, &title_fields, &mut draft);

    let Ok((host, children)) = hosts.single() else {
        return;
    };
    for children in children.into_iter() {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let error = ui.error.clone();
    match mode {
        Some(Mode::Setup) => {
            let Some(setup) = state.setup() else { return };
            let stock = stock_views(setup, &draft, &cart, &item_db);
            let rows = draft_views(&draft, &cart, &item_db);
            let note = format!("SP already paid: {}", setup.sp_spent);
            commands
                .spawn_scene(scene::editor(stock, rows, draft.title.clone(), note, error))
                .insert(ChildOf(host));
        }
        Some(Mode::Shop) => {
            let Some(shop) = state.active_shop() else {
                return;
            };
            let rows = shop_views(shop, &item_db);
            let sales = sale_views(state.recent_sales(), &item_db);
            let note = format!("Earned {}z", shop.total_earned);
            commands
                .spawn_scene(scene::monitor(rows, sales, note, error))
                .insert(ChildOf(host));
        }
        None => {}
    }
}

fn absorb_typed_fields(
    fields: &Query<(&VendingDraftField, &EditableText)>,
    title_fields: &Query<&EditableText, With<VendingTitleField>>,
    draft: &mut SellerDraft,
) {
    if let Ok(title) = title_fields.single() {
        draft.title = title.value().to_string();
    }
    for (field, text) in fields {
        let parsed = text.value().to_string().trim().parse::<u32>().ok();
        let Some(row) = draft
            .rows
            .iter_mut()
            .find(|row| row.cart_index == field.cart_index)
        else {
            continue;
        };
        if let Some(parsed) = parsed {
            match field.kind {
                DraftFieldKind::Amount => row.amount = parsed,
                DraftFieldKind::Price => row.price = parsed,
            }
        }
    }
}

fn stock_views(
    setup: &VendingSetup,
    draft: &SellerDraft,
    cart: &Cart,
    item_db: &ItemDb,
) -> Vec<scene::StockCellView> {
    setup
        .sellable_cart_indices
        .iter()
        .filter_map(|&cart_index| {
            let item = cart.get(u16::try_from(cart_index).ok()?)?;
            Some(scene::StockCellView {
                cart_index,
                label: item_label(item_db, item),
                amount: item.amount,
                selected: draft.row(cart_index).is_some(),
            })
        })
        .collect()
}

fn draft_views(draft: &SellerDraft, cart: &Cart, item_db: &ItemDb) -> Vec<scene::DraftRowView> {
    draft
        .rows
        .iter()
        .filter_map(|row| {
            let item = cart.get(u16::try_from(row.cart_index).ok()?)?;
            Some(scene::DraftRowView {
                cart_index: row.cart_index,
                label: item_label(item_db, item),
                stock: item.amount,
                amount_input: row.amount.to_string(),
                price_input: row.price.to_string(),
            })
        })
        .collect()
}

fn shop_views(
    shop: &net_contract::dto::VendingSnapshot,
    item_db: &ItemDb,
) -> Vec<scene::ShopRowView> {
    shop.items
        .iter()
        .map(|item| scene::ShopRowView {
            label: item_db
                .name(item.nameid, item.identified)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Item #{}", item.nameid)),
            amount: item.amount,
            price: item.price,
        })
        .collect()
}

fn sale_views(
    sales: &[lifthrasir_client::domain::vending::VendingSale],
    item_db: &ItemDb,
) -> Vec<scene::SaleRowView> {
    sales
        .iter()
        .map(|sale| scene::SaleRowView {
            label: item_db
                .name(sale.nameid, true)
                .map(str::to_string)
                .unwrap_or_else(|| format!("Item #{}", sale.nameid)),
            amount: sale.amount,
            zeny: sale.zeny_gained,
            buyer: sale.buyer_name.clone(),
        })
        .collect()
}

fn item_label(item_db: &ItemDb, item: &net_contract::dto::CartItem) -> String {
    item_db
        .name(item.nameid, item.identified)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Item #{}", item.nameid))
}

fn sync_texts(
    state: Res<VendingState>,
    ui: Res<VendingUi>,
    mut notes: Query<&mut Text, (With<VendingModeNote>, Without<VendingErrorLine>)>,
    mut errors: Query<&mut Text, With<VendingErrorLine>>,
) {
    let note = match (ui.mode, state.setup(), state.active_shop()) {
        (Some(Mode::Setup), Some(setup), _) => format!("SP already paid: {}", setup.sp_spent),
        (Some(Mode::Shop), _, Some(shop)) => format!("Earned {}z", shop.total_earned),
        _ => String::new(),
    };
    for mut text in &mut notes {
        if text.0 != note {
            text.0 = note.clone();
        }
    }
    let error = ui.error.clone().unwrap_or_default();
    for mut text in &mut errors {
        if text.0 != error {
            text.0 = error.clone();
        }
    }
}

pub fn on_stock_toggled(
    activate: On<Activate>,
    cells: Query<&VendingStockCell>,
    state: Res<VendingState>,
    mut draft: ResMut<SellerDraft>,
) {
    let Ok(cell) = cells.get(activate.entity) else {
        return;
    };
    let Some(setup) = state.setup() else {
        return;
    };
    draft.toggle(cell.cart_index, setup);
}

pub fn on_row_removed(
    activate: On<Activate>,
    removals: Query<&VendingRowRemove>,
    state: Res<VendingState>,
    mut draft: ResMut<SellerDraft>,
) {
    let Ok(removal) = removals.get(activate.entity) else {
        return;
    };
    let Some(setup) = state.setup() else {
        return;
    };
    draft.toggle(removal.cart_index, setup);
}

#[expect(
    clippy::too_many_arguments,
    reason = "submitting reads typed fields, the draft, the cart and the setup"
)]
pub fn on_open_shop(
    _: On<Activate>,
    state: Res<VendingState>,
    cart: Res<Cart>,
    mut draft: ResMut<SellerDraft>,
    fields: Query<(&VendingDraftField, &EditableText)>,
    title_fields: Query<&EditableText, With<VendingTitleField>>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
    mut focus: ResMut<InputFocus>,
) {
    if ui.pending_close || state.open_pending() {
        return;
    }
    let Some(setup) = state.setup().cloned() else {
        return;
    };
    absorb_typed_fields(&fields, &title_fields, &mut draft);
    if let Ok(title) = title_fields.single() {
        draft.title = clamp_title(&title.value().to_string(), setup.max_title_bytes);
    }

    if let Err(error) = draft.validate(&setup, &cart) {
        ui.set_error(Some(error));
        return;
    }
    ui.set_error(None);
    focus.clear();
    intents.write(VendingIntent::Open {
        setup_id: setup.setup_id,
        title: draft.title.trim().to_string(),
        entries: draft.offers(),
    });
}

pub fn on_cancel_setup(
    _: On<Activate>,
    state: Res<VendingState>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(setup) = state.setup() else {
        return;
    };
    if ui.pending_close {
        return;
    }
    ui.pending_close = true;
    focus.clear();
    intents.write(VendingIntent::Close {
        target: VendingCloseTarget::Setup(setup.setup_id.clone()),
    });
}

pub fn on_close_shop(
    _: On<Activate>,
    state: Res<VendingState>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
) {
    let Some(shop) = state.active_shop() else {
        return;
    };
    if ui.pending_close {
        return;
    }
    ui.pending_close = true;
    intents.write(VendingIntent::Close {
        target: VendingCloseTarget::Shop(shop.shop_id.clone()),
    });
}

/// The titlebar close button: cancels preparation, hides an active shop (the
/// server keeps selling), and never closes a shop implicitly.
pub fn on_close_button(
    _: On<Activate>,
    state: Res<VendingState>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
) {
    if let Some(setup) = state.setup() {
        if !ui.pending_close {
            ui.pending_close = true;
            intents.write(VendingIntent::Close {
                target: VendingCloseTarget::Setup(setup.setup_id.clone()),
            });
        }
        return;
    }
    ui.hide();
}

fn cancel_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<VendingState>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
) {
    if !keys.just_pressed(KeyCode::Escape) || !ui.visible {
        return;
    }
    if let Some(setup) = state.setup() {
        if !ui.pending_close {
            ui.pending_close = true;
            intents.write(VendingIntent::Close {
                target: VendingCloseTarget::Setup(setup.setup_id.clone()),
            });
        }
        return;
    }
    ui.hide();
}

fn deliver_vending_feedback(
    mut notices: MessageReader<VendingNotice>,
    mut chat: MessageWriter<ChatHeard>,
) {
    for notice in notices.read() {
        chat.write(ChatHeard {
            gid: 0,
            message: vending_notice_text(notice),
        });
    }
}
