//! The left panel of the vending window: what can be dragged into the right one.
//!
//! During setup it shows the cart's sellable stock; while browsing, the vendor's
//! listings. Its cells are drag sources (and double-click shortcuts); the panel
//! itself accepts rows dragged back out of the right panel to remove them. It
//! holds no typed fields, so it is rebuilt freely whenever its structure changes.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use lifthrasir_assets::catalogs::item::registry::ItemDb;
use lifthrasir_client::domain::cart::Cart;
use lifthrasir_client::domain::vending::VendingState;
use lifthrasir_ui::theme;
use lifthrasir_ui::theme::feathers_theme::{TOKEN_PANEL_BG, TOKEN_TEXT, TOKEN_TEXT_DIM};
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use net_contract::dto::{VendingSetup, VendingSnapshot};

use super::buyer::BuyerBasket;
use super::cells::{ItemCellView, item_cell, item_icon, item_name};
use super::drag::{DragKind, DragSource, DropZone, on_cell_click, on_drag_start, on_zone_drop};
use super::{Mode, SellerDraft, VendingUi, current_mode, on_item_inspect};

const PANEL_WIDTH: f32 = 214.0;
const GRID_HEIGHT: f32 = 236.0;
const STOCK_HINT: &str = "Drag or double-click items to list them.";
const LISTING_HINT: &str = "Drag or double-click to buy. Right-click for details.";

/// The host the left panel is spawned into; hidden when there is nothing to show.
#[derive(Component, Default, Clone)]
pub struct VendingSourceHost;

/// The panel's info line: the hovered item, or a usage hint.
#[derive(Component, Default, Clone)]
pub struct VendingSourceInfo;

/// The panel's identity: mode, setup or shop id, and each cell's
/// (index, amount, staged), where staged is 1 for a listed stock slot and the
/// basket quantity for a vendor listing.
pub(super) type SourceStructure = (Mode, String, Vec<(u32, u32, u32)>);

/// One cell to render: its drag identity, look, and staged count.
struct SourceCell {
    source: DragSource,
    view: ItemCellView,
    staged: u32,
}

/// What the panel shows in one mode.
struct Panel {
    mode: Mode,
    id: String,
    title: String,
    zone: DropZone,
    cells: Vec<SourceCell>,
}

/// Sellable cart slots: dimmed and ticked once listed.
fn stock_cells(
    setup: &VendingSetup,
    draft: &SellerDraft,
    cart: &Cart,
    item_db: &ItemDb,
) -> Vec<SourceCell> {
    setup
        .sellable_cart_indices
        .iter()
        .filter_map(|&cart_index| {
            let item = cart.get(u16::try_from(cart_index).ok()?)?;
            let listed = draft.row(cart_index).is_some();
            Some(SourceCell {
                source: DragSource {
                    kind: DragKind::Stock,
                    index: cart_index,
                },
                view: ItemCellView {
                    icon: item_icon(item_db, item.nameid, item.identified),
                    amount: item.amount,
                    refine: item.refine,
                    dimmed: listed,
                    ticked: listed,
                },
                staged: u32::from(listed),
            })
        })
        .collect()
}

/// Vendor listings: ticked once in the basket, dimmed when all stock is staged.
fn listing_cells(
    shop: &VendingSnapshot,
    basket: &BuyerBasket,
    item_db: &ItemDb,
) -> Vec<SourceCell> {
    shop.items
        .iter()
        .map(|listing| {
            let staged = basket.quantity(listing.index);
            SourceCell {
                source: DragSource {
                    kind: DragKind::Listing,
                    index: listing.index,
                },
                view: ItemCellView {
                    icon: item_icon(item_db, listing.nameid, listing.identified),
                    amount: listing.amount,
                    refine: listing.refine,
                    dimmed: staged >= listing.amount,
                    ticked: staged > 0,
                },
                staged,
            }
        })
        .collect()
}

fn panel(
    state: &VendingState,
    cart: &Cart,
    item_db: &ItemDb,
    draft: &SellerDraft,
    basket: &BuyerBasket,
) -> Option<Panel> {
    match current_mode(state)? {
        Mode::Setup => {
            let setup = state.setup()?;
            Some(Panel {
                mode: Mode::Setup,
                id: setup.setup_id.clone(),
                title: "Cart stock".to_string(),
                zone: DropZone::CartStock,
                cells: stock_cells(setup, draft, cart, item_db),
            })
        }
        Mode::Browse => {
            let shop = state.browse()?;
            Some(Panel {
                mode: Mode::Browse,
                id: shop.shop_id.clone(),
                title: shop.title.clone(),
                zone: DropZone::VendorListing,
                cells: listing_cells(shop, basket, item_db),
            })
        }
        Mode::Shop => None,
    }
}

