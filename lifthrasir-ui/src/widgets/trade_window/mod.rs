use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use bevy_feathers::{FeathersCorePlugin, FeathersPlugins};
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::components::status::CharacterStatus;
use game_engine::domain::entities::markers::LocalPlayer;
use game_engine::domain::inventory::{Inventory, Item};
use game_engine::domain::trade::TradeSession;
use game_engine::infrastructure::item::ItemDb;
use net_contract::commands::{
    AddTradeItem, CancelTrade, ConfirmTrade, LockTrade, RemoveTradeItem, SetTradeZeny,
};

use crate::theme::feathers_theme::install_norse_theme;

pub mod feedback;
pub mod request_dialog;
pub mod scene;
pub mod slash;

pub use request_dialog::PendingTradeRequest;
pub use slash::TradeSlashSubmitted;

#[derive(Resource, Default)]
pub struct TradeUi {
    pub amount_prompt: Option<AmountPrompt>,
    pub zeny_draft: String,
    pub error: Option<&'static str>,
    previous_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmountPrompt {
    pub index: u16,
    pub max: u16,
}

#[derive(Component, Default, Clone)]
pub struct TradeWindowRoot;
#[derive(Component, Default, Clone)]
pub struct TradeWindowTitle;
#[derive(Component, Default, Clone)]
pub struct TradeBagHost;
#[derive(Component, Default, Clone)]
pub struct TradeOwnHost;
#[derive(Component, Default, Clone)]
pub struct TradePartnerHost;
#[derive(Component, Default, Clone, Copy)]
pub struct TradeBagCell(pub u16);
#[derive(Component, Default, Clone, Copy)]
pub struct TradeOwnCell(pub u32);
#[derive(Component, Default, Clone)]
pub struct TradeZenyField;
#[derive(Component, Default, Clone)]
pub struct TradeAmountField;
#[derive(Component, Default, Clone)]
pub struct TradeAmountConfirm;
#[derive(Component, Default, Clone)]
pub struct TradeAmountCancel;
#[derive(Component, Default, Clone)]
pub struct TradeLockButton;
#[derive(Component, Default, Clone)]
pub struct TradeConfirmButton;
#[derive(Component, Default, Clone)]
pub struct TradeCancelButton;
#[derive(Component, Default, Clone)]
pub struct TradeErrorHost;
#[derive(Component, Default, Clone)]
pub struct TradeOverlayHost;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmountValidationError {
    Empty,
    NotNumber,
    OutOfRange,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZenyValidationError {
    NotNumber,
    OverBalance,
}

pub(crate) fn bag_row_enabled(item: &Item, session: &TradeSession) -> bool {
    session.current().is_some_and(|open| {
        !open.own_locked
            && !session.is_offered(u32::from(item.index))
            && !session.offer_full()
            && item.bound == 0
            && !item.is_equipped()
            && item.amount > 0
    })
}

pub(crate) fn validate_amount(draft: &str, max: u16) -> Result<u32, AmountValidationError> {
    let draft = draft.trim();
    if draft.is_empty() {
        return Err(AmountValidationError::Empty);
    }
    let amount = draft
        .parse::<u32>()
        .map_err(|_| AmountValidationError::NotNumber)?;
    if amount == 0 || amount > u32::from(max) {
        return Err(AmountValidationError::OutOfRange);
    }
    Ok(amount)
}

pub(crate) fn validate_zeny(draft: &str, balance: u32) -> Result<u64, ZenyValidationError> {
    if draft.trim().is_empty() {
        return Ok(0);
    }
    let amount = draft
        .trim()
        .parse::<u64>()
        .map_err(|_| ZenyValidationError::NotNumber)?;
    if amount > u64::from(balance) {
        return Err(ZenyValidationError::OverBalance);
    }
    Ok(amount)
}

pub(crate) fn amount_prompt_for(item: &Item) -> Option<AmountPrompt> {
    (item.amount > 1).then_some(AmountPrompt {
        index: item.index,
        max: item.amount,
    })
}

fn amount_error(error: AmountValidationError) -> &'static str {
    match error {
        AmountValidationError::Empty => "Enter an amount.",
        AmountValidationError::NotNumber => "Enter a valid number.",
        AmountValidationError::OutOfRange => "Enter an amount within the available stack.",
    }
}

fn zeny_error(error: ZenyValidationError) -> &'static str {
    match error {
        ZenyValidationError::NotNumber => "Enter a valid zeny amount.",
        ZenyValidationError::OverBalance => "You do not have that much zeny.",
    }
}

