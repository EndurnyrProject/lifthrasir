//! Vending window: the seller's setup editor, its read-only monitor, the
//! buyer's basket, and the shared vending feedback line.
//!
//! The window has two panels. The left one ([`source`]) shows what can be
//! dragged in: the cart's sellable stock during setup, the vendor's listings
//! while browsing. The right one is the editor, monitor or basket. Items move
//! between them by drag-and-drop or double-click ([`drag`]).
//!
//! Authority stays in [`VendingState`] and the server: this module only projects
//! that state into a window and turns affordances into [`VendingIntent`]s. It
//! never charges SP, moves items, or edits inventory/zeny.
//!
//! The chrome and the content host are spawned once. [`rebuild_content`] refills
//! the host only when its *structure* changes (mode, setup/shop identity, or the
//! set of drafted cart slots), so a focused quantity/price/title field survives
//! typing and every unrelated inventory, zeny, or sale update.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::vending::{
    RevealVendingSeller, VendingIntent, VendingNotice, VendingState, VendingSystems,
};
use net_contract::dto::{VendingCloseTarget, VendingListOutcome, VendingPurchaseOutcome};
use net_contract::events::ChatHeard;

pub mod buyer;
pub mod cells;
pub mod drag;
pub mod feedback;
pub mod scene;
pub mod seller;
pub mod source;

#[cfg(test)]
mod tests;

use buyer::BuyerBasket;
use cells::{ItemCellView, item_icon, item_name};
use feedback::vending_notice_text;
pub use seller::{DraftRow, SellerDraft, clamp_title};

use crate::widgets::info_modal::{InfoContent, ShowInfoModal, view};

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

/// The submit affordance of the editor.
#[derive(Component, Default, Clone)]
pub struct VendingOpenButton;

/// One basket row, keyed by its opaque listing index.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendingBasketRow {
    pub index: u32,
}

/// The remove affordance of a basket row.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasketRemove {
    pub index: u32,
}

/// One quantity stepper of a basket row.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasketStep {
    pub index: u32,
    pub up: bool,
}

/// The cancel affordance of the editor.
#[derive(Component, Default, Clone)]
pub struct VendingCancelButton;

/// Body identity: what is shown, which shop or setup it belongs to, and the rows
/// it renders (cart slots for the editor, staged quantities for the basket).
type Structure = (Mode, String, Vec<(u32, u32)>);

/// Which body the window is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Setup,
    Shop,
    Browse,
}

