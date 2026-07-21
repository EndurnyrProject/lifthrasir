//! Item info modal content: renders an [`ItemInfoView`] through the shell chrome —
//! header, card slots, meta grid, and description — plus a footer whose action
//! depends on where the item came from (`ItemRef`). Optional sections (cards, meta,
//! description, footer) are omitted entirely when their view field is empty, rather
//! than spawned as empty wrappers.
//!
//! Footer actions revalidate the item at the stored index against the `item_id`
//! captured when the modal was built, since the inventory can change while the
//! modal is open (item consumed, moved, sold). A mismatch warns and closes the
//! modal without writing a command.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::text::{FontSize, FontSourceTemplate};
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use bevy_feathers::controls::{ButtonVariant, FeathersButton};
use bevy_feathers::theme::ThemedText;

use game_engine::domain::equipment::{EquipItemRequested, UnequipItemRequested};
use game_engine::domain::inventory::{Inventory, ItemCategory, UseItemRequested};

use crate::theme;
use crate::widgets::chrome::{glyph_icon, ignore_picking};

use super::shell::{self, HeaderView};
use super::view::ItemInfoView;
use super::{InfoModalRoot, ItemRef};

/// Which command a footer button writes on click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FooterActionKind {
    Use,
    Equip,
    Unequip,
}

impl FooterActionKind {
    fn label(self) -> &'static str {
        match self {
            FooterActionKind::Use => "Use",
            FooterActionKind::Equip => "Equip",
            FooterActionKind::Unequip => "Unequip",
        }
    }
}

/// Carries the data a footer button's click observer needs to revalidate and act:
/// where the item lives, the `item_id` it had when the modal was built, and which
/// command to write.
#[derive(Component, Clone, Copy)]
pub(super) struct FooterAction {
    item_ref: ItemRef,
    item_id: u32,
    kind: FooterActionKind,
    disabled: bool,
}

/// Every field is always set explicitly at the `template_value` call site; this
/// impl exists only so `FooterAction` satisfies `bsn!`'s `Template` bound.
impl Default for FooterAction {
    fn default() -> Self {
        Self {
            item_ref: ItemRef::Cart(0),
            item_id: 0,
            kind: FooterActionKind::Use,
            disabled: false,
        }
    }
}

/// Marks the display-only favorite indicator in the item footer — `.im-act.ghost`
/// (`.on` when lit). Carries the flag it renders so tests can assert lit state
/// without inspecting colors; not a button, never targeted by an observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FavoriteStar {
    pub lit: bool,
}

/// Marks the native RO collection illustration in the item details body.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ItemIllustration;

fn item_illustration(path: String) -> impl Scene {
    bsn! {
        ItemIllustration
        ImageNode { image: {path} }
        Node {
            width: px(75),
            height: px(100),
            flex_shrink: 0.0,
            align_self: AlignSelf::Center,
        }
        ignore_picking()
    }
}

