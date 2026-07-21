use bevy::input_focus::AutoFocus;
use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{EditableText, EditableTextFilter, FontSize, FontSourceTemplate};
use bevy::ui_widgets::{ControlOrientation, ScrollArea};
use bevy_feathers::controls::{FeathersButton, FeathersScrollbar};
use bevy_feathers::theme::{ThemeBackgroundColor, ThemeBorderColor};
use game_engine::domain::assets::item_icon_path;
use game_engine::domain::inventory::{Inventory, ItemCategory};
use game_engine::domain::storage::Storage;
use game_engine::infrastructure::item::ItemDb;

use crate::theme;
use crate::theme::feathers_theme::{TOKEN_WINDOW_BG, TOKEN_WINDOW_BORDER};
use crate::widgets::chrome::{chrome_text, drag_window, glyph_icon, ignore_picking};

use super::*;

const WINDOW_LEFT: f32 = 210.0;
const WINDOW_TOP: f32 = 90.0;
const WINDOW_WIDTH: f32 = 724.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageCellView {
    pub selection: StorageSelection,
    pub icon: String,
    pub name: String,
    pub category: &'static str,
    pub amount: u32,
    pub refine: u32,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoragePaneView {
    pub title: &'static str,
    pub subtitle: String,
    pub cells: Vec<StorageCellView>,
    pub empty_message: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) enum PaneSide {
    Bag,
    Vault,
}

fn category_label(category: ItemCategory) -> &'static str {
    match category {
        ItemCategory::Use => "Use",
        ItemCategory::Etc => "Etc",
        ItemCategory::Equip => "Equip",
    }
}

pub(crate) fn pane_views(
    inventory: &Inventory,
    storage: &Storage,
    ui: &StorageUi,
    item_db: &ItemDb,
) -> (StoragePaneView, StoragePaneView) {
    let empty_message = if ui.query().is_empty() {
        "Nothing here yet."
    } else {
        "Nothing matches your search."
    };
    let bag = bag_projection(inventory, item_db, ui.category, ui.query())
        .into_iter()
        .map(|item| StorageCellView {
            selection: StorageSelection::Bag(item.index),
            icon: item_icon_path(
                item_db
                    .icon_resource(item.item_id, item.identified)
                    .expect("Storage window item must have an icon resource"),
            ),
            name: item_db
                .name(item.item_id, item.identified)
                .expect("Storage window item must have a name")
                .to_string(),
            category: category_label(item.category()),
            amount: u32::from(item.amount),
            refine: u32::from(item.refine),
            selected: ui.selection == Some(StorageSelection::Bag(item.index)),
        })
        .collect();
    let vault = vault_projection(storage, item_db, ui.category, ui.query())
        .into_iter()
        .map(|item| StorageCellView {
            selection: StorageSelection::Vault(item.index),
            icon: item_icon_path(
                item_db
                    .icon_resource(item.nameid, item.identified)
                    .expect("Storage window item must have an icon resource"),
            ),
            name: item_db
                .name(item.nameid, item.identified)
                .expect("Storage window item must have a name")
                .to_string(),
            category: category_label(item_category(item.type_)),
            amount: item.amount,
            refine: item.refine,
            selected: ui.selection == Some(StorageSelection::Vault(item.index)),
        })
        .collect();

    (
        StoragePaneView {
            title: "Your Bag",
            subtitle: format!("{} items", inventory.stackables().count()),
            cells: bag,
            empty_message,
        },
        StoragePaneView {
            title: "Storage Vault",
            subtitle: format!("{} / {}", storage.len(), storage.capacity()),
            cells: vault,
            empty_message,
        },
    )
}