/// Rebuilds the left panel when its structure changes and hides it otherwise.
#[expect(
    clippy::too_many_arguments,
    reason = "the panel projects the state, cart, draft, basket and item names into its host"
)]
pub(super) fn rebuild_source(
    mut commands: Commands,
    state: Res<VendingState>,
    cart: Res<Cart>,
    item_db: Res<ItemDb>,
    draft: Res<SellerDraft>,
    basket: Res<BuyerBasket>,
    mut ui: ResMut<VendingUi>,
    mut hosts: Query<(Entity, &mut Node, Option<&Children>), With<VendingSourceHost>>,
    new_hosts: Query<(), Added<VendingSourceHost>>,
) {
    let panel = panel(&state, &cart, &item_db, &draft, &basket);
    let structure = panel.as_ref().map(|panel| {
        let cells = panel
            .cells
            .iter()
            .map(|cell| (cell.source.index, cell.view.amount, cell.staged))
            .collect();
        (panel.mode, panel.id.clone(), cells)
    });
    if structure == ui.source_structure && new_hosts.is_empty() {
        return;
    }
    ui.source_structure = structure;

    let Ok((host, mut node, children)) = hosts.single_mut() else {
        return;
    };
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    let Some(panel) = panel else {
        node.display = Display::None;
        return;
    };
    node.display = Display::Flex;
    commands
        .spawn_scene(source_panel(panel.title, panel.cells))
        .insert((ChildOf(host), panel.zone));
}

/// Shows the hovered item, or the usage hint, on the info line.
pub(super) fn sync_source_info(
    state: Res<VendingState>,
    cart: Res<Cart>,
    item_db: Res<ItemDb>,
    ui: Res<VendingUi>,
    mut lines: Query<&mut Text, With<VendingSourceInfo>>,
) {
    let line = source_info(ui.hovered, &state, &cart, &item_db);
    for mut text in &mut lines {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}

pub(super) fn source_info(
    hovered: Option<DragSource>,
    state: &VendingState,
    cart: &Cart,
    item_db: &ItemDb,
) -> String {
    if let Some(shop) = state.browse() {
        let described = hovered
            .filter(|source| source.kind == DragKind::Listing)
            .and_then(|source| shop.items.iter().find(|item| item.index == source.index))
            .map(|listing| {
                format!(
                    "{}  -  {}z  -  {} left",
                    item_name(item_db, listing.nameid, listing.identified),
                    listing.price,
                    listing.amount
                )
            });
        return described.unwrap_or_else(|| LISTING_HINT.to_string());
    }
    let described = hovered
        .filter(|source| source.kind == DragKind::Stock && state.setup().is_some())
        .and_then(|source| cart.get(u16::try_from(source.index).ok()?))
        .map(|item| {
            format!(
                "{} x{}",
                item_name(item_db, item.nameid, item.identified),
                item.amount
            )
        });
    described.unwrap_or_else(|| STOCK_HINT.to_string())
}

pub fn on_cell_over(over: On<PointerOver>, sources: Query<&DragSource>, mut ui: ResMut<VendingUi>) {
    if let Ok(source) = sources.get(over.entity) {
        ui.hovered = Some(*source);
    }
}

pub fn on_cell_out(out: On<PointerOut>, sources: Query<&DragSource>, mut ui: ResMut<VendingUi>) {
    if sources
        .get(out.entity)
        .is_ok_and(|source| ui.hovered == Some(*source))
    {
        ui.hovered = None;
    }
}

/// The panel root; the caller adds its `DropZone`.
fn source_panel(title: String, cells: Vec<SourceCell>) -> impl Scene {
    let empty = cells.is_empty();
    let items: Vec<_> = cells.into_iter().map(source_cell).collect();
    let empty = empty.then(|| {
        EntityScene(chrome_text(
            "Nothing to sell.".to_string(),
            12.0,
            theme::TEXT_DIM,
        ))
    });
    bsn! {
        Node {
            width: px(PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            padding: {UiRect::all(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        ThemeBackgroundColor({TOKEN_PANEL_BG})
        BorderColor::all(theme::GOLD_FAINT)
        Pickable
        on(on_zone_drop)
        Children [
            Text(title)
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_TITLE),
                font_size: {FontSize::Px(12.0)},
            }
            ThemeTextColor({TOKEN_TEXT})
            @ignore_picking() --
            Node { height: px(GRID_HEIGHT), position_type: PositionType::Relative }
            @ignore_picking()
            Children [
                #grid
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), top: px(0), right: px(0), bottom: px(0),
                    overflow: {Overflow::scroll_y()},
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FlexStart,
                    column_gap: px(6),
                    row_gap: px(6),
                    padding: {UiRect { right: Val::Px(10.0), ..default() }},
                }
                ScrollArea
                Pickable
                Children [ {items} -- {empty} ]
                --
                @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
                Node {
                    position_type: PositionType::Absolute,
                    right: px(0),
                    top: px(0),
                    bottom: px(0),
                    width: px(6),
                }
            ] --
            VendingSourceInfo
            Text("")
            TextFont {
                font: FontSourceTemplate::Handle(theme::FONT_BODY),
                font_size: {FontSize::Px(11.0)},
            }
            ThemeTextColor({TOKEN_TEXT_DIM})
            @ignore_picking()
        ]
    }
}

fn source_cell(cell: SourceCell) -> impl Scene {
    let SourceCell { source, view, .. } = cell;
    bsn! {
        @item_cell(view)
        DragSource { kind: {source.kind}, index: {source.index} }
        Pickable
        on(on_drag_start)
        on(on_cell_click)
        on(on_cell_over)
        on(on_cell_out)
        on(on_item_inspect)
    }
}
