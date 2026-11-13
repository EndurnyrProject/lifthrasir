//! Vending window: the seller's shop setup, the seller's open shop, a buyer's view of
//! someone else's shop, and the summary a closed shop leaves behind.
//!
//! Authority stays in [`VendingState`] and the server: this module only projects that
//! state into a window and turns its controls into [`VendingIntent`]s. It never
//! charges SP, moves items, or edits inventory or zeny.
//!
//! The frame and its body host are spawned once. [`rebuild_content`] refills the host
//! when what it shows changes. The setup body is identified by its listed slots alone,
//! so a focused field survives typing and every unrelated update, and
//! [`sync_setup_readouts`] keeps its notes and totals current in place.

use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::Activate;
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_assets::catalogs::random_option::RandomOptionDb;
use lifthrasir_client::core::state::GameState;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::entities::character::components::status::CharacterStatus;
use lifthrasir_client::domain::entities::components::EntityName;
use lifthrasir_client::domain::entities::markers::LocalPlayer;
use lifthrasir_client::domain::entities::registry::EntityRegistry;
use lifthrasir_client::domain::vending::{
    RevealVendingSeller, VendingIntent, VendingNotice, VendingState, VendingSystems,
};
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::primitives::WINDOW_SHADOW;
use net_contract::dto::{
    VendingCloseTarget, VendingListOutcome, VendingPurchaseOutcome, VendingSnapshot,
};
use net_contract::events::ChatNotice;

pub mod buyer;
pub mod feedback;
pub mod scene;
pub mod seller;
mod view;

#[cfg(test)]
mod tests;

use buyer::BuyerBasket;
use feedback::vending_notice_text;
pub use seller::{DraftRow, SellerDraft, clamp_title};

use crate::screens::shared::{
    DISABLED_TINT, light_field_focus, set_field_text, shown_if, typed_number,
};
use crate::widgets::info_modal::{InfoContent, ShowInfoModal, view as info_view};
use crate::widgets::pushcart_window::CartWindowRoot;

/// Marks the window root so visibility, width, drag and close act on this window only.
#[derive(Component, Default, Clone)]
pub struct VendingWindowRoot;

/// The draggable title bar. Its text child is retitled as the body changes.
#[derive(Component, Default, Clone)]
pub struct VendingWindowTitlebar;

/// The region [`rebuild_content`] refills; typed fields live here.
#[derive(Component, Default, Clone)]
pub struct VendingContentHost;

/// The shop name field; read on submit, never mirrored per keystroke.
#[derive(Component, Default, Clone)]
pub struct VendingTitleField;

/// Which typed value a listing field holds.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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

/// The quantity field of the browsed listing with this index. The row's stepper edits
/// it and Add reads it.
#[derive(Component, Default, Clone, Copy)]
pub struct VendingQtyField(pub u32);

/// The Open Shop button, greyed out until the draft can open.
#[derive(Component, Default, Clone)]
pub struct VendingOpenButton;

/// Where an inspected item lives.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// A cart slot, by its cart index.
    #[default]
    Cart,
    /// A listing of the shop being browsed, by its opaque listing index.
    Listing,
    /// A listing of the seller's own open shop.
    Shop,
}

/// An item row; a right-click opens its details.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendingItem {
    pub surface: Surface,
    pub index: u32,
}

/// A scroll viewport whose offset survives a rebuild.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingScroll {
    #[default]
    Stock,
    Listings,
    Sales,
    ForSale,
    Basket,
}

/// A text of the setup body that follows what is typed. The keyed ones belong to the
/// listing of that cart slot.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupReadout {
    #[default]
    TitleCount,
    /// The title on the sign preview.
    Sign,
    Note(u32),
    /// The price spelled out, as in `2 million`.
    Words(u32),
    Total(u32),
    /// What the whole shop earns if everything sells.
    Potential,
    Status,
}