pub(crate) fn pane(view: StoragePaneView, side: PaneSide) -> impl Scene {
    let cells: Vec<_> = view.cells.into_iter().map(cell).collect();
    let empty = cells
        .is_empty()
        .then(|| EntityScene(empty_state(view.empty_message, side)));
    let accent = match side {
        PaneSide::Bag => theme::EMERALD,
        PaneSide::Vault => theme::GOLD,
    };
    bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            min_width: px(0),
            border: px(1),
            border_radius: BorderRadius::all(px(10)),
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.2)})
        BorderColor::all(accent)
        ignore_picking()
        Children [
            pane_header(view.title, view.subtitle, side),
            (
                Node { height: px(268), position_type: PositionType::Relative }
                ignore_picking()
                Children [
                    (
                        #grid
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0), top: px(0), right: px(0), bottom: px(0),
                            overflow: {Overflow::scroll_y()},
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(6),
                            padding: {UiRect { left: Val::Px(10.0), right: Val::Px(14.0), top: Val::Px(10.0), bottom: Val::Px(10.0) }},
                        }
                        ScrollArea
                        Pickable
                        Children [ {cells}, {empty} ]
                    ),
                    @FeathersScrollbar { @target: #grid, @orientation: {ControlOrientation::Vertical} }
                    Node { position_type: PositionType::Absolute, right: px(3), top: px(4), bottom: px(4), width: px(6) }
                ]
            ),
        ]
    }
}

fn pane_header(title: &'static str, subtitle: String, side: PaneSide) -> impl Scene {
    let icon = match side {
        PaneSide::Bag => "bag",
        PaneSide::Vault => "vault",
    };
    bsn! {
        Node {
            height: px(54),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(9),
            padding: {UiRect::horizontal(px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            glyph_icon(icon, 17.0, theme::GOLD),
            chrome_text(title.to_string(), 13.0, theme::TEXT),
            (Node { flex_grow: 1.0 } ignore_picking()),
            chrome_text(subtitle, 10.0, theme::TEXT_DIM),
        ]
    }
}

fn cell(view: StorageCellView) -> impl Scene {
    let border = if view.selected {
        theme::EMERALD
    } else {
        theme::STROKE
    };
    let amount = (view.amount > 1).then(|| EntityScene(badge(view.amount.to_string(), false)));
    let refine = (view.refine > 0).then(|| EntityScene(badge(format!("+{}", view.refine), true)));
    let quick_icon = match view.selection {
        StorageSelection::Bag(_) => "chevr",
        StorageSelection::Vault(_) => "chevl",
    };
    bsn! {
        template_value(StorageCell(view.selection))
        Node {
            width: percent(100), height: px(52),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(8))},
            border: px(1),
            border_radius: BorderRadius::all(px(7)),
            overflow: {Overflow::clip()},
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(border)
        Pickable
        on(on_cell_select)
        Children [
            (
                ImageNode { image: {view.icon} }
                Node { width: px(38), height: px(38), flex_shrink: 0.0 }
                ignore_picking()
            ),
            (
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    overflow: {Overflow::clip()},
                }
                ignore_picking()
                Children [
                    chrome_text(view.name, 10.5, theme::TEXT_DIM),
                    chrome_text(view.category.to_string(), 8.5, theme::TEXT_FAINT),
                ]
            ),
            {refine}, {amount},
            (
                @FeathersButton { @caption: bsn! { glyph_icon(quick_icon, 10.0, theme::TEXT_DIM) } }
                template_value(StorageQuickTransfer(view.selection))
                Node { width: px(24), height: px(24), flex_shrink: 0.0 }
                on(stop_quick_transfer_propagation)
                on(on_quick_transfer_activate)
            ),
        ]
    }
}

fn badge(text: String, refine: bool) -> impl Scene {
    let color = if refine { theme::GOLD } else { theme::TEXT };
    bsn! {
        Text(text)
        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(9.0)} }
        TextColor(color)
        Node { min_width: px(18), flex_shrink: 0.0 }
        ignore_picking()
    }
}

fn empty_state(message: &'static str, side: PaneSide) -> impl Scene {
    let icon = match side {
        PaneSide::Bag => "bag",
        PaneSide::Vault => "vault",
    };
    bsn! {
        Node {
            width: percent(100), height: px(240),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(11),
        }
        ignore_picking()
        Children [
            glyph_icon(icon, 28.0, theme::TEXT_FAINT),
            chrome_text(message.to_string(), 11.0, theme::TEXT_FAINT),
        ]
    }
}

