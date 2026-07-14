use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::inventory::item::item_category;
use game_engine::domain::inventory::{Inventory, Item, ItemCategory};
use game_engine::domain::storage::Storage;
use game_engine::infrastructure::item::ItemDb;
use net_contract::commands::{CloseStorage, DepositStorageItem, WithdrawStorageItem};
use net_contract::dto::StorageItem;
use net_contract::events::{StorageRejection, StorageResult};
use std::time::Duration;

use crate::theme;
use crate::theme::feathers_theme::install_norse_theme;

pub mod scene;

const DOUBLE_CLICK: Duration = Duration::from_millis(300);

#[derive(Component, Default, Clone)]
pub struct StorageWindowRoot;

#[derive(Component, Default, Clone)]
pub struct StorageWindowTitlebar;

#[derive(Component, Default, Clone)]
pub struct StorageBagHost;

#[derive(Component, Default, Clone)]
pub struct StorageVaultHost;

#[derive(Component, Default, Clone)]
pub struct StorageOverlayHost;

#[derive(Component, Default, Clone)]
pub struct StorageErrorHost;

#[derive(Component, Default, Clone)]
pub struct StorageSearchField;

#[derive(Component, Default, Clone)]
pub struct StorageSearchPlaceholder;

#[derive(Component, Default, Clone)]
pub struct StorageAmountField;

type StorageFieldFilter = Or<(With<StorageSearchField>, With<StorageAmountField>)>;

#[derive(Component, Default, Clone)]
pub struct StorageAmountConfirm;

#[derive(Component, Default, Clone)]
pub struct StorageAmountCancel;

#[derive(Component, Default, Clone)]
pub struct StorageCloseControl;

#[derive(Component, Default, Clone, Copy)]
pub struct StorageCategoryButton(pub StorageCategory);

#[derive(Component, Default, Clone, Copy)]
pub struct StorageCell(pub StorageSelection);

#[derive(Component, Default, Clone, Copy)]
pub struct StorageQuickTransfer(pub StorageSelection);

#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageTransferDirection {
    #[default]
    Deposit,
    Withdraw,
}