/// The footer's favorite indicator — gold when `favorite`, dim otherwise. Rendered
/// only for `Inventory` refs, since favorite is server item state with no command
/// to toggle it here. `ignore_picking` (not a `FeathersButton`, no observer) keeps
/// it a display-only ghost button per the mockup.
fn favorite_star(favorite: bool) -> impl Scene {
    let (bg, border, color) = if favorite {
        (
            Color::srgba(0.851, 0.643, 0.255, 0.1),
            theme::GOLD_FAINT,
            theme::GOLD,
        )
    } else {
        (
            Color::WHITE.with_alpha(0.03),
            theme::STROKE,
            theme::TEXT_DIM,
        )
    };
    bsn! {
        template_value(FavoriteStar { lit: favorite })
        Node {
            width: px(42),
            height: px(40),
            flex_shrink: 0.0,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        ignore_picking()
        Children [ glyph_icon("star", 15.0, color) ]
    }
}

/// The item modal's whole content: header, then the scrollable section stack, then
/// the contextual footer.
pub(super) fn scene(
    view: ItemInfoView,
    item_ref: ItemRef,
    category: Option<ItemCategory>,
) -> impl Scene {
    let item_id = view.item_id;
    let header = shell::header(HeaderView {
        icon_path: view.icon_path.clone(),
        refine: view.refine,
        sockets_filled: view.sockets_filled,
        sockets_total: view.sockets_total,
        edge: view.edge,
        name: view.name.clone(),
        tags: view.tags.clone(),
    });
    let cards = (view.sockets_total > 0).then(|| EntityScene(card_slots_section(&view)));
    let meta = (!view.meta.is_empty()).then(|| {
        EntityScene(shell::meta_grid(
            view.meta
                .iter()
                .cloned()
                .map(|(key, value)| shell::meta_cell(key, value))
                .collect(),
        ))
    });
    let description = (!view.description.is_empty())
        .then(|| EntityScene(shell::description_section(view.description.clone())));
    let illustration = view
        .illustration_path
        .clone()
        .map(|path| EntityScene(item_illustration(path)));

    let primary_actions = footer_actions(item_ref, item_id, view.identified, category);
    let star = matches!(item_ref, ItemRef::Inventory(_)).then(|| favorite_star(view.favorite));
    let mut footer_children: Vec<Box<dyn Scene>> = primary_actions
        .into_iter()
        .map(|action| Box::new(action) as Box<dyn Scene>)
        .collect();
    footer_children.extend(star.map(|star| Box::new(star) as Box<dyn Scene>));
    let footer =
        (!footer_children.is_empty()).then(|| EntityScene(shell::footer_bar(footer_children)));

    bsn! {
        Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_height: px(0) }
        ignore_picking()
        Children [
            header,
            shell::scroll_body(bsn! {
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: px(14),
                    padding: {UiRect { left: px(20), right: px(20), top: px(0), bottom: px(6) }},
                }
                ignore_picking()
                Children [ {illustration}, {cards}, {meta}, {description} ]
            }),
            {footer},
        ]
    }
}

/// The Card Slots section: a section label with the `filled/total` counter, and one
/// row per socket — filled ones name the card, empty ones say so.
fn card_slots_section(view: &ItemInfoView) -> impl Scene + use<> {
    let counter = format!("{} / {}", view.cards.len(), view.sockets_total);
    let slots: Vec<_> = (0..view.sockets_total)
        .map(|i| card_slot(view.cards.get(i as usize).cloned()))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        ignore_picking()
        Children [
            shell::section_label("Card Slots".to_string(), Some(counter)),
            (
                Node { flex_direction: FlexDirection::Column, row_gap: px(7) }
                ignore_picking()
                Children [ {slots} ]
            ),
        ]
    }
}

fn card_slot(name: Option<String>) -> impl Scene {
    let filled = name.is_some();
    let label = name.unwrap_or_else(|| "Empty Socket".to_string());
    let (bg, border, color) = if filled {
        (
            Color::srgba(0.851, 0.643, 0.255, 0.06),
            theme::GOLD_FAINT,
            theme::TEXT,
        )
    } else {
        (
            Color::BLACK.with_alpha(0.22),
            theme::STROKE,
            theme::TEXT_FAINT,
        )
    };
    bsn! {
        Node {
            padding: {UiRect::axes(px(11), px(9))},
            border: px(1),
            border_radius: BorderRadius::all(px(9)),
        }
        BackgroundColor(bg)
        BorderColor::all(border)
        ignore_picking()
        Children [
            (
                Text(label)
                TextFont {
                    font: FontSourceTemplate::Handle(theme::FONT_BODY),
                    font_size: {FontSize::Px(12.0)},
                }
                TextColor(color)
                ignore_picking()
            ),
        ]
    }
}