pub fn build(commands: &mut Commands, parent: Entity) {
    commands.spawn_scene(window()).insert(ChildOf(parent));
}

pub(crate) fn window() -> impl Scene {
    bsn! {
        StorageWindowRoot
        Node {
            position_type: PositionType::Absolute,
            left: px(WINDOW_LEFT),
            top: px(WINDOW_TOP),
            width: px(WINDOW_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            border: px(1),
            border_radius: BorderRadius::all(px(13)),
        }
        ThemeBackgroundColor({TOKEN_WINDOW_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Visibility::Hidden
        Pickable
        Children [
            storage_titlebar(),
            filter_strip(),
            panes_shell(),
            (
                StorageErrorHost
                Node { min_height: px(20), padding: {UiRect::horizontal(px(14))} }
                ignore_picking()
            ),
            footer(),
            (
                StorageOverlayHost
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0), right: px(0), top: px(0), bottom: px(0),
                }
                ignore_picking()
            ),
        ]
    }
}

fn storage_titlebar() -> impl Scene {
    bsn! {
        StorageWindowTitlebar
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBackgroundColor({crate::theme::feathers_theme::TOKEN_TITLEBAR_BG})
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        Pickable
        on(drag_window::<StorageWindowTitlebar, StorageWindowRoot>)
        Children [
            glyph_icon("vault", 16.0, theme::GOLD),
            (
                Text("Storage Vault")
                TextFont { font: FontSourceTemplate::Handle(theme::FONT_TITLE), font_size: {FontSize::Px(15.0)} }
                TextColor({theme::TEXT})
                Node { flex_grow: 1.0 }
                ignore_picking()
            ),
            (
                StorageCloseControl
                @FeathersButton { @caption: bsn! { glyph_icon("close", 13.0, theme::TEXT_DIM) } }
                Node { width: px(22), height: px(22) }
                on(on_storage_close)
            ),
        ]
    }
}

fn filter_strip() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(5),
            padding: {UiRect::axes(px(12), px(8))},
            border: {UiRect { bottom: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            category_button("All", StorageCategory::All, true),
            category_button("Use", StorageCategory::Use, false),
            category_button("Etc", StorageCategory::Etc, false),
            category_button("Equip", StorageCategory::Equip, false),
            (Node { flex_grow: 1.0 } ignore_picking()),
            search_field(),
        ]
    }
}

fn category_button(label: &'static str, category: StorageCategory, active: bool) -> impl Scene {
    let bg = if active {
        theme::EMERALD_INK
    } else {
        theme::FIELD
    };
    bsn! {
        @FeathersButton { @caption: bsn! { chrome_text(label.to_string(), 11.0, theme::TEXT) } }
        template_value(StorageCategoryButton(category))
        Node { height: px(30), padding: {UiRect::horizontal(px(12))} }
        BackgroundColor(bg)
        on(on_category_activate)
    }
}

fn search_field() -> impl Scene {
    bsn! {
        Node {
            width: px(188), height: px(30),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: {UiRect::horizontal(px(10))},
            border: px(1),
            border_radius: BorderRadius::all(px(8)),
            position_type: PositionType::Relative,
        }
        BackgroundColor({theme::FIELD})
        BorderColor::all(theme::STROKE)
        Children [
            glyph_icon("search", 13.0, theme::TEXT_FAINT),
            (
                #search
                StorageSearchField
                EditableText
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor({theme::TEXT})
                Node { flex_grow: 1.0, min_width: px(0), height: px(18) }
            ),
            (
                StorageSearchPlaceholder
                Text("Search items…")
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor({theme::TEXT_FAINT})
                Node { position_type: PositionType::Absolute, left: px(32), top: px(7) }
                ignore_picking()
            ),
        ]
    }
}

fn panes_shell() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            column_gap: px(10),
            padding: {UiRect::all(px(14))},
        }
        ignore_picking()
        Children [
            (StorageBagHost Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) } ignore_picking()),
            transfer_column(),
            (StorageVaultHost Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0) } ignore_picking()),
        ]
    }
}