#[derive(Component, Default, Clone, Copy)]
pub struct StorageTransferButton {
    pub direction: StorageTransferDirection,
    pub enabled: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StorageCategory {
    #[default]
    All,
    Use,
    Etc,
    Equip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSelection {
    Bag(u16),
    Vault(u32),
}

impl Default for StorageSelection {
    fn default() -> Self {
        Self::Bag(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTransfer {
    pub source: StorageSelection,
    pub amount: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastStorageClick {
    pub selection: StorageSelection,
    pub at: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferIntent {
    Deposit { inventory_index: u32, amount: u32 },
    Withdraw { storage_index: u32, amount: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmountValidationError {
    Empty,
    NotNumber,
    OutOfRange,
    SourceMissing,
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct StorageUi {
    pub category: StorageCategory,
    query: String,
    pub selection: Option<StorageSelection>,
    pub pending_transfer: Option<PendingTransfer>,
    pub awaiting_result: bool,
    pub panel_error: Option<String>,
    pub last_click: Option<LastStorageClick>,
    pub previous_open: bool,
}

impl StorageUi {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
    }
}

pub(crate) fn matches_filter(
    selected: StorageCategory,
    query: &str,
    item_category: ItemCategory,
    item_name: &str,
) -> bool {
    let category_matches = match selected {
        StorageCategory::All => true,
        StorageCategory::Use => item_category == ItemCategory::Use,
        StorageCategory::Etc => item_category == ItemCategory::Etc,
        StorageCategory::Equip => item_category == ItemCategory::Equip,
    };
    category_matches
        && item_name
            .to_lowercase()
            .contains(&query.trim().to_lowercase())
}

pub(crate) fn bag_projection<'a>(
    inventory: &'a Inventory,
    item_db: &ItemDb,
    category: StorageCategory,
    query: &str,
) -> Vec<&'a Item> {
    inventory
        .iter()
        .filter(|item| !item.is_equipped())
        .filter(|item| {
            let name = item_db
                .name(item.item_id, item.identified)
                .expect("open Storage item must exist in ItemDb");
            matches_filter(category, query, item.category(), name)
        })
        .collect()
}

pub(crate) fn vault_projection<'a>(
    storage: &'a Storage,
    item_db: &ItemDb,
    category: StorageCategory,
    query: &str,
) -> Vec<&'a StorageItem> {
    storage
        .iter()
        .filter(|item| {
            let name = item_db
                .name(item.nameid, item.identified)
                .expect("open Storage item must exist in ItemDb");
            matches_filter(category, query, item_category(item.type_), name)
        })
        .collect()
}

pub(crate) fn validated_selection(
    selection: Option<StorageSelection>,
    bag: &[&Item],
    vault: &[&StorageItem],
) -> Option<StorageSelection> {
    selection.filter(|selection| match selection {
        StorageSelection::Bag(index) => bag.iter().any(|item| item.index == *index),
        StorageSelection::Vault(index) => vault.iter().any(|item| item.index == *index),
    })
}

pub(crate) fn transfer_intent(source: StorageSelection, amount: u32) -> TransferIntent {
    match source {
        StorageSelection::Bag(index) => TransferIntent::Deposit {
            inventory_index: u32::from(index),
            amount,
        },
        StorageSelection::Vault(index) => TransferIntent::Withdraw {
            storage_index: index,
            amount,
        },
    }
}

pub(crate) fn validate_live_amount(
    source: StorageSelection,
    input: &str,
    inventory: &Inventory,
    storage: &Storage,
) -> Result<u32, AmountValidationError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(AmountValidationError::Empty);
    }
    let amount = input
        .parse::<u32>()
        .map_err(|_| AmountValidationError::NotNumber)?;
    let available = match source {
        StorageSelection::Bag(index) => inventory
            .get(index)
            .filter(|item| !item.is_equipped())
            .map(|item| u32::from(item.amount)),
        StorageSelection::Vault(index) => storage.get(index).map(|item| item.amount),
    }
    .ok_or(AmountValidationError::SourceMissing)?;

    if amount == 0 || amount > available {
        return Err(AmountValidationError::OutOfRange);
    }
    Ok(amount)
}

pub(crate) fn rejection_message(rejection: StorageRejection) -> String {
    match rejection {
        StorageRejection::Full => "Storage is full.".to_string(),
        StorageRejection::InventoryFull => "Your inventory is full.".to_string(),
        StorageRejection::Overweight => "You are carrying too much weight.".to_string(),
        StorageRejection::NotStorable => "This item cannot be stored.".to_string(),
        StorageRejection::ItemEquipped => "Equipped items cannot be stored.".to_string(),
        StorageRejection::InvalidAmount => "Enter a valid amount.".to_string(),
        StorageRejection::NotOpen => "Storage is not open.".to_string(),
        StorageRejection::BasicSkillRequired => "Basic Skill level 6 is required.".to_string(),
        StorageRejection::Unknown(code) => format!("Storage request failed (code {code})."),
    }
}

pub(crate) fn is_double_click(
    last: Option<LastStorageClick>,
    selection: StorageSelection,
    now: Duration,
) -> bool {
    last.is_some_and(|last| {
        last.selection == selection && now.saturating_sub(last.at) <= DOUBLE_CLICK
    })
}

fn write_transfer(
    source: StorageSelection,
    amount: u32,
    deposit: &mut MessageWriter<DepositStorageItem>,
    withdraw: &mut MessageWriter<WithdrawStorageItem>,
) {
    match transfer_intent(source, amount) {
        TransferIntent::Deposit {
            inventory_index,
            amount,
        } => {
            deposit.write(DepositStorageItem {
                inventory_index,
                amount,
            });
        }
        TransferIntent::Withdraw {
            storage_index,
            amount,
        } => {
            withdraw.write(WithdrawStorageItem {
                storage_index,
                amount,
            });
        }
    }
}

fn begin_transfer(
    source: StorageSelection,
    inventory: &Inventory,
    storage: &Storage,
    ui: &mut StorageUi,
    deposit: &mut MessageWriter<DepositStorageItem>,
    withdraw: &mut MessageWriter<WithdrawStorageItem>,
) {
    if ui.awaiting_result || ui.pending_transfer.is_some() || !storage.is_open() {
        return;
    }
    let available = match source {
        StorageSelection::Bag(index) => inventory
            .get(index)
            .filter(|item| !item.is_equipped())
            .map(|item| u32::from(item.amount)),
        StorageSelection::Vault(index) => storage.get(index).map(|item| item.amount),
    };
    let Some(available) = available.filter(|amount| *amount > 0) else {
        return;
    };
    ui.last_click = None;
    ui.panel_error = None;
    if available == 1 {
        write_transfer(source, 1, deposit, withdraw);
        ui.awaiting_result = true;
    } else {
        ui.pending_transfer = Some(PendingTransfer {
            source,
            amount: "1".to_string(),
        });
    }
}

pub(crate) fn on_transfer_activate(
    activate: On<Activate>,
    buttons: Query<&StorageTransferButton>,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    mut ui: ResMut<StorageUi>,
    mut deposit: MessageWriter<DepositStorageItem>,
    mut withdraw: MessageWriter<WithdrawStorageItem>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(source) = ui.selection else {
        return;
    };
    let direction_matches = matches!(
        (button.direction, source),
        (StorageTransferDirection::Deposit, StorageSelection::Bag(_))
            | (
                StorageTransferDirection::Withdraw,
                StorageSelection::Vault(_)
            )
    );
    if !button.enabled || !direction_matches {
        return;
    }
    begin_transfer(
        source,
        &inventory,
        &storage,
        &mut ui,
        &mut deposit,
        &mut withdraw,
    );
}

pub(crate) fn on_quick_transfer_activate(
    activate: On<Activate>,
    buttons: Query<&StorageQuickTransfer>,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    mut ui: ResMut<StorageUi>,
    mut deposit: MessageWriter<DepositStorageItem>,
    mut withdraw: MessageWriter<WithdrawStorageItem>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    begin_transfer(
        button.0,
        &inventory,
        &storage,
        &mut ui,
        &mut deposit,
        &mut withdraw,
    );
}

fn amount_validation_message(error: AmountValidationError) -> &'static str {
    match error {
        AmountValidationError::Empty => "Enter an amount.",
        AmountValidationError::NotNumber => "Enter a valid number.",
        AmountValidationError::OutOfRange => "Enter an amount within the available stack.",
        AmountValidationError::SourceMissing => "The source item is no longer available.",
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_amount_confirm(
    _: On<Activate>,
    fields: Query<(Entity, &EditableText), With<StorageAmountField>>,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    mut input_focus: ResMut<InputFocus>,
    mut ui: ResMut<StorageUi>,
    mut deposit: MessageWriter<DepositStorageItem>,
    mut withdraw: MessageWriter<WithdrawStorageItem>,
) {
    if ui.awaiting_result || !storage.is_open() {
        return;
    }
    let (Some(source), Ok((field_entity, field))) = (
        ui.pending_transfer.as_ref().map(|pending| pending.source),
        fields.single(),
    ) else {
        return;
    };
    let input = field.value().to_string();
    match validate_live_amount(source, &input, &inventory, &storage) {
        Ok(amount) => {
            write_transfer(source, amount, &mut deposit, &mut withdraw);
            ui.pending_transfer = None;
            ui.awaiting_result = true;
            ui.panel_error = None;
            if input_focus.get() == Some(field_entity) {
                input_focus.clear();
            }
        }
        Err(error) => {
            if let Some(pending) = &mut ui.pending_transfer {
                pending.amount = input;
            }
            ui.panel_error = Some(amount_validation_message(error).to_string());
        }
    }
}

pub(crate) fn on_amount_cancel(
    _: On<Activate>,
    fields: Query<(), With<StorageAmountField>>,
    mut input_focus: ResMut<InputFocus>,
    mut ui: ResMut<StorageUi>,
) {
    ui.pending_transfer = None;
    ui.panel_error = None;
    if input_focus
        .get()
        .is_some_and(|entity| fields.contains(entity))
    {
        input_focus.clear();
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_panes(
    mut commands: Commands,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    item_db: Res<ItemDb>,
    mut ui: ResMut<StorageUi>,
    bag_hosts: Query<(Entity, Option<&Children>), With<StorageBagHost>>,
    vault_hosts: Query<(Entity, Option<&Children>), With<StorageVaultHost>>,
    new_bag_hosts: Query<(), Added<StorageBagHost>>,
    new_vault_hosts: Query<(), Added<StorageVaultHost>>,
) {
    if !inventory.is_changed()
        && !storage.is_changed()
        && !ui.is_changed()
        && new_bag_hosts.is_empty()
        && new_vault_hosts.is_empty()
    {
        return;
    }
    let (Ok((bag_host, bag_children)), Ok((vault_host, vault_children))) =
        (bag_hosts.single(), vault_hosts.single())
    else {
        return;
    };
    let bag_items = bag_projection(&inventory, &item_db, ui.category, ui.query());
    let vault_items = vault_projection(&storage, &item_db, ui.category, ui.query());
    let selection = validated_selection(ui.selection, &bag_items, &vault_items);
    if ui.selection != selection {
        ui.selection = selection;
    }
    let (bag, vault) = scene::pane_views(&inventory, &storage, &ui, &item_db);

    for children in [bag_children, vault_children].into_iter().flatten() {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::pane(bag, scene::PaneSide::Bag))
        .insert(ChildOf(bag_host));
    commands
        .spawn_scene(scene::pane(vault, scene::PaneSide::Vault))
        .insert(ChildOf(vault_host));
}

pub(crate) fn on_category_activate(
    activate: On<Activate>,
    buttons: Query<&StorageCategoryButton>,
    mut ui: ResMut<StorageUi>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    ui.category = button.0;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_cell_select(
    click: On<Pointer<Click>>,
    cells: Query<&StorageCell>,
    time: Res<Time>,
    inventory: Res<Inventory>,
    storage: Res<Storage>,
    mut ui: ResMut<StorageUi>,
    mut deposit: MessageWriter<DepositStorageItem>,
    mut withdraw: MessageWriter<WithdrawStorageItem>,
) {
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    ui.selection = Some(cell.0);
    if ui.awaiting_result {
        return;
    }
    let now = time.elapsed();
    let double_click = is_double_click(ui.last_click, cell.0, now);
    ui.last_click = Some(LastStorageClick {
        selection: cell.0,
        at: now,
    });
    if double_click {
        begin_transfer(
            cell.0,
            &inventory,
            &storage,
            &mut ui,
            &mut deposit,
            &mut withdraw,
        );
    }
}

pub(crate) fn stop_quick_transfer_propagation(mut click: On<Pointer<Click>>) {
    click.propagate(false);
}

fn sync_search(
    fields: Query<&EditableText, With<StorageSearchField>>,
    mut placeholders: Query<&mut Visibility, With<StorageSearchPlaceholder>>,
    mut ui: ResMut<StorageUi>,
) {
    let Ok(field) = fields.single() else {
        return;
    };
    let value = field.value().to_string();
    let normalized = value.trim().to_lowercase();
    if normalized != ui.query() {
        ui.set_query(&value);
    }
    let visible = if value.is_empty() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut visibility in &mut placeholders {
        *visibility = visible;
    }
}

fn sync_controls(
    mut commands: Commands,
    ui: Res<StorageUi>,
    mut categories: Query<
        (&StorageCategoryButton, &mut BackgroundColor),
        Without<StorageTransferButton>,
    >,
    mut transfers: Query<
        (Entity, &mut StorageTransferButton, &mut BackgroundColor),
        Without<StorageCategoryButton>,
    >,
    quick_transfers: Query<Entity, With<StorageQuickTransfer>>,
) {
    for (button, mut background) in &mut categories {
        background.0 = if button.0 == ui.category {
            theme::EMERALD_INK
        } else {
            theme::FIELD
        };
    }
    for (entity, mut button, mut background) in &mut transfers {
        button.enabled = !ui.awaiting_result
            && matches!(
                (button.direction, ui.selection),
                (
                    StorageTransferDirection::Deposit,
                    Some(StorageSelection::Bag(_))
                ) | (
                    StorageTransferDirection::Withdraw,
                    Some(StorageSelection::Vault(_))
                )
            );
        background.0 = if button.enabled {
            match button.direction {
                StorageTransferDirection::Deposit => theme::GOLD,
                StorageTransferDirection::Withdraw => theme::EMERALD,
            }
        } else {
            theme::FIELD
        };
        if button.enabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
    for entity in &quick_transfers {
        if ui.awaiting_result {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
}

fn rebuild_feedback(
    mut commands: Commands,
    ui: Res<StorageUi>,
    errors: Query<(Entity, Option<&Children>), With<StorageErrorHost>>,
    overlays: Query<(Entity, Option<&Children>), With<StorageOverlayHost>>,
) {
    if !ui.is_changed() {
        return;
    }
    let (Ok((error_host, error_children)), Ok((overlay_host, overlay_children))) =
        (errors.single(), overlays.single())
    else {
        return;
    };
    for children in [error_children, overlay_children].into_iter().flatten() {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if let (None, Some(error)) = (&ui.pending_transfer, &ui.panel_error) {
        commands
            .spawn_scene(scene::error_message(error.clone()))
            .insert(ChildOf(error_host));
    }
    if let Some(pending) = &ui.pending_transfer {
        commands
            .spawn_scene(scene::amount_overlay(
                pending.amount.clone(),
                ui.panel_error.clone(),
            ))
            .insert(ChildOf(overlay_host));
    }
}

fn apply_storage_results(
    mut results: MessageReader<StorageResult>,
    storage: Res<Storage>,
    mut ui: ResMut<StorageUi>,
) {
    for result in results.read() {
        if !storage.is_open() {
            warn!("ignoring Storage result while Storage is closed");
            continue;
        }
        if !ui.awaiting_result {
            warn!("ignoring Storage result without an in-flight request");
            continue;
        }
        ui.awaiting_result = false;
        ui.pending_transfer = None;
        ui.last_click = None;
        ui.panel_error = match result.outcome {
            Ok(()) => None,
            Err(rejection) => {
                if let StorageRejection::Unknown(code) = rejection {
                    warn!(code, "unknown Storage rejection code");
                }
                Some(rejection_message(rejection))
            }
        };
    }
}

fn clear_storage_focus(input_focus: &mut InputFocus, fields: &Query<(), StorageFieldFilter>) {
    if input_focus
        .get()
        .is_some_and(|entity| fields.contains(entity))
    {
        input_focus.clear();
    }
}

fn sync_window_visibility(
    storage: Res<Storage>,
    mut roots: Query<&mut Visibility, With<StorageWindowRoot>>,
    mut fields: Query<&mut EditableText, With<StorageSearchField>>,
    storage_fields: Query<(), StorageFieldFilter>,
    mut input_focus: ResMut<InputFocus>,
    mut ui: ResMut<StorageUi>,
) {
    let Ok(mut visibility) = roots.single_mut() else {
        return;
    };
    let open = storage.is_open();
    *visibility = if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if ui.previous_open != open {
        if let Ok(mut field) = fields.single_mut() {
            field.clear();
        }
        if !open {
            clear_storage_focus(&mut input_focus, &storage_fields);
        }
        *ui = StorageUi {
            previous_open: open,
            ..Default::default()
        };
    }
}

pub(crate) fn on_storage_close(
    _: On<Activate>,
    mut close: MessageWriter<CloseStorage>,
    mut roots: Query<&mut Visibility, With<StorageWindowRoot>>,
    mut fields: Query<&mut EditableText, With<StorageSearchField>>,
    storage_fields: Query<(), StorageFieldFilter>,
    mut input_focus: ResMut<InputFocus>,
    mut ui: ResMut<StorageUi>,
) {
    close.write(CloseStorage);
    if let Ok(mut root) = roots.single_mut() {
        *root = Visibility::Hidden;
    }
    if let Ok(mut field) = fields.single_mut() {
        field.clear();
    }
    clear_storage_focus(&mut input_focus, &storage_fields);
    *ui = StorageUi::default();
}

fn reset_storage_ui(mut ui: ResMut<StorageUi>) {
    *ui = StorageUi::default();
}

pub struct StorageWindowPlugin;

impl Plugin for StorageWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<StorageUi>()
            .add_systems(
                Update,
                (
                    sync_search,
                    apply_storage_results
                        .after(game_engine::domain::inventory::systems::apply_item_deltas)
                        .after(game_engine::domain::storage::systems::apply_storage_item_deltas)
                        .after(game_engine::domain::storage::systems::apply_storage_close),
                    rebuild_panes
                        .after(sync_search)
                        .after(apply_storage_results)
                        .after(game_engine::domain::inventory::systems::apply_item_deltas)
                        .after(game_engine::domain::storage::systems::apply_storage_opened)
                        .after(game_engine::domain::storage::systems::apply_storage_item_deltas),
                    sync_controls.after(rebuild_panes),
                    rebuild_feedback.after(rebuild_panes),
                    sync_window_visibility
                        .after(game_engine::domain::storage::systems::apply_storage_opened)
                        .after(game_engine::domain::storage::systems::apply_storage_close),
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), reset_storage_ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::{ItemData, ItemInfo};

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        for (id, name) in [(501, "Red Potion"), (2104, "Buckler")] {
            data.items.insert(
                id,
                ItemInfo {
                    identified_name: name.to_string(),
                    unidentified_name: format!("Unknown {name}"),
                    identified_resource: name.to_uppercase().replace(' ', "_"),
                    ..Default::default()
                },
            );
        }
        ItemDb::from_item_data(data)
    }

    fn storage_item(index: u32, nameid: u32, type_: u32, amount: u32) -> StorageItem {
        StorageItem {
            index,
            nameid,
            amount,
            type_,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight: 0,
            identified: true,
            cards: vec![],
        }
    }

    fn click_event(target: Entity, window: Entity) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;

        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary.normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
                duration: Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    #[test]
    fn plugin_initializes_presentation_state_only() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<bevy::shader::Shader>();

        app.add_plugins(StorageWindowPlugin);

        assert_eq!(app.world().resource::<StorageUi>(), &StorageUi::default());
        let ui = app.world().resource::<StorageUi>();
        assert_eq!(ui.category, StorageCategory::All);
        assert_eq!(ui.query, "");
        assert_eq!(ui.selection, None);
        assert_eq!(ui.pending_transfer, None);
        assert!(!ui.awaiting_result);
        assert_eq!(ui.panel_error, None);
        assert_eq!(ui.last_click, None);
        assert!(!ui.previous_open);
    }

    #[test]
    fn search_query_is_trimmed_and_case_normalized() {
        let mut ui = StorageUi::default();

        ui.set_query("  Red POTION  ");

        assert_eq!(ui.query(), "red potion");
    }

    #[test]
    fn category_and_search_use_one_case_insensitive_predicate() {
        assert!(matches_filter(
            StorageCategory::All,
            "red potion",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(matches_filter(
            StorageCategory::Use,
            "POTION",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(!matches_filter(
            StorageCategory::Equip,
            "potion",
            ItemCategory::Use,
            "Red Potion"
        ));
        assert!(!matches_filter(
            StorageCategory::Use,
            "blue",
            ItemCategory::Use,
            "Red Potion"
        ));
    }

    #[test]
    fn bag_and_vault_share_projection_without_mutating_sources() {
        let db = item_db();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 1,
            item_id: 501,
            item_type: 0,
            identified: true,
            ..Default::default()
        });
        inventory.upsert(Item {
            index: 2,
            item_id: 501,
            item_type: 0,
            wear_state: 1,
            identified: true,
            ..Default::default()
        });
        inventory.upsert(Item {
            index: 3,
            item_id: 2104,
            item_type: 4,
            identified: true,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(
            100,
            vec![
                storage_item(70_000, 501, 0, 10),
                storage_item(70_001, 2104, 4, 1),
            ],
        );

        let bag = bag_projection(&inventory, &db, StorageCategory::Use, "POTION");
        let vault = vault_projection(&storage, &db, StorageCategory::Use, "POTION");

        assert_eq!(bag.iter().map(|item| item.index).collect::<Vec<_>>(), [1]);
        assert_eq!(
            vault.iter().map(|item| item.index).collect::<Vec<_>>(),
            [70_000]
        );
        assert_eq!(inventory.len(), 3);
        assert_eq!(storage.len(), 2);
        assert!(inventory.get(2).unwrap().is_equipped());
    }

    #[test]
    fn selection_is_cleared_when_missing_from_the_filtered_pane() {
        let bag_item = Item {
            index: 7,
            ..Default::default()
        };
        let vault_item = storage_item(70_000, 501, 0, 1);

        assert_eq!(
            validated_selection(Some(StorageSelection::Bag(7)), &[&bag_item], &[&vault_item]),
            Some(StorageSelection::Bag(7))
        );
        assert_eq!(
            validated_selection(Some(StorageSelection::Bag(8)), &[&bag_item], &[&vault_item]),
            None
        );
        assert_eq!(
            validated_selection(
                Some(StorageSelection::Vault(70_001)),
                &[&bag_item],
                &[&vault_item]
            ),
            None
        );
    }

    #[test]
    fn transfer_intent_preserves_direction_indices_and_amount() {
        assert_eq!(
            transfer_intent(StorageSelection::Bag(u16::MAX), 70_000),
            TransferIntent::Deposit {
                inventory_index: u16::MAX as u32,
                amount: 70_000,
            }
        );
        assert_eq!(
            transfer_intent(StorageSelection::Vault(70_001), 80_000),
            TransferIntent::Withdraw {
                storage_index: 70_001,
                amount: 80_000,
            }
        );
    }

    #[test]
    fn amount_validation_uses_the_live_source_stack() {
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 7,
            amount: 5,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(100, vec![storage_item(70_000, 501, 0, 4)]);

        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "5", &inventory, &storage),
            Ok(5)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Vault(70_000), "4", &inventory, &storage),
            Ok(4)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "", &inventory, &storage),
            Err(AmountValidationError::Empty)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "0", &inventory, &storage),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "five", &inventory, &storage),
            Err(AmountValidationError::NotNumber)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Bag(7), "6", &inventory, &storage),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(
            validate_live_amount(StorageSelection::Vault(9), "1", &inventory, &storage),
            Err(AmountValidationError::SourceMissing)
        );
    }

    #[test]
    fn every_storage_rejection_has_a_panel_message() {
        let cases = [
            (StorageRejection::Full, "Storage is full."),
            (StorageRejection::InventoryFull, "Your inventory is full."),
            (
                StorageRejection::Overweight,
                "You are carrying too much weight.",
            ),
            (StorageRejection::NotStorable, "This item cannot be stored."),
            (
                StorageRejection::ItemEquipped,
                "Equipped items cannot be stored.",
            ),
            (StorageRejection::InvalidAmount, "Enter a valid amount."),
            (StorageRejection::NotOpen, "Storage is not open."),
            (
                StorageRejection::BasicSkillRequired,
                "Basic Skill level 6 is required.",
            ),
        ];

        for (rejection, expected) in cases {
            assert_eq!(rejection_message(rejection), expected);
        }
        assert_eq!(
            rejection_message(StorageRejection::Unknown(-37)),
            "Storage request failed (code -37)."
        );
    }

    #[test]
    fn double_click_requires_the_same_selection_within_300ms() {
        let last = LastStorageClick {
            selection: StorageSelection::Vault(70_000),
            at: Duration::from_millis(100),
        };

        assert!(is_double_click(
            Some(last),
            StorageSelection::Vault(70_000),
            Duration::from_millis(400)
        ));
        assert!(!is_double_click(
            Some(last),
            StorageSelection::Vault(70_000),
            Duration::from_millis(401)
        ));
        assert!(!is_double_click(
            Some(last),
            StorageSelection::Bag(70),
            Duration::from_millis(200)
        ));
        assert!(!is_double_click(
            None,
            StorageSelection::Vault(70_000),
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn all_transfer_controls_follow_selection_and_awaiting_state() {
        let mut app = App::new();
        app.insert_resource(StorageUi {
            selection: Some(StorageSelection::Bag(7)),
            ..Default::default()
        });
        app.add_systems(Update, sync_controls);
        let deposit = app
            .world_mut()
            .spawn((
                StorageTransferButton {
                    direction: StorageTransferDirection::Deposit,
                    enabled: false,
                },
                BackgroundColor::default(),
            ))
            .id();
        let withdraw = app
            .world_mut()
            .spawn((
                StorageTransferButton {
                    direction: StorageTransferDirection::Withdraw,
                    enabled: false,
                },
                BackgroundColor::default(),
            ))
            .id();
        let quick_bag = app
            .world_mut()
            .spawn(StorageQuickTransfer(StorageSelection::Bag(7)))
            .id();
        let quick_vault = app
            .world_mut()
            .spawn(StorageQuickTransfer(StorageSelection::Vault(70_000)))
            .id();

        app.update();
        assert!(
            app.world()
                .get::<StorageTransferButton>(deposit)
                .unwrap()
                .enabled
        );
        assert!(
            !app.world()
                .get::<StorageTransferButton>(withdraw)
                .unwrap()
                .enabled
        );
        assert!(app.world().get::<InteractionDisabled>(deposit).is_none());
        assert!(app.world().get::<InteractionDisabled>(withdraw).is_some());
        assert!(app.world().get::<InteractionDisabled>(quick_bag).is_none());
        assert!(app
            .world()
            .get::<InteractionDisabled>(quick_vault)
            .is_none());

        app.world_mut().resource_mut::<StorageUi>().awaiting_result = true;
        app.update();
        assert!(
            !app.world()
                .get::<StorageTransferButton>(deposit)
                .unwrap()
                .enabled
        );
        assert!(app.world().get::<InteractionDisabled>(deposit).is_some());
        assert!(app.world().get::<InteractionDisabled>(quick_bag).is_some());
        assert!(app
            .world()
            .get::<InteractionDisabled>(quick_vault)
            .is_some());
    }

    #[test]
    fn directional_single_item_emits_deposit_without_mutating_containers() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.insert_resource(StorageUi {
            selection: Some(StorageSelection::Bag(7)),
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 1,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        let button = app
            .world_mut()
            .spawn(StorageTransferButton {
                direction: StorageTransferDirection::Deposit,
                enabled: true,
            })
            .observe(on_transfer_activate)
            .id();

        app.world_mut().trigger(Activate { entity: button });

        let deposits: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<DepositStorageItem>>()
            .drain()
            .collect();
        assert_eq!(deposits.len(), 1);
        assert_eq!(deposits[0].inventory_index, 7);
        assert_eq!(deposits[0].amount, 1);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<WithdrawStorageItem>>()
                .drain()
                .count(),
            0
        );
        assert!(app.world().resource::<StorageUi>().awaiting_result);
        assert!(app
            .world()
            .resource::<StorageUi>()
            .pending_transfer
            .is_none());
        assert_eq!(
            app.world().resource::<Inventory>().get(7).unwrap().amount,
            1
        );
        assert!(app.world().resource::<Storage>().is_empty());
    }

    #[test]
    fn directional_stack_opens_amount_prompt_at_one_without_command() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.insert_resource(StorageUi {
            selection: Some(StorageSelection::Bag(7)),
            panel_error: Some("old error".to_string()),
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 5,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        let button = app
            .world_mut()
            .spawn(StorageTransferButton {
                direction: StorageTransferDirection::Deposit,
                enabled: true,
            })
            .observe(on_transfer_activate)
            .id();

        app.world_mut().trigger(Activate { entity: button });

        assert_eq!(
            app.world().resource::<StorageUi>().pending_transfer,
            Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "1".to_string(),
            })
        );
        assert_eq!(app.world().resource::<StorageUi>().panel_error, None);
        assert!(!app.world().resource::<StorageUi>().awaiting_result);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn quick_transfer_uses_its_cell_source_for_withdrawal() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Inventory>();
        app.insert_resource(StorageUi::default());
        let mut storage = Storage::default();
        storage.open(600, vec![storage_item(70_000, 501, 0, 1)]);
        app.insert_resource(storage);
        let button = app
            .world_mut()
            .spawn(StorageQuickTransfer(StorageSelection::Vault(70_000)))
            .observe(on_quick_transfer_activate)
            .id();

        app.world_mut().trigger(Activate { entity: button });

        let withdrawals: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<WithdrawStorageItem>>()
            .drain()
            .collect();
        assert_eq!(withdrawals.len(), 1);
        assert_eq!(withdrawals[0].storage_index, 70_000);
        assert_eq!(withdrawals[0].amount, 1);
        assert!(app.world().resource::<StorageUi>().awaiting_result);
        assert_eq!(
            app.world()
                .resource::<Storage>()
                .get(70_000)
                .unwrap()
                .amount,
            1
        );
    }

    #[test]
    fn cell_double_click_uses_the_shared_transfer_path() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Time>();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.init_resource::<StorageUi>();
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 1,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        let window = app.world_mut().spawn(Window::default()).id();
        let cell = app
            .world_mut()
            .spawn(StorageCell(StorageSelection::Bag(7)))
            .observe(on_cell_select)
            .id();

        app.world_mut().trigger(click_event(cell, window));
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
        app.world_mut().trigger(click_event(cell, window));

        let deposits: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<DepositStorageItem>>()
            .drain()
            .collect();
        assert_eq!(deposits.len(), 1);
        assert_eq!(deposits[0].inventory_index, 7);
        assert_eq!(deposits[0].amount, 1);
        let ui = app.world().resource::<StorageUi>();
        assert!(ui.awaiting_result);
        assert_eq!(ui.last_click, None);
    }

    #[test]
    fn awaiting_result_blocks_direction_quick_transfer_and_double_click() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Time>();
        app.init_resource::<Inventory>();
        app.insert_resource(StorageUi {
            selection: Some(StorageSelection::Bag(7)),
            awaiting_result: true,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 1,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(600, vec![storage_item(70_000, 501, 0, 1)]);
        app.insert_resource(storage);
        let direction = app
            .world_mut()
            .spawn(StorageTransferButton {
                direction: StorageTransferDirection::Deposit,
                enabled: true,
            })
            .observe(on_transfer_activate)
            .id();
        let quick = app
            .world_mut()
            .spawn(StorageQuickTransfer(StorageSelection::Vault(70_000)))
            .observe(on_quick_transfer_activate)
            .id();
        let window = app.world_mut().spawn(Window::default()).id();
        let cell = app
            .world_mut()
            .spawn(StorageCell(StorageSelection::Bag(7)))
            .observe(on_cell_select)
            .id();

        app.world_mut().trigger(Activate { entity: direction });
        app.world_mut().trigger(Activate { entity: quick });
        app.world_mut().trigger(click_event(cell, window));
        app.world_mut().trigger(click_event(cell, window));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<WithdrawStorageItem>>()
                .drain()
                .count(),
            0
        );
        let ui = app.world().resource::<StorageUi>();
        assert!(ui.awaiting_result);
        assert!(ui.pending_transfer.is_none());
    }

    #[test]
    fn awaiting_click_then_result_does_not_turn_next_single_click_into_double_click() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.add_message::<StorageResult>();
        app.init_resource::<Time>();
        app.init_resource::<Inventory>();
        app.insert_resource(StorageUi {
            awaiting_result: true,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 1,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(600, vec![]);
        app.insert_resource(storage);
        app.add_systems(Update, apply_storage_results);
        let window = app.world_mut().spawn(Window::default()).id();
        let cell = app
            .world_mut()
            .spawn(StorageCell(StorageSelection::Bag(7)))
            .observe(on_cell_select)
            .id();

        app.world_mut().trigger(click_event(cell, window));
        assert_eq!(app.world().resource::<StorageUi>().last_click, None);
        app.world_mut()
            .write_message(StorageResult { outcome: Ok(()) });
        app.update();
        app.world_mut().trigger(click_event(cell, window));

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(
            app.world().resource::<StorageUi>().last_click,
            Some(LastStorageClick {
                selection: StorageSelection::Bag(7),
                at: Duration::ZERO,
            })
        );
    }

    #[test]
    fn invalid_confirmation_stays_in_prompt_and_emits_no_command() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.init_resource::<InputFocus>();
        app.insert_resource(StorageUi {
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "1".to_string(),
            }),
            ..Default::default()
        });
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 7,
            amount: 5,
            ..Default::default()
        });
        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        app.world_mut()
            .spawn((StorageAmountField, EditableText::new("6")));
        let confirm = app
            .world_mut()
            .spawn_empty()
            .observe(on_amount_confirm)
            .id();

        app.world_mut().trigger(Activate { entity: confirm });

        assert_eq!(
            app.world().resource::<StorageUi>().pending_transfer,
            Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "6".to_string(),
            })
        );
        assert_eq!(
            app.world().resource::<StorageUi>().panel_error.as_deref(),
            Some("Enter an amount within the available stack.")
        );
        assert!(!app.world().resource::<StorageUi>().awaiting_result);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn valid_confirmation_emits_exact_live_amount_and_releases_focus() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.init_resource::<Inventory>();
        app.insert_resource(StorageUi {
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Vault(70_000),
                amount: "1".to_string(),
            }),
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(600, vec![storage_item(70_000, 501, 0, 4)]);
        app.insert_resource(storage);
        let field = app
            .world_mut()
            .spawn((StorageAmountField, EditableText::new("3")))
            .id();
        app.insert_resource(InputFocus::from_entity(field));
        let confirm = app
            .world_mut()
            .spawn_empty()
            .observe(on_amount_confirm)
            .id();

        app.world_mut().trigger(Activate { entity: confirm });

        let withdrawals: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<WithdrawStorageItem>>()
            .drain()
            .collect();
        assert_eq!(withdrawals.len(), 1);
        assert_eq!(withdrawals[0].storage_index, 70_000);
        assert_eq!(withdrawals[0].amount, 3);
        let ui = app.world().resource::<StorageUi>();
        assert!(ui.pending_transfer.is_none());
        assert!(ui.awaiting_result);
        assert_eq!(ui.panel_error, None);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(
            app.world()
                .resource::<Storage>()
                .get(70_000)
                .unwrap()
                .amount,
            4
        );
    }

    #[test]
    fn cancelling_amount_prompt_emits_nothing_and_releases_focus() {
        let mut app = App::new();
        app.add_message::<DepositStorageItem>();
        app.add_message::<WithdrawStorageItem>();
        app.insert_resource(StorageUi {
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "3".to_string(),
            }),
            panel_error: Some("Enter an amount.".to_string()),
            ..Default::default()
        });
        let field = app
            .world_mut()
            .spawn((StorageAmountField, EditableText::new("3")))
            .id();
        app.insert_resource(InputFocus::from_entity(field));
        let cancel = app.world_mut().spawn_empty().observe(on_amount_cancel).id();

        app.world_mut().trigger(Activate { entity: cancel });

        let ui = app.world().resource::<StorageUi>();
        assert!(ui.pending_transfer.is_none());
        assert_eq!(ui.panel_error, None);
        assert!(!ui.awaiting_result);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<DepositStorageItem>>()
                .drain()
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<WithdrawStorageItem>>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn successful_result_releases_awaiting_state_without_mutating_containers() {
        let mut app = App::new();
        app.add_message::<StorageResult>();
        app.init_resource::<Inventory>();
        app.insert_resource(StorageUi {
            awaiting_result: true,
            panel_error: Some("old error".to_string()),
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(600, vec![storage_item(70_000, 501, 0, 4)]);
        app.insert_resource(storage);
        app.add_systems(Update, apply_storage_results);

        app.world_mut()
            .write_message(StorageResult { outcome: Ok(()) });
        app.update();

        let ui = app.world().resource::<StorageUi>();
        assert!(!ui.awaiting_result);
        assert_eq!(ui.panel_error, None);
        assert_eq!(
            app.world()
                .resource::<Storage>()
                .get(70_000)
                .unwrap()
                .amount,
            4
        );
        assert!(app.world().resource::<Inventory>().is_empty());
    }

    #[test]
    fn rejected_results_release_awaiting_and_show_known_or_unknown_errors() {
        let mut app = App::new();
        app.add_message::<StorageResult>();
        app.insert_resource(StorageUi::default());
        let mut storage = Storage::default();
        storage.open(600, vec![]);
        app.insert_resource(storage);
        app.add_systems(Update, apply_storage_results);

        let cases = [
            (StorageRejection::Full, "Storage is full."),
            (StorageRejection::InventoryFull, "Your inventory is full."),
            (
                StorageRejection::Overweight,
                "You are carrying too much weight.",
            ),
            (StorageRejection::NotStorable, "This item cannot be stored."),
            (
                StorageRejection::ItemEquipped,
                "Equipped items cannot be stored.",
            ),
            (StorageRejection::InvalidAmount, "Enter a valid amount."),
            (StorageRejection::NotOpen, "Storage is not open."),
            (
                StorageRejection::BasicSkillRequired,
                "Basic Skill level 6 is required.",
            ),
            (
                StorageRejection::Unknown(-37),
                "Storage request failed (code -37).",
            ),
        ];

        for (rejection, expected) in cases {
            app.world_mut().resource_mut::<StorageUi>().awaiting_result = true;
            app.world_mut().write_message(StorageResult {
                outcome: Err(rejection),
            });
            app.update();

            let ui = app.world().resource::<StorageUi>();
            assert!(!ui.awaiting_result);
            assert_eq!(ui.panel_error.as_deref(), Some(expected));
        }
    }

    #[test]
    fn result_received_while_closed_cannot_leak_into_reopen() {
        let mut app = App::new();
        app.add_message::<StorageResult>();
        app.init_resource::<Storage>();
        app.init_resource::<InputFocus>();
        app.insert_resource(StorageUi {
            awaiting_result: true,
            panel_error: Some("existing".to_string()),
            ..Default::default()
        });
        app.world_mut()
            .spawn((StorageWindowRoot, Visibility::Hidden));
        app.add_systems(
            Update,
            (apply_storage_results, sync_window_visibility).chain(),
        );

        app.world_mut().write_message(StorageResult {
            outcome: Err(StorageRejection::Full),
        });
        app.update();
        assert!(app.world().resource::<StorageUi>().awaiting_result);
        assert_eq!(
            app.world().resource::<StorageUi>().panel_error.as_deref(),
            Some("existing")
        );

        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        app.update();
        let ui = app.world().resource::<StorageUi>();
        assert!(!ui.awaiting_result);
        assert_eq!(ui.panel_error, None);
        assert!(ui.previous_open);
    }

    #[test]
    fn leaving_gameplay_cancels_prompt_and_awaiting_state() {
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_resource::<StorageUi>();
        app.add_systems(OnExit(GameState::InGame), reset_storage_ui);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        *app.world_mut().resource_mut::<StorageUi>() = StorageUi {
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "3".to_string(),
            }),
            awaiting_result: true,
            panel_error: Some("error".to_string()),
            previous_open: true,
            ..Default::default()
        };

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Login);
        app.update();

        assert_eq!(app.world().resource::<StorageUi>(), &StorageUi::default());
    }