/// What a vending button does when activated.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendingAction {
    /// The title bar's close button: cancels a setup, hides anything else.
    #[default]
    CloseWindow,
    /// Lists the cart slot with this index.
    List(u32),
    Unlist(u32),
    OpenShop,
    CancelSetup,
    /// Raises the Close Shop prompt.
    AskClose,
    KeepOpen,
    CloseShop,
    /// Steps the quantity field of a browsed listing down.
    RowDec(u32),
    RowInc(u32),
    /// Adds a browsed listing's typed quantity to the basket.
    AddRow(u32),
    RemoveLine(u32),
    /// Sends the basket, unless its total needs confirming first.
    Buy,
    /// Sends a basket the confirmation is up for.
    BuyNow,
    BackFromConfirm,
    /// Dismisses the note about what a refreshed shop changed.
    AckChange,
    Refresh,
    /// Leaves the closed shop's summary for the pushcart window.
    BackToCart,
    SetUpAgain,
}

/// Which body the window is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Setup,
    Shop,
    Browse,
    /// The summary of a shop that just closed.
    Closed,
}

impl Mode {
    /// Width of the window's shape around this body.
    fn width(self) -> f32 {
        match self {
            Mode::Setup => 964.0,
            Mode::Shop => 904.0,
            Mode::Browse => 944.0,
            Mode::Closed => 448.0,
        }
    }
}

/// What the window currently shows, and whether the player wants it open.
#[derive(Resource, Debug, Default)]
pub struct VendingUi {
    visible: bool,
    mode: Option<Mode>,
    /// The browse snapshot the basket was last reconciled against.
    last_browse: Option<VendingSnapshot>,
    error: Option<String>,
    /// What a refreshed shop took out of the basket, until acknowledged.
    change: Option<String>,
    pending_close: bool,
    /// The Close Shop prompt is up.
    ask_close: bool,
    /// The buyer's "check the zeros" prompt is up.
    confirm: bool,
    /// A freshly listed row whose price field should take focus once spawned.
    focus_price: Option<u32>,
    /// The open shop's id with each listing's amount when the window first saw it.
    opening: (String, Vec<(u32, u32)>),
    /// The closed shop whose summary the player has already seen.
    dismissed: Option<String>,
}

impl VendingUi {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Hides the window and retires the closed shop's summary with it.
    fn hide(&mut self, state: &VendingState) {
        self.visible = false;
        if let Some(shop) = state.last_closed_shop() {
            self.dismissed = Some(shop.shop_id.clone());
        }
    }
}

pub struct VendingWindowPlugin;