fn transfer_column() -> impl Scene {
    bsn! {
        Node {
            width: px(48),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(8),
        }
        ignore_picking()
        Children [
            transfer_button(StorageTransferDirection::Deposit, "chevr"),
            chrome_text("Move".to_string(), 9.0, theme::TEXT_FAINT),
            transfer_button(StorageTransferDirection::Withdraw, "chevl"),
        ]
    }
}

fn transfer_button(direction: StorageTransferDirection, icon: &'static str) -> impl Scene {
    bsn! {
        @FeathersButton { @caption: bsn! { glyph_icon(icon, 18.0, theme::TEXT_FAINT) } }
        template_value(StorageTransferButton { direction, enabled: false })
        Node { width: px(42), height: px(42), border_radius: BorderRadius::all(px(10)) }
        BackgroundColor({theme::FIELD})
        on(on_transfer_activate)
    }
}

fn footer() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            padding: {UiRect::axes(px(14), px(11))},
            border: {UiRect { top: Val::Px(1.0), ..default() }},
        }
        ThemeBorderColor({TOKEN_WINDOW_BORDER})
        ignore_picking()
        Children [
            (
                StorageCloseControl
                @FeathersButton { @caption: bsn! { chrome_text("Close".to_string(), 12.0, theme::TEXT) } }
                Node { width: px(96), height: px(36) }
                BackgroundColor({theme::EMERALD})
                on(on_storage_close)
            ),
        ]
    }
}

pub(crate) fn error_message(message: String) -> impl Scene {
    bsn! {
        Text(message)
        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(11.0)} }
        TextColor({theme::BAD})
        ignore_picking()
    }
}