/// The footer's action buttons for `item_ref`: Use or Equip (by `category`) for an
/// inventory item, Unequip for an equipped one, nothing for Storage/Cart/ShopBuy —
/// there is no valid primary action from those contexts.
fn footer_actions(
    item_ref: ItemRef,
    item_id: u32,
    identified: bool,
    category: Option<ItemCategory>,
) -> Vec<impl Scene> {
    let action = match (item_ref, category) {
        (ItemRef::Inventory(_), Some(ItemCategory::Equip)) => {
            Some((FooterActionKind::Equip, !identified))
        }
        (ItemRef::Inventory(_), Some(ItemCategory::Use)) => Some((FooterActionKind::Use, false)),
        (ItemRef::Inventory(_), Some(ItemCategory::Etc) | None) => None,
        (ItemRef::Equipped(_), _) => Some((FooterActionKind::Unequip, false)),
        (ItemRef::Storage(_) | ItemRef::Cart(_) | ItemRef::ShopBuy(_), _) => None,
    };
    action
        .map(|(kind, disabled)| action_button(item_ref, item_id, kind, disabled))
        .into_iter()
        .collect()
}

fn action_button(
    item_ref: ItemRef,
    item_id: u32,
    kind: FooterActionKind,
    disabled: bool,
) -> impl Scene {
    let label = kind.label().to_string();
    bsn! {
        template_value(FooterAction { item_ref, item_id, kind, disabled })
        @FeathersButton {
            @caption: bsn! { (Text(label) ThemedText) },
            @variant: ButtonVariant::Primary,
        }
        Node { flex_grow: 1.0, height: px(40), border_radius: BorderRadius::all(px(9)) }
        on(on_footer_action_click)
    }
}