    #[test]
    fn same_frame_container_deltas_reach_both_rendered_panes() {
        use bevy::scene::ScenePlugin;
        use net_contract::events::{ItemAdded, ItemRemoved, StorageItemAdded, StorageItemRemoved};

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app.add_message::<ItemAdded>();
        app.add_message::<ItemRemoved>();
        app.add_message::<StorageItemAdded>();
        app.add_message::<StorageItemRemoved>();
        app.init_resource::<Inventory>();
        app.init_resource::<StorageUi>();
        let mut storage = Storage::default();
        storage.open(600, vec![]);
        app.insert_resource(storage);
        app.insert_resource(item_db());
        app.add_systems(
            Update,
            (
                game_engine::domain::inventory::systems::apply_item_deltas,
                game_engine::domain::storage::systems::apply_storage_item_deltas,
                rebuild_panes
                    .after(game_engine::domain::inventory::systems::apply_item_deltas)
                    .after(game_engine::domain::storage::systems::apply_storage_item_deltas),
            ),
        );
        app.world_mut().spawn(StorageBagHost);
        app.world_mut().spawn(StorageVaultHost);
        app.world_mut().write_message(ItemAdded {
            index: 7,
            amount: 2,
            nameid: 501,
            identified: true,
            attribute: 0,
            refine: 0,
            cards: vec![],
            location: 0,
            type_: 0,
            result: 0,
            expire_time: 0,
            look: 0,
        });
        app.world_mut().write_message(StorageItemAdded {
            item: storage_item(70_000, 501, 0, 3),
        });

        app.update();

        let rendered: Vec<_> = app
            .world_mut()
            .query::<&StorageCell>()
            .iter(app.world())
            .map(|cell| cell.0)
            .collect();
        assert!(rendered.contains(&StorageSelection::Bag(7)));
        assert!(rendered.contains(&StorageSelection::Vault(70_000)));
    }