pub(crate) fn amount_overlay(amount: String, error: Option<String>) -> impl Scene {
    let error = error.map(|message| EntityScene(error_message(message)));
    bsn! {
        Node {
            width: percent(100), height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({Color::srgba(0.0, 0.0, 0.0, 0.55)})
        Pickable
        Children [
            (
                Node {
                    width: px(280),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    padding: {UiRect::all(px(20))},
                    border: px(1),
                    border_radius: BorderRadius::all(px(12)),
                }
                BackgroundColor({theme::GLASS_2})
                BorderColor::all(theme::STROKE_STRONG)
                Children [
                    chrome_text("Transfer amount".to_string(), 14.0, theme::TEXT),
                    (
                        StorageAmountField
                        template_value(EditableText::new(amount))
                        template_value(EditableTextFilter::new(|c| c.is_ascii_digit()))
                        AutoFocus
                        TextFont { font: FontSourceTemplate::Handle(theme::FONT_BODY), font_size: {FontSize::Px(13.0)} }
                        TextColor({theme::TEXT})
                        Node { height: px(34), padding: {UiRect::axes(px(10), px(7))}, border: px(1), border_radius: BorderRadius::all(px(7)) }
                        BackgroundColor({theme::FIELD})
                        BorderColor::all(theme::STROKE)
                    ),
                    {error},
                    (
                        Node { flex_direction: FlexDirection::Row, column_gap: px(8) }
                        ignore_picking()
                        Children [
                            (
                                StorageAmountCancel
                                @FeathersButton { @caption: bsn! { chrome_text("Cancel".to_string(), 12.0, theme::TEXT_DIM) } }
                                Node { flex_grow: 1.0, height: px(34) }
                                on(on_amount_cancel)
                            ),
                            (
                                StorageAmountConfirm
                                @FeathersButton { @caption: bsn! { chrome_text("Transfer".to_string(), 12.0, theme::TEXT) } }
                                Node { flex_grow: 1.0, height: px(34) }
                                BackgroundColor({theme::EMERALD})
                                on(on_amount_confirm)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use bevy::text::EditableText;
    use game_engine::domain::inventory::{Inventory, Item};
    use game_engine::domain::storage::Storage;
    use game_engine::infrastructure::item::ItemDb;
    use lifthrasir_data::{ItemData, ItemInfo};
    use net_contract::dto::StorageItem;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn item_db() -> ItemDb {
        let mut data = ItemData::default();
        data.items.insert(
            501,
            ItemInfo {
                identified_name: "Red Potion".to_string(),
                identified_resource: "RED_POTION".to_string(),
                ..Default::default()
            },
        );
        ItemDb::from_item_data(data)
    }

    fn vault_item(index: u32) -> StorageItem {
        StorageItem {
            index,
            nameid: 501,
            amount: 12,
            type_: 0,
            location: 0,
            attribute: 0,
            refine: 0,
            expire_time: 0,
            look: 0,
            weight: 10,
            identified: true,
            cards: vec![],
        }
    }

    #[test]
    fn shell_is_hidden_stable_and_omits_keeper_and_zeny() {
        let mut app = test_app();
        app.world_mut().spawn_scene(window()).expect("shell spawns");
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&StorageWindowRoot>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            *app.world_mut()
                .query_filtered::<&Visibility, With<StorageWindowRoot>>()
                .single(app.world())
                .unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageBagHost>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageVaultHost>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageOverlayHost>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageCloseControl>()
                .iter(app.world())
                .count(),
            2
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, (With<StorageSearchField>, With<EditableText>)>()
                .iter(app.world())
                .count(),
            1
        );
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.iter().any(|text| text == "Storage Vault"));
        assert!(!texts.iter().any(|text| text.contains("Idunn")
            || text.contains("Zeny")
            || text.contains("Keeper")));
    }

    #[test]
    fn pane_views_render_filtered_names_and_live_capacity() {
        let db = item_db();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 3,
            item_id: 501,
            item_type: 0,
            amount: 4,
            identified: true,
            ..Default::default()
        });
        inventory.upsert(Item {
            index: 4,
            item_id: 501,
            item_type: 0,
            wear_state: 1,
            identified: true,
            ..Default::default()
        });
        let mut storage = Storage::default();
        storage.open(600, vec![vault_item(70_000)]);
        let mut ui = StorageUi {
            category: StorageCategory::Use,
            ..Default::default()
        };
        ui.set_query("potion");

        let (bag, vault) = pane_views(&inventory, &storage, &ui, &db);

        assert_eq!(bag.cells.len(), 1);
        assert_eq!(bag.cells[0].name, "Red Potion");
        assert_eq!(vault.cells.len(), 1);
        assert_eq!(vault.subtitle, "1 / 600");
        ui.set_query("missing");
        let (bag, vault) = pane_views(&inventory, &storage, &ui, &db);
        assert_eq!(bag.empty_message, "Nothing matches your search.");
        assert_eq!(vault.empty_message, "Nothing matches your search.");
    }

    #[test]
    fn pane_rebuild_preserves_the_search_entity() {
        let mut app = test_app();
        app.init_resource::<Inventory>();
        app.init_resource::<Storage>();
        app.init_resource::<StorageUi>();
        app.insert_resource(item_db());
        app.add_systems(Update, super::super::rebuild_panes);
        app.world_mut().spawn_scene(window()).expect("shell spawns");
        app.world_mut().resource_mut::<Inventory>().upsert(Item {
            index: 3,
            item_id: 501,
            item_type: 0,
            amount: 4,
            identified: true,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<Storage>()
            .open(600, vec![vault_item(70_000)]);

        app.update();
        let search = app
            .world_mut()
            .query_filtered::<Entity, With<StorageSearchField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            app.world_mut()
                .query::<&StorageCell>()
                .iter(app.world())
                .count(),
            2
        );

        app.world_mut()
            .resource_mut::<StorageUi>()
            .set_query("missing");
        app.update();

        let search_after = app
            .world_mut()
            .query_filtered::<Entity, With<StorageSearchField>>()
            .single(app.world())
            .unwrap();
        assert_eq!(search_after, search);
        assert_eq!(
            app.world_mut()
                .query::<&StorageCell>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn pane_scene_renders_selection_name_and_empty_state() {
        let mut app = test_app();
        let selected = StorageCellView {
            selection: StorageSelection::Bag(7),
            icon: "ui/icons/bag.svg".to_string(),
            name: "Red Potion".to_string(),
            category: "Use",
            amount: 3,
            refine: 7,
            selected: true,
        };
        app.world_mut()
            .spawn_scene(pane(
                StoragePaneView {
                    title: "Your Bag",
                    subtitle: "1 items".to_string(),
                    cells: vec![selected],
                    empty_message: "Nothing here yet.",
                },
                PaneSide::Bag,
            ))
            .unwrap();
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&StorageCell>()
                .iter(app.world())
                .count(),
            1
        );
        let row = app
            .world_mut()
            .query_filtered::<&Node, With<StorageCell>>()
            .single(app.world())
            .unwrap();
        assert_eq!(row.width, percent(100));
        assert_eq!(row.height, px(52));
        assert_eq!(row.flex_direction, FlexDirection::Row);
        assert_eq!(row.flex_shrink, 0.0);
        let list = app
            .world_mut()
            .query_filtered::<&Node, With<ScrollArea>>()
            .single(app.world())
            .unwrap();
        assert_eq!(list.flex_direction, FlexDirection::Column);
        assert_eq!(list.align_items, AlignItems::Stretch);
        let texts: Vec<_> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.iter().any(|text| text == "Red Potion"));
        assert!(texts.iter().any(|text| text == "Use"));
        assert!(texts.iter().any(|text| text == "3"));
        assert!(texts.iter().any(|text| text == "+7"));
        assert!(!texts.iter().any(|text| text == "Nothing here yet."));

        app.world_mut()
            .spawn_scene(pane(
                StoragePaneView {
                    title: "Storage Vault",
                    subtitle: "0 / 600".to_string(),
                    cells: vec![],
                    empty_message: "Nothing here yet.",
                },
                PaneSide::Vault,
            ))
            .unwrap();
        app.update();
        assert!(
            app.world_mut()
                .query::<&Text>()
                .iter(app.world())
                .any(|text| text.0 == "Nothing here yet.")
        );
    }

    #[test]
    fn empty_states_show_side_specific_glyphs() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(empty_state("Nothing here yet.", PaneSide::Bag))
            .unwrap();
        app.world_mut()
            .spawn_scene(empty_state("Nothing here yet.", PaneSide::Vault))
            .unwrap();
        app.update();

        let image_ids: Vec<_> = app
            .world_mut()
            .query::<&ImageNode>()
            .iter(app.world())
            .map(|image| image.image.id())
            .collect();
        let asset_server = app.world().resource::<AssetServer>();
        let paths: Vec<_> = image_ids
            .into_iter()
            .map(|image| {
                asset_server
                    .get_path(image)
                    .expect("empty-state glyph has an asset path")
                    .to_string()
            })
            .collect();
        assert!(paths.iter().any(|path| path.ends_with("bag.svg")));
        assert!(paths.iter().any(|path| path.ends_with("vault.svg")));
    }

    #[test]
    fn error_is_red_and_pending_transfer_renders_overlay_controls() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(error_message("Storage is full.".to_string()))
            .unwrap();
        app.world_mut()
            .spawn_scene(amount_overlay("1".to_string(), None))
            .unwrap();
        app.update();

        assert!(
            app.world_mut()
                .query::<(&Text, &TextColor)>()
                .iter(app.world())
                .any(|(text, color)| text.0 == "Storage is full." && color.0 == theme::BAD)
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageAmountField>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageAmountConfirm>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&StorageAmountCancel>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, (
                    With<StorageAmountField>,
                    With<bevy::text::EditableTextFilter>,
                )>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn amount_overlay_renders_validation_feedback_in_red() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(amount_overlay(
                "6".to_string(),
                Some("Enter an amount within the available stack.".to_string()),
            ))
            .unwrap();
        app.update();

        assert!(
            app.world_mut()
                .query::<(&Text, &TextColor)>()
                .iter(app.world())
                .any(
                    |(text, color)| text.0 == "Enter an amount within the available stack."
                        && color.0 == theme::BAD
                )
        );
    }
}