type TradeInputFields<'w, 's> =
    Query<'w, 's, (), Or<(With<TradeZenyField>, With<TradeAmountField>)>>;

fn clear_trade_focus(input_focus: &mut InputFocus, fields: &TradeInputFields<'_, '_>) {
    if input_focus
        .get()
        .is_some_and(|entity| fields.contains(entity))
    {
        input_focus.clear();
    }
}

fn sync_window_visibility(
    session: Res<TradeSession>,
    mut ui: ResMut<TradeUi>,
    mut root: Query<&mut Visibility, With<TradeWindowRoot>>,
    mut zeny: Query<&mut EditableText, With<TradeZenyField>>,
    fields: TradeInputFields,
    mut input_focus: ResMut<InputFocus>,
) {
    let Ok(mut root) = root.single_mut() else {
        return;
    };
    let open = session.is_open();
    *root = if open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if ui.previous_open != open {
        clear_trade_focus(&mut input_focus, &fields);
        if let Ok(mut field) = zeny.single_mut() {
            field.clear();
        }
        *ui = TradeUi {
            previous_open: open,
            ..default()
        };
    }
}

fn sync_title(session: Res<TradeSession>, mut title: Query<&mut Text, With<TradeWindowTitle>>) {
    let Some(open) = session.current() else {
        return;
    };
    for mut text in &mut title {
        let name = format!("Trade with {}", open.partner_name);
        if text.0 != name {
            text.0 = name;
        }
    }
}

fn rebuild_bag(
    mut commands: Commands,
    session: Res<TradeSession>,
    inventory: Res<Inventory>,
    item_db: Res<ItemDb>,
    host: Query<(Entity, Option<&Children>), With<TradeBagHost>>,
) {
    let Ok((host, children)) = host.single() else {
        return;
    };
    if !session.is_changed() && !inventory.is_changed() && children.is_some() {
        return;
    }
    if !session.is_open() {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::bag_header())
        .insert(ChildOf(host));
    for item in inventory.stackables() {
        commands
            .spawn_scene(scene::bag_cell(
                item,
                &item_db,
                bag_row_enabled(item, &session),
            ))
            .insert(ChildOf(host));
    }
}