    #[test]
    fn unrelated_pane_rebuild_preserves_typed_amount_entity_and_focus() {
        use bevy::scene::ScenePlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 7,
            item_id: 501,
            item_type: 0,
            amount: 5,
            identified: true,
            ..Default::default()
        });
        app.insert_resource(inventory);
        let mut storage = Storage::default();
        storage.open(600, vec![]);
        app.insert_resource(storage);
        app.insert_resource(StorageUi {
            selection: Some(StorageSelection::Bag(7)),
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "1".to_string(),
            }),
            previous_open: true,
            ..Default::default()
        });
        app.insert_resource(item_db());
        app.add_systems(
            Update,
            (rebuild_panes, rebuild_feedback.after(rebuild_panes)),
        );
        app.world_mut().spawn(StorageBagHost);
        app.world_mut().spawn(StorageVaultHost);
        app.world_mut().spawn(StorageErrorHost);
        app.world_mut().spawn(StorageOverlayHost);
        app.update();
        let amount = app
            .world_mut()
            .query_filtered::<Entity, With<StorageAmountField>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<EditableText>(amount)
            .unwrap()
            .editor_mut()
            .set_text("3");
        app.insert_resource(InputFocus::from_entity(amount));

        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 8,
            item_id: 501,
            item_type: 0,
            amount: 1,
            identified: true,
            ..Default::default()
        });
        app.update();

        let amount_after = app
            .world_mut()
            .query_filtered::<Entity, With<StorageAmountField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(amount_after, amount);
        assert_eq!(
            app.world()
                .get::<EditableText>(amount_after)
                .unwrap()
                .value(),
            "3"
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(amount));
    }

    #[test]
    fn close_hides_shell_resets_ui_and_emits_command() {
        let mut app = App::new();
        app.add_message::<CloseStorage>();
        app.insert_resource(StorageUi {
            category: StorageCategory::Equip,
            pending_transfer: Some(PendingTransfer {
                source: StorageSelection::Bag(7),
                amount: "3".to_string(),
            }),
            awaiting_result: true,
            panel_error: Some("error".to_string()),
            ..Default::default()
        });
        let root = app
            .world_mut()
            .spawn((StorageWindowRoot, Visibility::Inherited))
            .id();
        let search = app
            .world_mut()
            .spawn((StorageSearchField, EditableText::new("potion")))
            .id();
        app.insert_resource(InputFocus::from_entity(search));
        let button = app.world_mut().spawn_empty().observe(on_storage_close).id();

        app.world_mut().trigger(Activate { entity: button });

        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<CloseStorage>>()
                .drain()
                .count(),
            1
        );
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<StorageUi>(), &StorageUi::default());
    }

    #[test]
    fn authoritative_storage_open_state_drives_shell_visibility() {
        let mut app = App::new();
        app.init_resource::<Storage>();
        app.init_resource::<StorageUi>();
        app.init_resource::<InputFocus>();
        app.add_systems(Update, sync_window_visibility);
        let root = app
            .world_mut()
            .spawn((StorageWindowRoot, Visibility::Hidden))
            .id();

        app.world_mut().resource_mut::<Storage>().open(600, vec![]);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Inherited)
        );
        assert!(app.world().resource::<StorageUi>().previous_open);

        let amount = app
            .world_mut()
            .spawn((StorageAmountField, EditableText::new("1")))
            .id();
        app.insert_resource(InputFocus::from_entity(amount));

        app.world_mut().resource_mut::<Storage>().close();
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(root),
            Some(&Visibility::Hidden)
        );
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(app.world().resource::<StorageUi>(), &StorageUi::default());
    }
}