impl Plugin for VendingWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SellerDraft>()
            .init_resource::<BuyerBasket>()
            .init_resource::<VendingUi>()
            .add_systems(
                Update,
                (
                    reveal_on_intent,
                    sync_window_state,
                    sync_browse_basket,
                    settle_buyer_basket,
                    rebuild_content,
                    focus_new_price_field,
                    sync_setup_readouts,
                    light_field_focus::<VendingQtyField>,
                    deliver_vending_feedback,
                    cancel_on_escape,
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

/// The world as the vending window reads it.
#[derive(SystemParam)]
pub struct VendingData<'w, 's> {
    state: Res<'w, VendingState>,
    cart: Res<'w, Cart>,
    item_db: Res<'w, ItemDb>,
    registry: Res<'w, EntityRegistry>,
    names: Query<'w, 's, &'static EntityName>,
    statuses: Query<'w, 's, &'static CharacterStatus, With<LocalPlayer>>,
}

impl VendingData<'_, '_> {
    /// The local player's zeny, weight and weight limit.
    fn purse(&self) -> (u32, u32, u32) {
        self.statuses.iter().next().map_or((0, 0, 0), |status| {
            (status.zeny, status.weight, status.max_weight)
        })
    }

    /// The vendor's name, once its entity has spawned and been named.
    fn vendor_name(&self, unit_id: u64) -> Option<String> {
        let entity = self.registry.get_entity(u32::try_from(unit_id).ok()?)?;
        self.names.get(entity).ok().map(|name| name.name.clone())
    }
}

/// The typed fields of the setup body.
#[derive(SystemParam)]
pub struct SetupFields<'w, 's> {
    title: Query<'w, 's, &'static EditableText, With<VendingTitleField>>,
    rows: Query<'w, 's, (&'static VendingDraftField, &'static EditableText)>,
}

impl SetupFields<'_, '_> {
    /// Copies what is typed into `draft`. An empty price stays unset and an empty
    /// quantity counts as zero, so neither can pass validation by accident.
    fn absorb(&self, draft: &mut SellerDraft) {
        if let Ok(title) = self.title.single() {
            draft.title = title.value().to_string();
        }
        for (field, text) in &self.rows {
            let Some(row) = draft
                .rows
                .iter_mut()
                .find(|row| row.cart_index == field.cart_index)
            else {
                continue;
            };
            let typed = typed_number(text);
            match field.kind {
                DraftFieldKind::Amount => row.amount = typed.unwrap_or(0),
                DraftFieldKind::Price => row.price = typed,
            }
        }
    }
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

/// Setup preparation wins over everything: the seller editor is authoritative. A
/// closed shop's summary shows last, and only until it is dismissed.
fn current_mode(state: &VendingState, dismissed: Option<&str>) -> Option<Mode> {
    if state.setup().is_some() {
        return Some(Mode::Setup);
    }
    if state.browse().is_some() {
        return Some(Mode::Browse);
    }
    if state.active_shop().is_some() {
        return Some(Mode::Shop);
    }
    state
        .last_closed_shop()
        .filter(|shop| dismissed != Some(shop.shop_id.as_str()))
        .map(|_| Mode::Closed)
}

/// Follows the domain into the body it calls for, then shows, sizes and titles the
/// window to match.
fn sync_window_state(
    data: VendingData,
    mut ui: ResMut<VendingUi>,
    mut draft: ResMut<SellerDraft>,
    mut roots: Query<(&mut Visibility, &mut Node), With<VendingWindowRoot>>,
    titlebars: Query<&Children, With<VendingWindowTitlebar>>,
    mut texts: Query<&mut Text>,
) {
    let state = &data.state;
    // A close that the domain no longer tracks has settled (or was rejected), so
    // the affordance becomes usable again instead of staying latched.
    if ui.pending_close && !state.close_pending() {
        ui.pending_close = false;
    }

    let mode = current_mode(state, ui.dismissed.as_deref());
    if ui.mode != mode {
        // A new setup always opens the editor, and a closed shop its summary.
        if matches!(mode, Some(Mode::Setup | Mode::Closed)) {
            ui.visible = true;
        }
        if mode == Some(Mode::Setup) {
            draft.reset();
        }
        ui.mode = mode;
        ui.error = None;
        ui.change = None;
        ui.pending_close = false;
        ui.ask_close = false;
        ui.confirm = false;
    }
    // A summary that another body replaced is not brought back afterwards.
    let stale_summary = state
        .last_closed_shop()
        .filter(|_| mode.is_some_and(|mode| mode != Mode::Closed))
        .filter(|shop| ui.dismissed.as_deref() != Some(shop.shop_id.as_str()));
    if let Some(shop) = stale_summary {
        ui.dismissed = Some(shop.shop_id.clone());
    }
    let newly_open = state
        .active_shop()
        .filter(|shop| shop.shop_id != ui.opening.0);
    if let Some(shop) = newly_open {
        let amounts = shop
            .items
            .iter()
            .map(|item| (item.index, item.amount))
            .collect();
        ui.opening = (shop.shop_id.clone(), amounts);
    }

    let Ok((mut visibility, mut node)) = roots.single_mut() else {
        return;
    };
    let shown = shown_if(ui.visible && mode.is_some());
    if *visibility != shown {
        *visibility = shown;
    }
    let Some(mode) = mode else {
        return;
    };
    let width = px(mode.width() + 2.0 * WINDOW_SHADOW);
    if node.width != width {
        node.width = width;
    }
    let title = match mode {
        Mode::Setup => "Set Up Shop".to_string(),
        Mode::Shop | Mode::Closed => "Your Shop".to_string(),
        Mode::Browse => state
            .browse()
            .and_then(|shop| data.vendor_name(shop.vendor_unit_id))
            .map_or_else(
                || "Shop".to_string(),
                |name| format!("{name}\u{2019}s Shop"),
            ),
    };
    for child in titlebars.iter().flatten() {
        let Ok(mut text) = texts.get_mut(*child) else {
            continue;
        };
        if text.0 != title {
            text.0.clone_from(&title);
        }
    }
}

/// What the body shows. The setup body is identified by its grant and its listed
/// slots alone, so nothing typed into it and no unrelated update redraws it.
#[derive(PartialEq)]
enum Body {
    Setup(String, Vec<u32>),
    Seller(view::SellerView),
    Buyer(view::BuyerView),
    Closed(view::ClosedView),
}

fn current_body(
    data: &VendingData,
    ui: &VendingUi,
    basket: &BuyerBasket,
    draft: &SellerDraft,
) -> Option<Body> {
    let state = &data.state;
    let (zeny, weight, max_weight) = data.purse();
    Some(match ui.mode? {
        Mode::Setup => Body::Setup(
            state.setup()?.setup_id.clone(),
            draft.rows.iter().map(|row| row.cart_index).collect(),
        ),
        Mode::Shop => Body::Seller(view::SellerView {
            zeny,
            closing: ui.pending_close || state.close_pending(),
            ask_close: ui.ask_close,
            error: ui.error.clone(),
            ..view::seller_view(
                state.active_shop()?,
                &ui.opening.1,
                state.recent_sales(),
                &data.item_db,
            )
        }),
        Mode::Browse => {
            let shop = state.browse()?;
            Body::Buyer(view::BuyerView {
                vendor: data.vendor_name(shop.vendor_unit_id),
                zeny,
                weight,
                max_weight,
                change: ui.change.clone(),
                error: ui.error.clone(),
                confirm: ui.confirm,
                pending: state.purchase_pending(),
                ..view::buyer_view(shop, basket, &data.item_db)
            })
        }
        Mode::Closed => Body::Closed(view::closed_view(state.last_closed_shop()?)),
    })
}

/// The body host and the scroll offsets of what it holds.
#[derive(SystemParam)]
struct Mounts<'w, 's> {
    hosts: Query<
        'w,
        's,
        (
            Entity,
            Ref<'static, VendingContentHost>,
            Option<&'static Children>,
        ),
    >,
    scrolls: Query<'w, 's, (&'static VendingScroll, &'static ScrollPosition)>,
}

impl Mounts<'_, '_> {
    fn scroll(&self, kind: VendingScroll) -> Vec2 {
        self.scrolls
            .iter()
            .find(|(scroll, _)| **scroll == kind)
            .map(|(_, position)| position.0)
            .unwrap_or_default()
    }
}

/// The seller's draft with the fields it is typed into.
#[derive(SystemParam)]
struct Drafting<'w, 's> {
    draft: ResMut<'w, SellerDraft>,
    fields: SetupFields<'w, 's>,
}

/// Refills the body host when what it shows changes, and empties it while the window
/// is hidden. Lists keep their scroll offsets, and what was typed into the setup body
/// is copied into the draft before its fields are respawned.
fn rebuild_content(
    mut commands: Commands,
    data: VendingData,
    ui: Res<VendingUi>,
    basket: Res<BuyerBasket>,
    mut drafting: Drafting,
    mounts: Mounts,
    mut shown: Local<Option<Body>>,
) {
    let Ok((host, mount, children)) = mounts.hosts.single() else {
        return;
    };
    let body = ui
        .is_visible()
        .then(|| current_body(&data, &ui, &basket, &drafting.draft))
        .flatten();
    if body == *shown && !mount.is_added() {
        return;
    }
    drafting.fields.absorb(&mut drafting.draft);
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    let scroll = |first, second| [mounts.scroll(first), mounts.scroll(second)];
    let content = match &body {
        Some(Body::Setup(..)) => data.state.setup().map(|setup| {
            let view = view::setup_view(setup, &drafting.draft, &data.cart, &data.item_db);
            let scroll = scroll(VendingScroll::Stock, VendingScroll::Listings);
            commands.spawn_scene(scene::setup(view, scroll)).id()
        }),
        Some(Body::Seller(view)) => {
            let scroll = scroll(VendingScroll::Stock, VendingScroll::Sales);
            Some(
                commands
                    .spawn_scene(scene::seller(view.clone(), scroll))
                    .id(),
            )
        }
        Some(Body::Buyer(view)) => {
            let scroll = scroll(VendingScroll::ForSale, VendingScroll::Basket);
            Some(
                commands
                    .spawn_scene(scene::buyer(view.clone(), scroll))
                    .id(),
            )
        }
        Some(Body::Closed(view)) => Some(commands.spawn_scene(scene::closed(view.clone())).id()),
        None => None,
    };
    if let Some(content) = content {
        commands.entity(content).insert(ChildOf(host));
    }
    *shown = body;
}

/// Focuses the price field of a row that was just listed.
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

type TitleField<'a> = (Entity, &'a EditableText, &'a mut BorderColor);
type DraftField<'a> = (
    Entity,
    &'a VendingDraftField,
    &'a EditableText,
    &'a mut BorderColor,
);

/// Keeps the setup body's read-outs, field borders and Open Shop button in step with
/// what is typed, without respawning anything.
fn sync_setup_readouts(
    data: VendingData,
    ui: Res<VendingUi>,
    focus: Res<InputFocus>,
    mut titles: Query<TitleField, With<VendingTitleField>>,
    mut rows: Query<DraftField, Without<VendingTitleField>>,
    mut readouts: Query<(&SetupReadout, &mut Text, &mut TextColor)>,
    mut open: Query<(&mut ImageNode, &mut Pickable), With<VendingOpenButton>>,
) {
    let Some(setup) = data.state.setup() else {
        return;
    };
    let Ok((title_field, title, mut title_border)) = titles.single_mut() else {
        return;
    };
    let mut typed: Vec<(u32, Option<u32>, Option<u32>)> = Vec::new();
    for (_, field, text, _) in &rows {
        let position = typed
            .iter()
            .position(|(cart_index, ..)| *cart_index == field.cart_index)
            .unwrap_or_else(|| {
                typed.push((field.cart_index, None, None));
                typed.len() - 1
            });
        match field.kind {
            DraftFieldKind::Amount => typed[position].1 = typed_number(text),
            DraftFieldKind::Price => typed[position].2 = typed_number(text),
        }
    }
    let live = view::setup_live(
        view::TypedSetup {
            title: &title.value().to_string(),
            rows: &typed,
        },
        setup,
        &data.cart,
        &data.item_db,
        ui.error(),
    );

    for (readout, mut text, mut color) in &mut readouts {
        let Some((value, tone)) = live.texts.get(readout) else {
            continue;
        };
        if text.0 != *value {
            text.0.clone_from(value);
        }
        if color.0 != *tone {
            color.0 = *tone;
        }
    }

    let border = |entity, bad| {
        BorderColor::all(if focus.get() == Some(entity) {
            ro_dark::FROST_GLOW
        } else if bad {
            ro_dark::STATUS_FULL
        } else {
            ro_dark::SLOT_STROKE
        })
    };
    let wanted = border(title_field, live.bad_title);
    if *title_border != wanted {
        *title_border = wanted;
    }
    for (entity, field, _, mut stroke) in &mut rows {
        let bad = live.bad_fields.contains(&(field.cart_index, field.kind));
        let wanted = border(entity, bad);
        if *stroke != wanted {
            *stroke = wanted;
        }
    }

    for (mut art, mut pickable) in &mut open {
        let (tint, usable) = if live.ready {
            (Color::WHITE, true)
        } else {
            (DISABLED_TINT, false)
        };
        if art.color != tint {
            art.color = tint;
        }
        if pickable.is_hoverable != usable {
            *pickable = Pickable {
                should_block_lower: usable,
                is_hoverable: usable,
            };
        }
    }
}

type QtyField = (&'static VendingQtyField, &'static mut EditableText);
/// Keeps the buyer's quantity fields apart from the setup fields [`SetupFields`] reads.
type NotSetupField = (Without<VendingDraftField>, Without<VendingTitleField>);

/// Everything a vending button may read or change. Only local staging and intents:
/// the server still validates the submitted shop or purchase.
#[derive(SystemParam)]
pub struct VendingActions<'w, 's> {
    data: VendingData<'w, 's>,
    draft: ResMut<'w, SellerDraft>,
    basket: ResMut<'w, BuyerBasket>,
    ui: ResMut<'w, VendingUi>,
    intents: MessageWriter<'w, VendingIntent>,
    focus: ResMut<'w, InputFocus>,
    fields: SetupFields<'w, 's>,
    quantities: Query<'w, 's, QtyField, NotSetupField>,
    carts: Query<'w, 's, &'static mut Visibility, With<CartWindowRoot>>,
}

impl VendingActions<'_, '_> {
    fn run(&mut self, action: VendingAction) {
        match action {
            VendingAction::CloseWindow => {
                close_window(&self.data.state, &mut self.ui, &mut self.intents);
            }
            VendingAction::List(cart_index) => {
                self.ui.error = self.list(cart_index).err();
            }
            VendingAction::Unlist(cart_index) => {
                self.draft.remove(cart_index);
            }
            VendingAction::OpenShop => self.open_shop(),
            VendingAction::CancelSetup => self.cancel_setup(),
            VendingAction::AskClose => self.ui.ask_close = true,
            VendingAction::KeepOpen => self.ui.ask_close = false,
            VendingAction::CloseShop => self.close_shop(),
            VendingAction::RowDec(index) => self.step_row(index, -1),
            VendingAction::RowInc(index) => self.step_row(index, 1),
            VendingAction::AddRow(index) => self.add_row(index),
            VendingAction::RemoveLine(index) => self.remove_line(index),
            VendingAction::Buy => self.buy(false),
            VendingAction::BuyNow => self.buy(true),
            VendingAction::BackFromConfirm => self.ui.confirm = false,
            VendingAction::AckChange => self.ui.change = None,
            VendingAction::Refresh => {
                self.intents.write(VendingIntent::Refresh);
            }
            VendingAction::BackToCart => {
                self.ui.hide(&self.data.state);
                for mut visibility in &mut self.carts {
                    *visibility = Visibility::Visible;
                }
            }
            VendingAction::SetUpAgain => {
                self.ui.hide(&self.data.state);
                self.intents.write(VendingIntent::EnterSeller);
            }
        }
    }

    /// Lists a cart slot with its whole stack and queues its price field for focus.
    fn list(&mut self, cart_index: u32) -> Result<(), String> {
        let setup = self.data.state.setup().ok_or("The shop setup has ended.")?;
        let stock = u16::try_from(cart_index)
            .ok()
            .and_then(|slot| self.data.cart.get(slot))
            .map(|item| item.amount)
            .ok_or("That cart slot no longer holds an item.")?;
        if self.draft.add(cart_index, stock, setup)? {
            self.ui.focus_price = Some(cart_index);
        }
        Ok(())
    }

    fn open_shop(&mut self) {
        if self.ui.pending_close || self.data.state.open_pending() {
            return;
        }
        let Some(setup) = self.data.state.setup() else {
            return;
        };
        self.fields.absorb(&mut self.draft);
        self.draft.title = clamp_title(self.draft.title.trim(), setup.max_title_bytes);
        if let Err(error) = self.draft.validate(setup, &self.data.cart) {
            self.ui.error = Some(error);
            return;
        }
        self.ui.error = None;
        self.focus.clear();
        self.intents.write(VendingIntent::Open {
            setup_id: setup.setup_id.clone(),
            title: self.draft.title.clone(),
            entries: self.draft.offers(),
        });
    }

    fn cancel_setup(&mut self) {
        let Some(setup) = self.data.state.setup() else {
            return;
        };
        if self.ui.pending_close {
            return;
        }
        self.ui.pending_close = true;
        self.focus.clear();
        self.intents.write(VendingIntent::Close {
            target: VendingCloseTarget::Setup(setup.setup_id.clone()),
        });
    }

    fn close_shop(&mut self) {
        let Some(shop) = self.data.state.active_shop() else {
            return;
        };
        if self.ui.pending_close {
            return;
        }
        self.ui.pending_close = true;
        self.ui.ask_close = false;
        self.intents.write(VendingIntent::Close {
            target: VendingCloseTarget::Shop(shop.shop_id.clone()),
        });
    }

    /// The typed quantity of a browsed listing. An empty field counts as one.
    fn typed_quantity(&self, index: u32) -> u32 {
        self.quantities
            .iter()
            .find(|(field, _)| field.0 == index)
            .and_then(|(_, text)| typed_number(text))
            .unwrap_or(1)
            .max(1)
    }

    /// Steps a listing's quantity field within what the basket has not taken yet.
    fn step_row(&mut self, index: u32, step: i32) {
        let Some(shop) = self.data.state.browse() else {
            return;
        };
        let Some(listing) = shop.items.iter().find(|item| item.index == index) else {
            return;
        };
        let room = listing
            .amount
            .saturating_sub(self.basket.quantity(index))
            .max(1);
        let quantity = self
            .typed_quantity(index)
            .saturating_add_signed(step)
            .clamp(1, room);
        if let Some((_, mut text)) = self
            .quantities
            .iter_mut()
            .find(|(field, _)| field.0 == index)
        {
            set_field_text(&mut text, &quantity.to_string());
        }
    }

    fn add_row(&mut self, index: u32) {
        if self.data.state.purchase_pending() {
            return;
        }
        let Some(shop) = self.data.state.browse() else {
            return;
        };
        let wanted = self
            .basket
            .quantity(index)
            .saturating_add(self.typed_quantity(index));
        self.basket.set_quantity(shop, index, wanted);
        self.ui.confirm = false;
    }

    fn remove_line(&mut self, index: u32) {
        if self.data.state.purchase_pending() {
            return;
        }
        if let Some(shop) = self.data.state.browse() {
            self.basket.set_quantity(shop, index, 0);
        }
        self.ui.confirm = false;
    }

    /// Confirms the staged quantities and quoted prices in exactly one request. A
    /// total of [`view::CONFIRM_FROM`] or more raises the prompt first, and only
    /// `confirmed` sends it.
    fn buy(&mut self, confirmed: bool) {
        if self.data.state.purchase_pending() {
            return;
        }
        let Some(shop) = self.data.state.browse() else {
            return;
        };
        let refusal = match self.basket.total(shop) {
            _ if self.basket.is_empty() => Err("Choose at least one item.".to_string()),
            None => Err("The basket no longer matches this shop.".to_string()),
            Some(total) => {
                let zeny = u64::from(self.data.purse().0);
                if total > zeny {
                    Err(format!("You need {} more zeny.", total - zeny))
                } else {
                    Ok(total)
                }
            }
        };
        let total = match refusal {
            Ok(total) => total,
            Err(error) => {
                self.ui.error = Some(error);
                return;
            }
        };
        self.ui.error = None;
        if total >= view::CONFIRM_FROM && !confirmed {
            self.ui.confirm = true;
            return;
        }
        self.ui.confirm = false;
        self.intents.write(VendingIntent::Purchase {
            vendor_unit_id: shop.vendor_unit_id,
            shop_id: shop.shop_id.clone(),
            items: self.basket.to_buy_lines(shop),
        });
    }
}

/// Runs the [`VendingAction`] of the activated button.
///
/// `bevy_ui_widgets`' `Button` fires `Activate` on release of any pointer button, so
/// a right-click on a row's button, meant only to open the item details, reaches this
/// handler too and is dropped here.
pub fn on_vending_button(
    activate: On<Activate>,
    actions: Query<&VendingAction>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut vending: VendingActions,
) {
    if mouse.is_some_and(|mouse| mouse.just_released(MouseButton::Right)) {
        return;
    }
    if let Ok(action) = actions.get(activate.entity) {
        vending.run(*action);
    }
}

/// Closing the window cancels a setup (the grant is still held), and hides anything
/// else. It never closes an open shop: the server keeps selling behind a hidden window.
fn close_window(
    state: &VendingState,
    ui: &mut VendingUi,
    intents: &mut MessageWriter<VendingIntent>,
) {
    let Some(setup) = state.setup() else {
        ui.hide(state);
        return;
    };
    if ui.pending_close {
        return;
    }
    ui.pending_close = true;
    intents.write(VendingIntent::Close {
        target: VendingCloseTarget::Setup(setup.setup_id.clone()),
    });
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
    close_window(&state, &mut ui, &mut intents);
}

fn deliver_vending_feedback(
    mut notices: MessageReader<VendingNotice>,
    mut chat: MessageWriter<ChatNotice>,
) {
    for notice in notices.read() {
        chat.write(ChatNotice {
            error: false,
            message: vending_notice_text(notice),
        });
    }
}

/// Binds the basket to the shop being browsed and re-applies it to a refreshed
/// snapshot, noting every dropped, re-priced, or clamped row for the buyer.
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
            ui.change = Some(messages.join(" "));
            ui.confirm = false;
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

/// The details view for a vending item: a cart item with its cards and options, or a
/// vendor or own-shop listing with its price and creator.
pub fn inspect_view(
    item: VendingItem,
    state: &VendingState,
    cart: &Cart,
    item_db: &ItemDb,
    option_db: Option<&RandomOptionDb>,
) -> Option<info_view::ItemInfoView> {
    let listing_in = |shop: Option<&VendingSnapshot>| {
        shop?
            .items
            .iter()
            .find(|listing| listing.index == item.index)
            .map(|listing| info_view::vending_item_view(listing, item_db, option_db))
    };
    match item.surface {
        Surface::Cart => {
            let slot = cart.get(u16::try_from(item.index).ok()?)?;
            Some(info_view::cart_item_view(slot, item_db, option_db))
        }
        Surface::Listing => listing_in(state.browse()),
        Surface::Shop => listing_in(state.active_shop()),
    }
}

/// A secondary click on any vending row opens its item's details. Info only: vending
/// items are never in the bag, so no equip, use or compound action applies.
pub fn on_item_inspect(
    click: On<PointerClick>,
    items: Query<&VendingItem>,
    data: VendingData,
    option_db: Option<Res<RandomOptionDb>>,
    mut info: MessageWriter<ShowInfoModal>,
) {
    if click.button != PointerButton::Secondary {
        return;
    }
    let Ok(item) = items.get(click.entity) else {
        return;
    };
    let view = inspect_view(
        *item,
        &data.state,
        &data.cart,
        &data.item_db,
        option_db.as_deref(),
    );
    if let Some(view) = view {
        info.write(ShowInfoModal {
            content: InfoContent::Item { view, action: None },
        });
    }
}