/// What the window currently shows, and whether the player wants it open.
#[derive(Resource, Debug, Default)]
pub struct VendingUi {
    visible: bool,
    mode: Option<Mode>,
    /// The structural identity of the rendered body, so unrelated updates do not
    /// respawn (and thereby unfocus) typed fields.
    structure: Option<Structure>,
    /// The browse snapshot the basket was last reconciled against.
    last_browse: Option<net_contract::dto::VendingSnapshot>,
    error: Option<String>,
    pending_close: bool,
    /// The left panel's rendered identity; see [`source::SourceStructure`].
    source_structure: Option<source::SourceStructure>,
    /// The item under the pointer in the left panel, for its info line.
    hovered: Option<drag::DragSource>,
    /// A freshly listed row whose price field should take focus once spawned.
    focus_price: Option<u32>,
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
            .init_resource::<BuyerBasket>()
            .init_resource::<VendingUi>()
            .init_resource::<drag::VendingDrag>()
            .init_resource::<drag::LastVendingClick>()
            .add_systems(
                Update,
                (
                    (
                        reveal_on_intent,
                        sync_window_state,
                        sync_browse_basket,
                        settle_buyer_basket,
                        rebuild_content,
                        source::rebuild_source,
                        focus_new_price_field,
                    )
                        .chain(),
                    (
                        sync_texts,
                        source::sync_source_info,
                        drag::highlight_drop_zones,
                        drag::end_drag_on_release,
                        drag::update_vending_ghost,
                        deliver_vending_feedback,
                        cancel_on_escape,
                    )
                        .chain(),
                )
                    .chain()
                    .after(VendingSystems::Projection)
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

fn reset_vending_ui(
    mut draft: ResMut<SellerDraft>,
    mut basket: ResMut<BuyerBasket>,
    mut ui: ResMut<VendingUi>,
) {
    draft.reset();
    basket.clear();
    *ui = VendingUi::default();
}

/// Shows the window for a seller reveal or a freshly loaded vendor shop.
///
/// Browse results are the only list notices (owner reconciliation emits none),
/// so a successful one means the player asked to see that shop.
fn reveal_on_intent(
    mut reveals: MessageReader<RevealVendingSeller>,
    mut notices: MessageReader<VendingNotice>,
    mut ui: ResMut<VendingUi>,
) {
    let seller = reveals.read().count() != 0;
    let browse_loaded = notices.read().any(|notice| {
        matches!(
            notice,
            VendingNotice::List {
                outcome: VendingListOutcome::Success,
                ..
            }
        )
    });
    if seller || browse_loaded {
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

    let mode = current_mode(&state);

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
        Some(Mode::Browse) => "Vendor",
        None => "Vending",
    };
    for mut title in &mut titles {
        if title.0 != label {
            title.0 = label.to_string();
        }
    }
}

/// Setup preparation wins over everything: the seller editor is authoritative.
fn current_mode(state: &VendingState) -> Option<Mode> {
    if state.setup().is_some() {
        return Some(Mode::Setup);
    }
    if state.browse().is_some() {
        return Some(Mode::Browse);
    }
    state.active_shop().map(|_| Mode::Shop)
}

fn state_identity(state: &VendingState) -> (Option<Mode>, String) {
    match current_mode(state) {
        Some(Mode::Setup) => (
            Some(Mode::Setup),
            state
                .setup()
                .map_or_else(String::new, |setup| setup.setup_id.clone()),
        ),
        Some(Mode::Shop) => (
            Some(Mode::Shop),
            state
                .active_shop()
                .map_or_else(String::new, |shop| shop.shop_id.clone()),
        ),
        Some(Mode::Browse) => (
            Some(Mode::Browse),
            state
                .browse()
                .map_or_else(String::new, |shop| shop.shop_id.clone()),
        ),
        None => (None, String::new()),
    }
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
    basket: Res<BuyerBasket>,
    statuses: Query<&CharacterStatus, With<LocalPlayer>>,
    fields: Query<(&VendingDraftField, &EditableText)>,
    title_fields: Query<&EditableText, With<VendingTitleField>>,
    hosts: Query<(Entity, Option<&Children>), With<VendingContentHost>>,
    new_hosts: Query<(), Added<VendingContentHost>>,
) {
    let (mode, identity) = state_identity(&state);
    let membership: Vec<(u32, u32)> = match mode {
        Some(Mode::Setup) => draft.rows.iter().map(|row| (row.cart_index, 0)).collect(),
        Some(Mode::Browse) => basket
            .lines()
            .iter()
            .map(|line| (line.index, line.amount))
            .collect(),
        _ => Vec::new(),
    };
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
            let rows = draft_views(&draft, &cart, &item_db);
            let note = format!("SP already paid: {}", setup.sp_spent);
            let slots = format!("{} / {} slots", draft.rows.len(), setup.max_slots);
            commands
                .spawn_scene(scene::editor(rows, draft.title.clone(), note, slots, error))
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
        Some(Mode::Browse) => {
            let Some(shop) = state.browse() else { return };
            let rows = basket_views(shop, &basket, &item_db);
            let total = basket.total(shop);
            let note = match total {
                Some(total) => format!("Total {}z", total),
                None => "Total -".to_string(),
            };
            let zeny = u64::from(statuses.iter().next().map_or(0, |status| status.zeny));
            let guidance = total.and_then(|total| {
                (total > zeny).then(|| format!("You need {} more zeny.", total - zeny))
            });
            commands
                .spawn_scene(scene::buyer(rows, note, guidance, error))
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

fn draft_views(draft: &SellerDraft, cart: &Cart, item_db: &ItemDb) -> Vec<scene::DraftRowView> {
    draft
        .rows
        .iter()
        .filter_map(|row| {
            let item = cart.get(u16::try_from(row.cart_index).ok()?)?;
            Some(scene::DraftRowView {
                cart_index: row.cart_index,
                cell: ItemCellView {
                    icon: item_icon(item_db, item.nameid, item.identified),
                    refine: item.refine,
                    ..default()
                },
                label: item_name(item_db, item.nameid, item.identified),
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
            index: item.index,
            cell: ItemCellView {
                icon: item_icon(item_db, item.nameid, item.identified),
                refine: item.refine,
                ..default()
            },
            label: item_name(item_db, item.nameid, item.identified),
            amount: item.amount,
            price: item.price,
        })
        .collect()
}

/// One row per staged basket line, in staging order.
fn basket_views(
    shop: &net_contract::dto::VendingSnapshot,
    basket: &BuyerBasket,
    item_db: &ItemDb,
) -> Vec<scene::BasketRowView> {
    basket
        .lines()
        .iter()
        .filter_map(|line| {
            let listing = shop.items.iter().find(|item| item.index == line.index)?;
            Some(scene::BasketRowView {
                index: line.index,
                cell: ItemCellView {
                    icon: item_icon(item_db, listing.nameid, listing.identified),
                    refine: listing.refine,
                    ..default()
                },
                label: item_name(item_db, listing.nameid, listing.identified),
                price: listing.price,
                quantity: line.amount,
                line_total: u64::from(listing.price) * u64::from(line.amount),
            })
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
            cell: ItemCellView {
                icon: item_icon(item_db, sale.nameid, true),
                ..default()
            },
            label: item_name(item_db, sale.nameid, true),
            amount: sale.amount,
            zeny: sale.zeny_gained,
            buyer: sale.buyer_name.clone(),
        })
        .collect()
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

pub fn on_row_removed(
    activate: On<Activate>,
    removals: Query<&VendingRowRemove>,
    mut draft: ResMut<SellerDraft>,
) {
    if let Ok(removal) = removals.get(activate.entity) {
        draft.remove(removal.cart_index);
    }
}

/// Focuses the price field of a row just listed by a drop or double-click.
fn focus_new_price_field(
    mut ui: ResMut<VendingUi>,
    fields: Query<(Entity, &VendingDraftField), Added<VendingDraftField>>,
    mut focus: ResMut<InputFocus>,
) {
    let Some(cart_index) = ui.focus_price else {
        return;
    };
    let price = fields
        .iter()
        .find(|(_, field)| field.cart_index == cart_index && field.kind == DraftFieldKind::Price);
    if let Some((entity, _)) = price {
        focus.set(entity, FocusCause::Navigated);
        ui.focus_price = None;
    }
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

/// Binds the basket to the shop being browsed and re-applies it to a refreshed
/// snapshot, explaining every dropped, re-priced, or clamped row.
fn sync_browse_basket(
    state: Res<VendingState>,
    mut basket: ResMut<BuyerBasket>,
    mut ui: ResMut<VendingUi>,
) {
    let Some(current) = state.browse().cloned() else {
        ui.last_browse = None;
        return;
    };
    if ui.last_browse.as_ref() == Some(&current) {
        return;
    }
    let previous = ui.last_browse.take();
    let same_shop = previous.as_ref().is_some_and(|previous| {
        previous.shop_id == current.shop_id && previous.vendor_unit_id == current.vendor_unit_id
    });
    if !state.purchase_pending() {
        basket.bind(current.vendor_unit_id, &current.shop_id);
    }
    if let Some(previous) = previous.filter(|_| same_shop && !state.purchase_pending()) {
        let messages = basket.reconcile(&previous, &current);
        if !messages.is_empty() {
            ui.set_error(Some(messages.join(" ")));
        }
    }
    ui.last_browse = Some(current);
}

/// A settled checkout clears the basket it bought from.
fn settle_buyer_basket(mut notices: MessageReader<VendingNotice>, mut basket: ResMut<BuyerBasket>) {
    for notice in notices.read() {
        if let VendingNotice::Purchase {
            outcome: VendingPurchaseOutcome::Success,
            ..
        } = notice
        {
            basket.clear();
        }
    }
}

/// The details view for a vending item: a cart item with its cards and
/// options, or a vendor/own-shop listing with its price and creator.
pub fn inspect_view(
    source: drag::DragSource,
    state: &VendingState,
    cart: &Cart,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> Option<view::ItemInfoView> {
    let listing_in = |shop: Option<&net_contract::dto::VendingSnapshot>| {
        shop?
            .items
            .iter()
            .find(|item| item.index == source.index)
            .map(|item| view::vending_item_view(item, item_db, option_db))
    };
    match source.kind {
        drag::DragKind::Stock | drag::DragKind::Draft => {
            let item = cart.get(u16::try_from(source.index).ok()?)?;
            Some(view::cart_item_view(item, item_db, option_db))
        }
        drag::DragKind::Listing | drag::DragKind::Basket => listing_in(state.browse()),
        drag::DragKind::Shop => listing_in(state.active_shop()),
    }
}

/// A secondary click on any vending item opens its details. Info only: vending
/// items are never in the bag, so no equip/use/compound action applies.
pub fn on_item_inspect(
    click: On<PointerClick>,
    sources: Query<&drag::DragSource>,
    state: Res<VendingState>,
    cart: Res<Cart>,
    item_db: Res<ItemDb>,
    option_db: Option<Res<RandomOptionDb>>,
    mut info: MessageWriter<ShowInfoModal>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }
    let Ok(source) = sources.get(click.entity) else {
        return;
    };
    let Some(view) = inspect_view(*source, &state, &cart, &item_db, option_db.as_deref()) else {
        return;
    };
    info.write(ShowInfoModal {
        content: InfoContent::Item { view, action: None },
    });
}

/// The basket row's remove button.
pub fn on_basket_remove(
    activate: On<Activate>,
    removals: Query<&BasketRemove>,
    state: Res<VendingState>,
    mut basket: ResMut<BuyerBasket>,
) {
    let Ok(removal) = removals.get(activate.entity) else {
        return;
    };
    if state.purchase_pending() {
        return;
    }
    if let Some(shop) = state.browse() {
        basket.set_quantity(shop, removal.index, 0);
    }
}

pub fn on_basket_step(
    activate: On<Activate>,
    steps: Query<&BasketStep>,
    state: Res<VendingState>,
    mut basket: ResMut<BuyerBasket>,
) {
    let Ok(step) = steps.get(activate.entity) else {
        return;
    };
    if state.purchase_pending() {
        return;
    }
    let Some(shop) = state.browse() else {
        return;
    };
    basket.step(shop, step.index, step.up);
}

pub fn on_basket_refresh(_: On<Activate>, mut intents: MessageWriter<VendingIntent>) {
    intents.write(VendingIntent::Refresh);
}

/// Confirms the staged quantities and quoted prices in exactly one request.
pub fn on_checkout(
    _: On<Activate>,
    state: Res<VendingState>,
    statuses: Query<&CharacterStatus, With<LocalPlayer>>,
    basket: Res<BuyerBasket>,
    mut ui: ResMut<VendingUi>,
    mut intents: MessageWriter<VendingIntent>,
) {
    if state.purchase_pending() {
        return;
    }
    let Some(shop) = state.browse() else {
        return;
    };
    if basket.is_empty() {
        ui.set_error(Some("Choose at least one item.".to_string()));
        return;
    }
    let Some(total) = basket.total(shop) else {
        ui.set_error(Some("The basket no longer matches this shop.".to_string()));
        return;
    };
    let zeny = u64::from(statuses.iter().next().map_or(0, |status| status.zeny));
    if total > zeny {
        ui.set_error(Some(format!("You need {} more zeny.", total - zeny)));
        return;
    }
    ui.set_error(None);
    intents.write(VendingIntent::Purchase {
        vendor_unit_id: shop.vendor_unit_id,
        shop_id: shop.shop_id.clone(),
        items: basket.to_buy_lines(shop),
    });
}