fn rebuild_offers(
    mut commands: Commands,
    session: Res<TradeSession>,
    item_db: Res<ItemDb>,
    own_host: Query<(Entity, Option<&Children>), With<TradeOwnHost>>,
    partner_host: Query<(Entity, Option<&Children>), With<TradePartnerHost>>,
) {
    let (Some(open), Ok((own_host, own_children)), Ok((partner_host, partner_children))) =
        (session.current(), own_host.single(), partner_host.single())
    else {
        return;
    };
    if !session.is_changed() && own_children.is_some() && partner_children.is_some() {
        return;
    }
    for children in [own_children, partner_children].into_iter().flatten() {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands
        .spawn_scene(scene::pane_header(
            "Your Offer",
            open.own_zeny,
            open.own_locked,
        ))
        .insert(ChildOf(own_host));
    for item in &open.own {
        commands
            .spawn_scene(scene::offer_cell(
                item,
                &item_db,
                Some(item.index),
                !open.own_locked,
            ))
            .insert(ChildOf(own_host));
    }
    commands
        .spawn_scene(scene::pane_header(
            "Partner Offer",
            open.partner_zeny,
            open.partner_locked,
        ))
        .insert(ChildOf(partner_host));
    for item in &open.partner {
        commands
            .spawn_scene(scene::offer_cell(item, &item_db, None, false))
            .insert(ChildOf(partner_host));
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "sync independent trade controls in one pass"
)]
fn sync_controls(
    mut commands: Commands,
    session: Res<TradeSession>,
    inventory: Res<Inventory>,
    bag: Query<(Entity, &TradeBagCell)>,
    own: Query<Entity, With<TradeOwnCell>>,
    lock: Query<Entity, With<TradeLockButton>>,
    confirm: Query<Entity, With<TradeConfirmButton>>,
    zeny: Query<Entity, With<TradeZenyField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    let unlocked = session.current().is_some_and(|open| !open.own_locked);
    for (entity, cell) in &bag {
        set_disabled(
            &mut commands,
            entity,
            inventory
                .get(cell.0)
                .is_none_or(|item| !bag_row_enabled(item, &session)),
        );
    }
    for entity in &own {
        set_disabled(&mut commands, entity, !unlocked);
    }
    for entity in &lock {
        set_disabled(&mut commands, entity, !unlocked);
    }
    for entity in &zeny {
        set_disabled(&mut commands, entity, !unlocked);
        if !unlocked && input_focus.get() == Some(entity) {
            input_focus.clear();
        }
    }
    for entity in &confirm {
        set_disabled(&mut commands, entity, !session.can_confirm());
    }
}

fn set_disabled(commands: &mut Commands, entity: Entity, disabled: bool) {
    if disabled {
        commands.entity(entity).insert(InteractionDisabled);
    } else {
        commands.entity(entity).remove::<InteractionDisabled>();
    }
}

fn sync_amount_prompt(
    mut commands: Commands,
    ui: Res<TradeUi>,
    mut previous: Local<Option<AmountPrompt>>,
    host: Query<(Entity, Option<&Children>), With<TradeOverlayHost>>,
) {
    let Ok((host, children)) = host.single() else {
        return;
    };
    if *previous == ui.amount_prompt {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if let Some(prompt) = ui.amount_prompt {
        commands
            .spawn_scene(scene::amount_prompt(prompt))
            .insert(ChildOf(host));
    }
    *previous = ui.amount_prompt;
}

fn sync_error(
    mut commands: Commands,
    ui: Res<TradeUi>,
    mut previous: Local<Option<&'static str>>,
    host: Query<(Entity, Option<&Children>), With<TradeErrorHost>>,
) {
    let Ok((host, children)) = host.single() else {
        return;
    };
    if *previous == ui.error {
        return;
    }
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if let Some(error) = ui.error {
        commands
            .spawn_scene(scene::error_message(error))
            .insert(ChildOf(host));
    }
    *previous = ui.error;
}

pub(crate) fn on_bag_click(
    click: On<Pointer<Click>>,
    cells: Query<&TradeBagCell>,
    inventory: Res<Inventory>,
    session: Res<TradeSession>,
    mut ui: ResMut<TradeUi>,
    mut add: MessageWriter<AddTradeItem>,
) {
    if click.button != PointerButton::Primary || ui.amount_prompt.is_some() {
        return;
    }
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    let Some(item) = inventory
        .get(cell.0)
        .filter(|item| bag_row_enabled(item, &session))
    else {
        return;
    };
    ui.error = None;
    if let Some(prompt) = amount_prompt_for(item) {
        ui.amount_prompt = Some(prompt);
    } else {
        add.write(AddTradeItem {
            index: u32::from(item.index),
            amount: 1,
        });
    }
}

pub(crate) fn on_own_click(
    click: On<Pointer<Click>>,
    cells: Query<&TradeOwnCell>,
    session: Res<TradeSession>,
    mut remove: MessageWriter<RemoveTradeItem>,
) {
    if click.button != PointerButton::Primary
        || session.current().is_none_or(|open| open.own_locked)
    {
        return;
    }
    let Ok(cell) = cells.get(click.entity) else {
        return;
    };
    if session.is_offered(cell.0) {
        remove.write(RemoveTradeItem { index: cell.0 });
    }
}

pub(crate) fn on_amount_confirm(
    _: On<Activate>,
    session: Res<TradeSession>,
    inventory: Res<Inventory>,
    fields: Query<(Entity, &EditableText), With<TradeAmountField>>,
    mut ui: ResMut<TradeUi>,
    mut input_focus: ResMut<InputFocus>,
    mut add: MessageWriter<AddTradeItem>,
) {
    let (Some(prompt), Ok((field_id, field))) = (ui.amount_prompt, fields.single()) else {
        return;
    };
    let Some(item) = inventory
        .get(prompt.index)
        .filter(|item| bag_row_enabled(item, &session))
    else {
        ui.error = Some("The source item is no longer available.");
        return;
    };
    match validate_amount(&field.value().to_string(), item.amount) {
        Ok(amount) => {
            add.write(AddTradeItem {
                index: u32::from(item.index),
                amount,
            });
            ui.amount_prompt = None;
            ui.error = None;
            if input_focus.get() == Some(field_id) {
                input_focus.clear();
            }
        }
        Err(error) => ui.error = Some(amount_error(error)),
    }
}

pub(crate) fn on_amount_cancel(
    _: On<Activate>,
    mut ui: ResMut<TradeUi>,
    fields: Query<(), With<TradeAmountField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    ui.amount_prompt = None;
    ui.error = None;
    if input_focus
        .get()
        .is_some_and(|entity| fields.contains(entity))
    {
        input_focus.clear();
    }
}

pub(crate) fn on_lock(
    _: On<Activate>,
    session: Res<TradeSession>,
    status: Query<&CharacterStatus, With<LocalPlayer>>,
    field: Query<&EditableText, With<TradeZenyField>>,
    mut ui: ResMut<TradeUi>,
    mut zeny: MessageWriter<SetTradeZeny>,
    mut lock: MessageWriter<LockTrade>,
) {
    let (Some(open), Ok(status), Ok(field)) = (session.current(), status.single(), field.single())
    else {
        return;
    };
    if open.own_locked {
        return;
    }
    ui.zeny_draft = field.value().to_string();
    match validate_zeny(&ui.zeny_draft, status.zeny) {
        Ok(amount) => {
            if amount != open.own_zeny {
                zeny.write(SetTradeZeny { amount });
            }
            lock.write(LockTrade);
            ui.error = None;
        }
        Err(error) => ui.error = Some(zeny_error(error)),
    }
}

pub(crate) fn on_confirm(
    _: On<Activate>,
    mut session: ResMut<TradeSession>,
    mut confirm: MessageWriter<ConfirmTrade>,
) {
    if session.can_confirm() {
        session.mark_confirm_sent();
        confirm.write(ConfirmTrade);
    }
}

pub(crate) fn on_cancel(
    _: On<Activate>,
    session: Res<TradeSession>,
    mut cancel: MessageWriter<CancelTrade>,
) {
    if session.is_open() {
        cancel.write(CancelTrade);
    }
}

fn reset_trade_ui(mut ui: ResMut<TradeUi>) {
    *ui = TradeUi::default();
}

pub struct TradeWindowPlugin;

impl Plugin for TradeWindowPlugin {
    fn build(&self, app: &mut App) {
        install_norse_theme(app);
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersPlugins);
        }
        app.init_resource::<PendingTradeRequest>()
            .init_resource::<TradeUi>()
            .add_message::<TradeSlashSubmitted>()
            .add_systems(
                Update,
                (
                    request_dialog::show_incoming_request,
                    request_dialog::claim_request_choice,
                    request_dialog::expire_pending_request,
                    request_dialog::clear_on_trade_opened,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_window_visibility
                        .after(game_engine::domain::trade::systems::apply_trade_opened)
                        .after(game_engine::domain::trade::systems::apply_trade_ended),
                    sync_title.after(sync_window_visibility),
                    rebuild_bag
                        .after(sync_window_visibility)
                        .after(game_engine::domain::trade::systems::apply_trade_offer)
                        .after(game_engine::domain::inventory::systems::apply_item_deltas),
                    rebuild_offers
                        .after(sync_window_visibility)
                        .after(game_engine::domain::trade::systems::apply_trade_offer),
                    sync_controls
                        .after(rebuild_bag)
                        .after(rebuild_offers)
                        .after(game_engine::domain::trade::systems::apply_trade_ended),
                    sync_amount_prompt.after(sync_window_visibility),
                    sync_error.after(sync_window_visibility),
                    feedback::ingest_trade_feedback,
                    slash::dispatch_trade_slash,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                (reset_trade_ui, request_dialog::reset_pending_request),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_and_zeny_validation_reject_out_of_range_inputs() {
        assert_eq!(validate_amount("2", 3), Ok(2));
        assert_eq!(validate_amount("", 3), Err(AmountValidationError::Empty));
        assert_eq!(
            validate_amount("no", 3),
            Err(AmountValidationError::NotNumber)
        );
        assert_eq!(
            validate_amount("0", 3),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(
            validate_amount("4", 3),
            Err(AmountValidationError::OutOfRange)
        );
        assert_eq!(validate_zeny("", 50), Ok(0));
        assert_eq!(validate_zeny("50", 50), Ok(50));
        assert_eq!(
            validate_zeny("garbage", 50),
            Err(ZenyValidationError::NotNumber)
        );
        assert_eq!(
            validate_zeny("51", 50),
            Err(ZenyValidationError::OverBalance)
        );
        assert_eq!(
            validate_zeny("18446744073709551616", 50),
            Err(ZenyValidationError::NotNumber)
        );
    }

    fn item(index: u16, amount: u16) -> Item {
        Item {
            index,
            item_id: 501,
            amount,
            identified: true,
            ..default()
        }
    }

    fn offer(
        own_locked: bool,
        partner_locked: bool,
        count: usize,
    ) -> net_contract::events::TradeOfferUpdated {
        let own = (0..count)
            .map(|i| net_contract::events::ZoneInventoryItem {
                index: u32::try_from(i).unwrap(),
                nameid: 501,
                type_: 0,
                amount: 1,
                location: 0,
                identified: true,
                attribute: 0,
                refine: 0,
                cards: vec![],
                expire_time: 0,
                bound: 0,
                favorite: false,
                look: 0,
            })
            .collect();
        net_contract::events::TradeOfferUpdated {
            own,
            partner: vec![],
            own_zeny: 0,
            partner_zeny: 0,
            own_locked,
            partner_locked,
        }
    }

    #[test]
    fn bag_row_guards_locked_offered_full_bound_and_equipped() {
        let mut session = TradeSession::default();
        let mut source = item(7, 2);
        assert!(!bag_row_enabled(&source, &session));
        session.open(10, "Bob".into());
        assert!(bag_row_enabled(&source, &session));
        session.apply_offer(&offer(false, false, 1));
        source.index = 0;
        assert!(!bag_row_enabled(&source, &session));
        source.index = 7;
        session.apply_offer(&offer(
            false,
            false,
            game_engine::domain::trade::MAX_OFFER_SLOTS,
        ));
        assert!(session.offer_full());
        assert!(!bag_row_enabled(&source, &session));
        session.apply_offer(&offer(false, false, 0));
        source.bound = 1;
        assert!(!bag_row_enabled(&source, &session));
        source.bound = 0;
        source.wear_state = 1;
        assert!(!bag_row_enabled(&source, &session));
        source.wear_state = 0;
        session.apply_offer(&offer(true, false, 0));
        assert!(!bag_row_enabled(&source, &session));
        assert_eq!(amount_prompt_for(&item(7, 1)), None);
        assert_eq!(
            amount_prompt_for(&item(7, 4)),
            Some(AmountPrompt { index: 7, max: 4 })
        );
    }

    fn test_app() -> App {
        use bevy::scene::ScenePlugin;
        use lifthrasir_data::{ItemData, ItemInfo};
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".into(),
                identified_resource: "RED_POTION".into(),
                ..default()
            },
        );
        app.insert_resource(ItemDb::from_item_data(data));
        app.init_resource::<TradeSession>()
            .init_resource::<TradeUi>()
            .init_resource::<InputFocus>()
            .init_resource::<Inventory>();
        app.add_message::<AddTradeItem>()
            .add_message::<SetTradeZeny>()
            .add_message::<LockTrade>()
            .add_message::<ConfirmTrade>()
            .add_message::<CancelTrade>()
            .add_message::<RemoveTradeItem>();
        app
    }

    #[test]
    fn shell_starts_hidden_and_visibility_tracks_authoritative_session() {
        let mut app = test_app();
        app.world_mut().spawn_scene(scene::window()).unwrap();
        app.add_systems(Update, (sync_window_visibility, sync_title).chain());
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<TradeWindowRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Hidden
        );
        let zeny_field = app
            .world_mut()
            .query_filtered::<Entity, With<TradeZenyField>>()
            .single(app.world())
            .unwrap();
        app.insert_resource(InputFocus::from_entity(zeny_field));
        app.world_mut().resource_mut::<TradeUi>().zeny_draft = "old".into();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert!(app.world().resource::<TradeUi>().zeny_draft.is_empty());
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );
        assert!(
            app.world_mut()
                .query_filtered::<&Text, With<TradeWindowTitle>>()
                .single(app.world())
                .unwrap()
                .0
                .contains("Bob")
        );
        app.insert_resource(InputFocus::from_entity(zeny_field));
        app.world_mut().resource_mut::<TradeSession>().close();
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Hidden
        );
        assert!(!app.world().resource::<TradeUi>().previous_open);
    }

    #[test]
    fn rows_follow_snapshots_and_controls_disable_after_lock() {
        let mut app = test_app();
        app.world_mut().spawn_scene(scene::window()).unwrap();
        app.add_systems(
            Update,
            (
                sync_window_visibility,
                rebuild_bag,
                rebuild_offers,
                sync_controls,
            )
                .chain(),
        );
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(item(7, 3));
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(10, "Bob".into());
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&TradeBagCell>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&TradeOwnCell>()
                .iter(app.world())
                .count(),
            0
        );
        let mut update = offer(false, false, 1);
        update.own[0].index = 7;
        update.partner = vec![net_contract::events::ZoneInventoryItem {
            index: 0,
            ..update.own[0].clone()
        }];
        app.world_mut()
            .resource_mut::<TradeSession>()
            .apply_offer(&update);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&TradeOwnCell>()
                .iter(app.world())
                .count(),
            1
        );
        let partner_host = app
            .world_mut()
            .query_filtered::<Entity, With<TradePartnerHost>>()
            .single(app.world())
            .unwrap();
        let partner: Vec<_> = app
            .world()
            .get::<Children>(partner_host)
            .unwrap()
            .iter()
            .collect();
        assert!(
            !partner
                .iter()
                .any(|entity| app.world().get::<TradeOwnCell>(*entity).is_some())
        );
        let cell = app
            .world_mut()
            .query_filtered::<Entity, With<TradeBagCell>>()
            .single(app.world())
            .unwrap();
        assert!(app.world().get::<InteractionDisabled>(cell).is_some());
        let confirm = app
            .world_mut()
            .query_filtered::<Entity, With<TradeConfirmButton>>()
            .single(app.world())
            .unwrap();
        assert!(app.world().get::<InteractionDisabled>(confirm).is_some());
        update.own_locked = true;
        update.partner_locked = true;
        let zeny_field = app
            .world_mut()
            .query_filtered::<Entity, With<TradeZenyField>>()
            .single(app.world())
            .unwrap();
        app.insert_resource(InputFocus::from_entity(zeny_field));
        app.world_mut()
            .resource_mut::<TradeSession>()
            .apply_offer(&update);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        let lock = app
            .world_mut()
            .query_filtered::<Entity, With<TradeLockButton>>()
            .single(app.world())
            .unwrap();
        let zeny = app
            .world_mut()
            .query_filtered::<Entity, With<TradeZenyField>>()
            .single(app.world())
            .unwrap();
        assert!(app.world().get::<InteractionDisabled>(lock).is_some());
        assert!(app.world().get::<InteractionDisabled>(zeny).is_some());
        assert!(app.world().get::<InteractionDisabled>(confirm).is_none());
    }

    fn click_event(target: Entity) -> Pointer<Click> {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::backend::HitData;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Primary
                        .normalize(Some(Entity::PLACEHOLDER))
                        .unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Click {
                button: PointerButton::Primary,
                hit: HitData::new(target, 0.0, None, None),
                duration: std::time::Duration::ZERO,
                count: 1,
            },
            target,
        )
    }

    #[test]
    fn bag_click_sends_single_item_or_prompts_for_stack_and_rechecks_bound() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        let cell = app
            .world_mut()
            .spawn(TradeBagCell(7))
            .observe(on_bag_click)
            .id();
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(item(7, 1));
        app.world_mut().trigger(click_event(cell));
        let sent: Vec<_> = app
            .world()
            .resource::<Messages<AddTradeItem>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!((sent[0].index, sent[0].amount), (7, 1));
        app.world_mut()
            .resource_mut::<Messages<AddTradeItem>>()
            .clear();
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(item(7, 5));
        app.world_mut().trigger(click_event(cell));
        assert_eq!(
            app.world().resource::<TradeUi>().amount_prompt,
            Some(AmountPrompt { index: 7, max: 5 })
        );
        assert!(app.world().resource::<Messages<AddTradeItem>>().is_empty());
        app.world_mut().resource_mut::<TradeUi>().amount_prompt = None;
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            bound: 1,
            ..item(7, 5)
        });
        app.world_mut().trigger(click_event(cell));
        assert!(app.world().resource::<TradeUi>().amount_prompt.is_none());
        assert!(app.world().resource::<Messages<AddTradeItem>>().is_empty());
    }

    #[test]
    fn own_click_only_removes_while_unlocked() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        let mut update = offer(false, false, 1);
        update.own[0].index = 7;
        app.world_mut()
            .resource_mut::<TradeSession>()
            .apply_offer(&update);
        let cell = app
            .world_mut()
            .spawn(TradeOwnCell(7))
            .observe(on_own_click)
            .id();
        app.world_mut().trigger(click_event(cell));
        let sent: Vec<_> = app
            .world()
            .resource::<Messages<RemoveTradeItem>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].index, 7);
        app.world_mut()
            .resource_mut::<Messages<RemoveTradeItem>>()
            .clear();
        update.own_locked = true;
        app.world_mut()
            .resource_mut::<TradeSession>()
            .apply_offer(&update);
        app.world_mut().trigger(click_event(cell));
        assert!(
            app.world()
                .resource::<Messages<RemoveTradeItem>>()
                .is_empty()
        );
    }

    #[test]
    fn amount_confirmation_validates_live_stack_and_keeps_prompt_on_error() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .upsert(item(7, 5));
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        app.world_mut().resource_mut::<TradeUi>().amount_prompt =
            Some(AmountPrompt { index: 7, max: 5 });
        let field = app
            .world_mut()
            .spawn((TradeAmountField, EditableText::new("6")))
            .id();
        let button = app
            .world_mut()
            .spawn(TradeAmountConfirm)
            .observe(on_amount_confirm)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        assert!(app.world().resource::<Messages<AddTradeItem>>().is_empty());
        assert!(app.world().resource::<TradeUi>().amount_prompt.is_some());
        assert_eq!(
            app.world().resource::<TradeUi>().error,
            Some("Enter an amount within the available stack.")
        );
        app.world_mut()
            .get_mut::<EditableText>(field)
            .unwrap()
            .editor_mut()
            .set_text("3");
        app.world_mut().trigger(Activate { entity: button });
        let added: Vec<_> = app
            .world()
            .resource::<Messages<AddTradeItem>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!((added[0].index, added[0].amount), (7, 3));
        assert!(app.world().resource::<TradeUi>().amount_prompt.is_none());
    }

    #[test]
    fn lock_sends_zeny_before_lock_only_if_balance_allows_it() {
        use game_engine::domain::entities::character::components::status::CharacterStatus;
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        let field = app
            .world_mut()
            .spawn((TradeZenyField, EditableText::new("45")))
            .id();
        app.world_mut().spawn((
            LocalPlayer,
            CharacterStatus {
                zeny: 100,
                ..default()
            },
        ));
        app.world_mut().spawn(TradeLockButton).observe(on_lock);
        let button = app
            .world_mut()
            .query_filtered::<Entity, With<TradeLockButton>>()
            .single(app.world())
            .unwrap();
        app.world_mut().trigger(Activate { entity: button });
        let zeny: Vec<_> = app
            .world()
            .resource::<Messages<SetTradeZeny>>()
            .iter_current_update_messages()
            .collect();
        assert_eq!(zeny.len(), 1);
        assert_eq!(zeny[0].amount, 45);
        assert_eq!(app.world().resource::<Messages<LockTrade>>().len(), 1);
        app.world_mut()
            .resource_mut::<Messages<SetTradeZeny>>()
            .clear();
        app.world_mut()
            .resource_mut::<Messages<LockTrade>>()
            .clear();
        app.world_mut()
            .get_mut::<EditableText>(field)
            .unwrap()
            .editor_mut()
            .set_text("101");
        app.world_mut().trigger(Activate { entity: button });
        assert!(app.world().resource::<Messages<SetTradeZeny>>().is_empty());
        assert!(app.world().resource::<Messages<LockTrade>>().is_empty());
        assert_eq!(
            app.world().resource::<TradeUi>().error,
            Some("You do not have that much zeny.")
        );
        app.world_mut()
            .get_mut::<EditableText>(field)
            .unwrap()
            .editor_mut()
            .set_text("");
        app.world_mut().trigger(Activate { entity: button });
        assert!(app.world().resource::<Messages<SetTradeZeny>>().is_empty());
        assert_eq!(app.world().resource::<Messages<LockTrade>>().len(), 1);
    }

    #[test]
    fn confirm_only_once_and_cancel_waits_for_server_close() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<TradeSession>()
            .open(1, "Bob".into());
        app.world_mut()
            .resource_mut::<TradeSession>()
            .apply_offer(&offer(true, true, 0));
        let confirm = app
            .world_mut()
            .spawn(TradeConfirmButton)
            .observe(on_confirm)
            .id();
        let cancel = app
            .world_mut()
            .spawn(TradeCancelButton)
            .observe(on_cancel)
            .id();
        app.world_mut().trigger(Activate { entity: confirm });
        app.world_mut().trigger(Activate { entity: confirm });
        assert_eq!(app.world().resource::<Messages<ConfirmTrade>>().len(), 1);
        app.world_mut().trigger(Activate { entity: cancel });
        assert_eq!(app.world().resource::<Messages<CancelTrade>>().len(), 1);
        assert!(app.world().resource::<TradeSession>().is_open());
    }
}