/// Applies [`FooterAction::disabled`] as Feathers' `InteractionDisabled` once per
/// spawned button — the modal rebuilds from scratch on every show, so `Added` fires
/// exactly once per button and needs no steady-state upkeep.
pub(super) fn apply_footer_disabled(
    buttons: Query<(Entity, &FooterAction), Added<FooterAction>>,
    mut commands: Commands,
) {
    for (entity, action) in &buttons {
        if action.disabled {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
}

/// Revalidates the item at the stored index still has the button's `item_id`
/// before writing its command; a mismatch (item consumed/moved/sold while the
/// modal was open) warns and closes the modal without writing anything. Either
/// way, the modal closes.
#[allow(clippy::too_many_arguments)]
fn on_footer_action_click(
    activate: On<Activate>,
    actions: Query<&FooterAction>,
    inventory: Res<Inventory>,
    root: Query<Entity, With<InfoModalRoot>>,
    mut commands: Commands,
    mut use_writer: MessageWriter<UseItemRequested>,
    mut equip_writer: MessageWriter<EquipItemRequested>,
    mut unequip_writer: MessageWriter<UnequipItemRequested>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let index = match action.item_ref {
        ItemRef::Inventory(index) | ItemRef::Equipped(index) => index,
        ItemRef::Storage(_) | ItemRef::Cart(_) | ItemRef::ShopBuy(_) => return,
    };
    let still_valid = inventory
        .get(index)
        .is_some_and(|item| item.item_id == action.item_id);
    if still_valid {
        match action.kind {
            FooterActionKind::Use => {
                use_writer.write(UseItemRequested {
                    index: index as u32,
                });
            }
            FooterActionKind::Equip => {
                equip_writer.write(EquipItemRequested { index });
            }
            FooterActionKind::Unequip => {
                unequip_writer.write(UnequipItemRequested { index });
            }
        }
    } else {
        warn!(
            "info modal: item at index {index} changed before the footer action fired; closing without acting"
        );
    }
    if let Ok(root) = root.single() {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;
    use game_engine::domain::inventory::Item;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.init_asset::<Font>();
        app
    }

    fn full_view() -> ItemInfoView {
        ItemInfoView {
            item_id: 501,
            icon_path: Some("items/red_potion.png".to_string()),
            illustration_path: Some("items/red_potion_illustration.png".to_string()),
            edge: shell::EdgeGrade::Fine,
            name: "Red Potion".to_string(),
            identified: true,
            favorite: false,
            tags: vec!["Usable".to_string()],
            refine: Some(4),
            sockets_filled: 1,
            sockets_total: 2,
            cards: vec!["Poring Card".to_string()],
            description: vec![vec![(theme::TEXT_DIM, "Restores HP.".to_string())]],
            meta: vec![("Weight".to_string(), "10".to_string())],
        }
    }

    fn empty_view() -> ItemInfoView {
        ItemInfoView {
            item_id: 502,
            icon_path: None,
            illustration_path: None,
            edge: shell::EdgeGrade::Common,
            name: "Junk".to_string(),
            identified: true,
            favorite: false,
            tags: vec![],
            refine: None,
            sockets_filled: 0,
            sockets_total: 0,
            cards: vec![],
            description: vec![],
            meta: vec![],
        }
    }

    fn texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect()
    }

    #[test]
    fn full_view_renders_header_cards_meta_and_description() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(
                full_view(),
                ItemRef::Inventory(1),
                Some(ItemCategory::Use),
            ))
            .expect("scene spawns");
        app.update();

        let texts = texts(&mut app);
        assert!(texts.contains(&"Red Potion".to_string()), "{texts:?}");
        assert!(texts.contains(&"CARD SLOTS".to_string()), "{texts:?}");
        assert!(texts.contains(&"1 / 2".to_string()), "{texts:?}");
        assert!(texts.contains(&"Poring Card".to_string()), "{texts:?}");
        assert!(texts.contains(&"Empty Socket".to_string()), "{texts:?}");
        assert!(texts.contains(&"WEIGHT".to_string()), "{texts:?}");
        assert!(texts.contains(&"10".to_string()), "{texts:?}");
        assert!(texts.contains(&"Restores HP.".to_string()), "{texts:?}");
        assert!(texts.contains(&"Use".to_string()), "{texts:?}");
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ItemIllustration>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn empty_view_renders_no_optional_sections() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(empty_view(), ItemRef::Cart(1), None))
            .expect("scene spawns");
        app.update();

        let texts = texts(&mut app);
        assert!(texts.contains(&"Junk".to_string()), "{texts:?}");
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ItemIllustration>>()
                .iter(app.world())
                .count(),
            0
        );
        for absent in [
            "CARD SLOTS",
            "Empty Socket",
            "WEIGHT",
            "Use",
            "Equip",
            "Unequip",
        ] {
            assert!(
                !texts.iter().any(|t| t == absent),
                "unexpected {absent:?} in {texts:?}"
            );
        }
    }

    #[test]
    fn storage_cart_and_shop_refs_have_no_footer_action() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(full_view(), ItemRef::ShopBuy(501), None))
            .expect("scene spawns");
        app.update();

        let count = app
            .world_mut()
            .query::<&FooterAction>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn etc_inventory_item_has_no_footer_action() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(
                full_view(),
                ItemRef::Inventory(1),
                Some(ItemCategory::Etc),
            ))
            .expect("scene spawns");
        app.update();

        let count = app
            .world_mut()
            .query::<&FooterAction>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn equip_button_carries_disabled_state_when_unidentified() {
        let mut app = test_app();
        let mut view = full_view();
        view.identified = false;
        app.world_mut()
            .spawn_scene(scene(
                view,
                ItemRef::Inventory(1),
                Some(ItemCategory::Equip),
            ))
            .expect("scene spawns");
        app.update();

        let actions: Vec<_> = app
            .world_mut()
            .query::<&FooterAction>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, FooterActionKind::Equip);
        assert!(actions[0].disabled);
    }

    #[test]
    fn identified_equip_button_is_not_disabled() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(
                full_view(),
                ItemRef::Inventory(1),
                Some(ItemCategory::Equip),
            ))
            .expect("scene spawns");
        app.update();

        let actions: Vec<_> = app
            .world_mut()
            .query::<&FooterAction>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(actions.len(), 1);
        assert!(!actions[0].disabled);
    }

    #[test]
    fn apply_footer_disabled_inserts_interaction_disabled_only_when_flagged() {
        let mut app = App::new();
        app.add_systems(Update, apply_footer_disabled);
        let disabled = app
            .world_mut()
            .spawn(FooterAction {
                item_ref: ItemRef::Inventory(1),
                item_id: 1,
                kind: FooterActionKind::Equip,
                disabled: true,
            })
            .id();
        let enabled = app
            .world_mut()
            .spawn(FooterAction {
                item_ref: ItemRef::Inventory(1),
                item_id: 1,
                kind: FooterActionKind::Use,
                disabled: false,
            })
            .id();
        app.update();

        assert!(app.world().get::<InteractionDisabled>(disabled).is_some());
        assert!(app.world().get::<InteractionDisabled>(enabled).is_none());
    }

    #[test]
    fn favorite_star_renders_for_inventory_ref_with_no_primary_action() {
        let mut app = test_app();
        let mut view = full_view();
        view.favorite = true;
        app.world_mut()
            .spawn_scene(scene(view, ItemRef::Inventory(1), Some(ItemCategory::Etc)))
            .expect("scene spawns");
        app.update();

        let stars: Vec<_> = app
            .world_mut()
            .query::<&FavoriteStar>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(stars.len(), 1);
        assert!(stars[0].lit);
    }

    #[test]
    fn favorite_star_unlit_when_view_not_favorited() {
        let mut app = test_app();
        let mut view = full_view();
        view.favorite = false;
        app.world_mut()
            .spawn_scene(scene(view, ItemRef::Inventory(1), Some(ItemCategory::Use)))
            .expect("scene spawns");
        app.update();

        let stars: Vec<_> = app
            .world_mut()
            .query::<&FavoriteStar>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(stars.len(), 1);
        assert!(!stars[0].lit);
    }

    #[test]
    fn favorite_star_absent_for_non_inventory_refs() {
        for item_ref in [
            ItemRef::Equipped(1),
            ItemRef::Cart(1),
            ItemRef::ShopBuy(501),
        ] {
            let mut app = test_app();
            app.world_mut()
                .spawn_scene(scene(full_view(), item_ref, None))
                .expect("scene spawns");
            app.update();

            let count = app
                .world_mut()
                .query::<&FavoriteStar>()
                .iter(app.world())
                .count();
            assert_eq!(count, 0, "{item_ref:?}");
        }
    }

    #[test]
    fn favorite_star_is_not_interactive() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(
                full_view(),
                ItemRef::Inventory(1),
                Some(ItemCategory::Use),
            ))
            .expect("scene spawns");
        app.update();

        let pickable = app
            .world_mut()
            .query_filtered::<&Pickable, With<FavoriteStar>>()
            .single(app.world())
            .expect("star entity has a Pickable component");
        assert!(!pickable.is_hoverable);
    }

    fn revalidation_app() -> App {
        let mut app = App::new();
        app.add_message::<UseItemRequested>();
        app.add_message::<EquipItemRequested>();
        app.add_message::<UnequipItemRequested>();
        app
    }

    #[test]
    fn revalidation_blocks_the_command_when_the_item_changed() {
        let mut app = revalidation_app();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 5,
            item_id: 999,
            identified: true,
            ..Default::default()
        });
        app.insert_resource(inventory);

        let root = app.world_mut().spawn(InfoModalRoot).id();
        let button = app
            .world_mut()
            .spawn(FooterAction {
                item_ref: ItemRef::Inventory(5),
                item_id: 501,
                kind: FooterActionKind::Use,
                disabled: false,
            })
            .observe(on_footer_action_click)
            .id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let used = app
            .world()
            .resource::<Messages<UseItemRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(used, 0);
        assert!(app.world().get_entity(root).is_err());
    }

    #[test]
    fn revalidation_writes_the_command_when_the_item_still_matches() {
        let mut app = revalidation_app();
        let mut inventory = Inventory::default();
        inventory.upsert(Item {
            index: 5,
            item_id: 501,
            identified: true,
            ..Default::default()
        });
        app.insert_resource(inventory);

        let root = app.world_mut().spawn(InfoModalRoot).id();
        let button = app
            .world_mut()
            .spawn(FooterAction {
                item_ref: ItemRef::Inventory(5),
                item_id: 501,
                kind: FooterActionKind::Use,
                disabled: false,
            })
            .observe(on_footer_action_click)
            .id();

        app.world_mut().trigger(Activate { entity: button });
        app.world_mut().flush();

        let used = app
            .world()
            .resource::<Messages<UseItemRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(used, 1);
        assert!(app.world().get_entity(root).is_err());
    }
}
