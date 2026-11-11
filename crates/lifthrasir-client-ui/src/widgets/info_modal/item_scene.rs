//! Item info modal content: an [`ItemInfoView`] drawn as the RO-dark "Item Info"
//! window. A parchment plate with the collection illustration sits beside the type,
//! name and description; the stats, card slots, random options and flags follow, and
//! the footer holds the action the summoning surface chose (`ItemAction`). Sections
//! whose view field is empty are omitted entirely, not spawned as empty wrappers.
//! Sizes follow the `Inventory` artboard of the Lifthrasir Widgets design.
//!
//! Footer actions revalidate the item at the stored index against the `item_id`
//! captured when the modal was built, since the inventory can change while the
//! modal is open (item consumed, moved, sold). A mismatch warns and closes the
//! modal without writing a command.

use bevy::prelude::*;
use bevy::scene::EntityScene;
use bevy::ui_widgets::{Activate, ControlOrientation, ScrollArea};
use bevy_feathers::controls::FeathersScrollbar;

use lifthrasir_client::domain::equipment::{EquipItemRequested, UnequipItemRequested};
use lifthrasir_client::domain::inventory::{Inventory, UseItemRequested};

use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::{chrome_text, ignore_picking};
use lifthrasir_ui::widgets::primitives::{
    ButtonArt, WINDOW_SHADOW, close_button, diamond, display_text, inset, window_title_bar,
    window_titled,
};

use crate::screens::shared::{flush_button, glow, usable};
use crate::widgets::info_modal::description::slate_ink;
use crate::widgets::info_modal::shell::{self, EdgeGrade, PARCHMENT};
use crate::widgets::info_modal::view::{ColoredLine, ItemInfoView, decorated_name};
use crate::widgets::info_modal::{InfoModalRoot, ItemAction, ItemActionKind};

const WINDOW_WIDTH: f32 = 360.0;
/// The sections scroll beyond this, so a long description never outgrows the screen.
const BODY_MAX_HEIGHT: f32 = 520.0;
/// The parchment plate, sized for RO's 75x100 collection art at 1.5x.
const PLATE: Vec2 = Vec2::new(120.0, 160.0);
const ILLUSTRATION: Vec2 = Vec2::new(112.0, 150.0);
/// Width of the bronze ring and dark gap around the plate.
const PLATE_RING: f32 = 3.0;

fn action_label(kind: ItemActionKind) -> &'static str {
    match kind {
        ItemActionKind::Use => "Use",
        ItemActionKind::Equip => "Equip",
        ItemActionKind::Unequip => "Unequip",
    }
}

/// Carries the data a footer button's click observer needs to revalidate and act:
/// the inventory slot, the `item_id` it had when the modal was built, and which
/// command to write.
#[derive(Component, Clone, Copy)]
pub(super) struct FooterAction {
    index: u16,
    item_id: u32,
    kind: ItemActionKind,
}

/// Every field is always set explicitly where the button is built; this impl exists
/// only so `FooterAction` satisfies `bsn!`'s `Template` bound.
impl Default for FooterAction {
    fn default() -> Self {
        Self {
            index: 0,
            item_id: 0,
            kind: ItemActionKind::Use,
        }
    }
}

/// Marks the parchment plate holding the native RO collection illustration.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ItemIllustration;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ItemIllustrationImage;

/// The item window, moved by [`drag_item_window`].
#[derive(Component, Default, Clone)]
pub(super) struct ItemInfoWindow;

/// The title bar, which is the drag handle.
#[derive(Component, Default, Clone)]
struct ItemInfoTitlebar;

/// How far the player has dragged the item window from the screen centre. It outlives
/// the modal, so the next item opens where the last one was left.
#[derive(Resource, Default)]
pub(crate) struct ItemInfoOffset(pub Vec2);

/// Drags the window by its title bar. `PointerDrag` bubbles up from the close button,
/// so a drag whose target is not the bar itself is ignored. The pointer delta is in
/// window pixels, so it is divided by the UI scale to keep the bar under the cursor.
fn drag_item_window(
    drag: On<PointerDrag>,
    titlebars: Query<(), With<ItemInfoTitlebar>>,
    mut windows: Query<&mut Node, With<ItemInfoWindow>>,
    mut offset: ResMut<ItemInfoOffset>,
    ui_scale: Res<UiScale>,
) {
    if titlebars.get(drag.entity).is_err() {
        return;
    }
    offset.0 += drag.delta / ui_scale.0;
    for mut node in &mut windows {
        node.left = px(offset.0.x);
        node.top = px(offset.0.y);
    }
}

/// The whole item window: title bar, the scrolling section stack, then the footer with
/// `action` (if any). It stops clicks from reaching the backdrop, so clicking inside
/// does not dismiss the modal. The caller offsets it with `left`/`top`.
pub(super) fn scene(view: ItemInfoView, action: Option<ItemAction>) -> impl Scene {
    let mut sections: Vec<Box<dyn Scene>> = vec![Box::new(summary(&view))];
    if !view.meta.is_empty() {
        sections.push(Box::new(stats_panel(view.meta.clone())));
    }
    if view.sockets_total > 0 {
        sections.push(Box::new(card_slots(&view)));
    }
    if !view.options.is_empty() {
        sections.push(Box::new(random_options(view.options.clone())));
    }
    let flags = flags(&view);
    if !flags.is_empty() {
        sections.push(Box::new(flag_lines(flags)));
    }
    let footer = action.map(|action| EntityScene(footer(&view, action)));

    bsn! {
        ItemInfoWindow
        @window_titled()
        Node {
            width: px(WINDOW_WIDTH + 2.0 * WINDOW_SHADOW),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
        }
        Pickable
        on(|mut click: On<PointerClick>| click.propagate(false))
        Children [
            ItemInfoTitlebar
            @window_title_bar("Item Info", bsn! { @close_button() on(shell::close_modal) })
            Pickable
            on(drag_item_window)
            --
            @scroll_body(sections)
            --
            {footer}
        ]
    }
}

/// The section stack, scrolling past [`BODY_MAX_HEIGHT`]. The scrollbar sits in the
/// window's side padding and hides while everything fits.
fn scroll_body(sections: Vec<Box<dyn Scene>>) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Relative,
            margin: {UiRect::new(px(0), px(0), px(10), px(8))},
        }
        @ignore_picking()
        Children [
            #viewport
            ScrollArea
            Pickable
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                max_height: px(BODY_MAX_HEIGHT),
                flex_direction: FlexDirection::Column,
                overflow: {Overflow::scroll_y()},
            }
            Children [
                Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0, row_gap: px(12) }
                @ignore_picking()
                Children [ {sections} ]
            ]
            --
            @FeathersScrollbar { @target: #viewport, @orientation: {ControlOrientation::Vertical} }
            Node {
                position_type: PositionType::Absolute,
                right: px(-14),
                top: px(0),
                bottom: px(0),
                width: px(6),
            }
        ]
    }
}

/// The plate beside the item's type line, name and description.
fn summary(view: &ItemInfoView) -> impl Scene + use<> {
    let rare = view.edge == EdgeGrade::Rare;
    let plate = view
        .illustration_path
        .clone()
        .map(|path| EntityScene(plate(path, view.identified, rare)));
    let kind = view.tags.join(" \u{b7} ").to_uppercase();
    let refine = view.refine.and_then(|r| u8::try_from(r).ok()).unwrap_or(0);
    let name = decorated_name(&view.name, refine, view.sockets_total);
    let name_color = if rare {
        ro_dark::RARE_RING
    } else {
        ro_dark::STRONG
    };
    let description =
        (!view.description.is_empty()).then(|| EntityScene(description(view.description.clone())));
    bsn! {
        Node { align_items: AlignItems::FlexStart, column_gap: px(16) }
        @ignore_picking()
        Children [
            {plate}
            --
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                flex_basis: px(0),
                min_width: px(0),
            }
            @ignore_picking()
            Children [
                @chrome_text(kind, 10.0, ro_dark::ACCENT)
                --
                @chrome_text(name, 15.0, name_color)
                Node { margin: {UiRect::top(px(2))} }
                --
                @rule()
                --
                {description}
            ]
        ]
    }
}

/// The parchment plate: a bronze ring (gold and glowing for a rare item) around the
/// collection illustration, studded at the corners. The art is tinted with the
/// parchment so it reads as printed on it, and an unidentified item's is faded behind
/// a question mark.
fn plate(path: String, identified: bool, rare: bool) -> impl Scene {
    let (ring, halo) = if rare {
        (
            ro_dark::RARE_RING,
            glow(ro_dark::RARE_GLOW.with_alpha(0.45)),
        )
    } else {
        (ro_dark::TAB_STROKE, glow(Color::NONE))
    };
    let tint = if identified {
        PARCHMENT
    } else {
        PARCHMENT.with_alpha(0.22)
    };
    let unknown = (!identified).then(|| {
        EntityScene(overlay(display_text(
            "?".into(),
            56.0,
            ro_dark::BRONZE_SHADOW,
        )))
    });
    bsn! {
        ItemIllustration
        Node {
            width: px(PLATE.x + 2.0 * PLATE_RING),
            height: px(PLATE.y + 2.0 * PLATE_RING),
            flex_shrink: 0.0,
            padding: px(PLATE_RING - 1.0),
            border: px(1),
            border_radius: BorderRadius::all(px(6)),
        }
        BackgroundColor(ro_dark::CORNER_DISC)
        BorderColor::all(ring)
        halo
        @ignore_picking()
        Children [
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: px(1),
                border_radius: BorderRadius::all(px(4)),
                position_type: PositionType::Relative,
            }
            BackgroundColor(PARCHMENT)
            BorderColor::all(ro_dark::BRONZE_SHADOW)
            @ignore_picking()
            Children [
                ItemIllustrationImage
                ImageNode { image: {path}, color: {tint} }
                Node { width: px(ILLUSTRATION.x), height: px(ILLUSTRATION.y) }
                @ignore_picking()
                --
                {unknown}
                --
                @stud(false, false) -- @stud(true, false) -- @stud(false, true) -- @stud(true, true)
            ]
        ]
    }
}

/// `content` centred over its parent.
fn overlay(content: impl Scene) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0), top: px(0), right: px(0), bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        @ignore_picking()
        Children [ @content ]
    }
}

/// A bronze stud in one corner of the plate.
fn stud(right: bool, bottom: bool) -> impl Scene {
    let inset = |far: bool| {
        if far {
            (Val::Auto, px(4))
        } else {
            (px(4), Val::Auto)
        }
    };
    let ((left, right), (top, bottom)) = (inset(right), inset(bottom));
    bsn! {
        @diamond(5.0, ro_dark::BRONZE, ro_dark::BRONZE_SHADOW)
        Node {
            position_type: PositionType::Absolute,
            left: {left},
            right: {right},
            top: {top},
            bottom: {bottom},
        }
    }
}

/// The short diamond-and-line rule under the name.
fn rule() -> impl Scene {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: px(6),
            margin: {UiRect::new(px(0), px(0), px(10), px(8))},
        }
        @ignore_picking()
        Children [
            @diamond(5.0, ro_dark::DIVIDER, ro_dark::DIVIDER)
            --
            Node { flex_grow: 1.0, height: px(1) }
            BackgroundColor({ro_dark::DIVIDER.with_alpha(0.6)})
            @ignore_picking()
        ]
    }
}

/// The description lines, recoloured to stay readable on the window body.
fn description(lines: Vec<ColoredLine>) -> impl Scene {
    let lines = lines
        .into_iter()
        .map(|runs| {
            runs.into_iter()
                .map(|(color, text)| (slate_ink(color), text))
                .collect()
        })
        .collect();
    shell::description_section(lines)
}

/// The contextual `(label, value)` rows (weight, price, creator), two per line.
fn stats_panel(entries: Vec<(String, String)>) -> impl Scene {
    let cells: Vec<_> = entries
        .into_iter()
        .map(|(key, value)| stat(key, value))
        .collect();
    bsn! {
        @inset()
        Node {
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(2),
            padding: {UiRect::axes(px(12), px(8))},
        }
        @ignore_picking()
        Children [ {cells} ]
    }
}

fn stat(key: String, value: String) -> impl Scene {
    bsn! {
        Node {
            width: percent(46),
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(8),
        }
        @ignore_picking()
        Children [
            @chrome_text(key, 12.0, ro_dark::MUTED)
            --
            @chrome_text(value, 12.0, ro_dark::STRONG)
        ]
    }
}

/// A labelled stack of rows.
fn section<S: Scene>(label: &'static str, rows: Vec<S>) -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(2) }
        @ignore_picking()
        Children [
            @chrome_text(label.into(), 10.0, ro_dark::ACCENT)
            Node { margin: {UiRect::bottom(px(2))} }
            --
            {rows}
        ]
    }
}

/// One row per card slot: filled ones name the card, empty ones say so.
fn card_slots(view: &ItemInfoView) -> impl Scene + use<> {
    let rows: Vec<_> = (0..usize::from(view.sockets_total))
        .map(|slot| card_slot(view.cards.get(slot).cloned()))
        .collect();
    section("CARD SLOTS", rows)
}

fn card_slot(name: Option<String>) -> impl Scene {
    let (fill, stroke, color) = if name.is_some() {
        (ro_dark::FROST, ro_dark::FROST_GLOW, ro_dark::BODY)
    } else {
        (Color::NONE, ro_dark::MUTED, ro_dark::MUTED)
    };
    let label = name.unwrap_or_else(|| "Empty slot".to_string());
    bsn! {
        Node { align_items: AlignItems::Center, column_gap: px(8) }
        @ignore_picking()
        Children [
            @diamond(7.0, fill, stroke)
            --
            @chrome_text(label, 12.0, color)
        ]
    }
}

/// One frost line per crafted option, in slot order.
fn random_options(options: Vec<String>) -> impl Scene {
    let rows: Vec<_> = options
        .into_iter()
        .map(|option| chrome_text(option, 12.0, ro_dark::FROST))
        .collect();
    section("RANDOM OPTIONS", rows)
}

/// State lines under the sections. Favorite is the display-only server flag, which
/// only a bag item's view carries.
fn flags(view: &ItemInfoView) -> Vec<(&'static str, Color)> {
    let mut flags = Vec::new();
    if !view.identified {
        flags.push(("Unidentified", ro_dark::STATUS_BUSY));
    }
    if view.favorite == Some(true) {
        flags.push(("Favorite", ro_dark::ACCENT));
    }
    flags
}

fn flag_lines(flags: Vec<(&'static str, Color)>) -> impl Scene {
    let lines: Vec<_> = flags
        .into_iter()
        .map(|(text, color)| chrome_text(text.into(), 12.0, color))
        .collect();
    bsn! {
        Node { flex_direction: FlexDirection::Column }
        @ignore_picking()
        Children [ {lines} ]
    }
}

/// The footer's primary button. Equip is disabled for an unidentified item, since
/// the server refuses it; the other actions are always live.
fn footer(view: &ItemInfoView, action: ItemAction) -> impl Scene + use<> {
    let enabled = action.kind != ItemActionKind::Equip || view.identified;
    let template = FooterAction {
        index: action.index,
        item_id: view.item_id,
        kind: action.kind,
    };
    let label = action_label(action.kind);
    bsn! {
        Node { margin: {UiRect::new(px(0), px(0), px(6), px(8))} }
        @ignore_picking()
        Children [
            template
            @flush_button(ButtonArt::PRIMARY, label)
            @usable(enabled, Color::WHITE)
            on(on_footer_action_click)
        ]
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
    let index = action.index;
    let still_valid = inventory
        .get(index)
        .is_some_and(|item| item.item_id == action.item_id);
    if still_valid {
        match action.kind {
            ItemActionKind::Use => {
                use_writer.write(UseItemRequested {
                    index: index as u32,
                });
            }
            ItemActionKind::Equip => {
                equip_writer.write(EquipItemRequested { index });
            }
            ItemActionKind::Unequip => {
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
    use bevy::color::Luminance;
    use bevy::scene::ScenePlugin;
    use lifthrasir_client::domain::inventory::Item;

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
            favorite: Some(false),
            tags: vec!["Usable".to_string()],
            refine: Some(4),
            sockets_filled: 1,
            sockets_total: 2,
            cards: vec!["Poring Card".to_string()],
            options: vec!["STR + 5".to_string()],
            description: vec![vec![(
                lifthrasir_ui::theme::TEXT_DIM,
                "Restores HP.".to_string(),
            )]],
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
            favorite: None,
            tags: vec![],
            refine: None,
            sockets_filled: 0,
            sockets_total: 0,
            cards: vec![],
            options: vec![],
            description: vec![],
            meta: vec![],
        }
    }

    fn action(kind: ItemActionKind) -> Option<ItemAction> {
        Some(ItemAction { kind, index: 1 })
    }

    fn texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect()
    }

    fn text_color(app: &mut App, wanted: &str) -> Color {
        app.world_mut()
            .query::<(&Text, &TextColor)>()
            .iter(app.world())
            .find(|(text, _)| text.0 == wanted)
            .map(|(_, color)| color.0)
            .unwrap_or_else(|| panic!("no text {wanted:?}"))
    }

    fn footer_actions(app: &mut App) -> Vec<FooterAction> {
        app.world_mut()
            .query::<&FooterAction>()
            .iter(app.world())
            .copied()
            .collect()
    }

    fn footer_is_hoverable(app: &mut App) -> bool {
        app.world_mut()
            .query_filtered::<&Pickable, With<FooterAction>>()
            .single(app.world())
            .expect("one footer button")
            .is_hoverable
    }

    fn plates(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<ItemIllustration>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn full_view_renders_summary_cards_stats_and_description() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(full_view(), action(ItemActionKind::Use)))
            .expect("scene spawns");
        app.update();

        let texts = texts(&mut app);
        for expected in [
            "Item Info",
            "USABLE",
            "+4 Red Potion [2]",
            "CARD SLOTS",
            "Poring Card",
            "Empty slot",
            "Weight",
            "10",
            "Restores HP.",
            "Use",
        ] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected:?} missing from {texts:?}"
            );
        }
        assert_eq!(plates(&mut app), 1);
    }

    #[test]
    fn random_options_render_as_a_section() {
        let mut app = test_app();
        let mut view = empty_view();
        view.options = vec!["STR + 5".to_string(), "MHP + 4%".to_string()];
        app.world_mut().spawn_scene(scene(view, None)).unwrap();
        app.update();

        let rendered = texts(&mut app);
        assert!(
            rendered.contains(&"RANDOM OPTIONS".to_string()),
            "{rendered:?}"
        );
        assert!(rendered.contains(&"STR + 5".to_string()), "{rendered:?}");
        assert!(rendered.contains(&"MHP + 4%".to_string()), "{rendered:?}");
    }

    #[test]
    fn item_without_options_omits_the_section() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(empty_view(), None))
            .unwrap();
        app.update();

        assert!(!texts(&mut app).contains(&"RANDOM OPTIONS".to_string()));
    }

    #[test]
    fn weight_and_price_share_one_wrapping_inset_panel() {
        let mut app = test_app();
        let mut view = empty_view();
        view.meta = vec![
            ("Weight".to_string(), "10".to_string()),
            ("Price".to_string(), "50 z".to_string()),
        ];
        app.world_mut().spawn_scene(scene(view, None)).unwrap();
        app.update();

        let cells: Vec<_> = app
            .world_mut()
            .query::<(&Text, &ChildOf)>()
            .iter(app.world())
            .filter(|(text, _)| matches!(text.0.as_str(), "Weight" | "Price"))
            .map(|(_, parent)| parent.parent())
            .collect();
        assert_eq!(cells.len(), 2);
        let panel = app.world().get::<ChildOf>(cells[0]).unwrap().parent();
        assert_eq!(
            app.world().get::<ChildOf>(cells[1]).unwrap().parent(),
            panel
        );
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().flex_wrap,
            FlexWrap::Wrap
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(panel).unwrap().0,
            ro_dark::INSET
        );
        let values = texts(&mut app);
        assert!(values.contains(&"10".to_string()));
        assert!(values.contains(&"50 z".to_string()));
    }

    #[test]
    fn description_stays_readable_on_the_window_body() {
        let mut app = test_app();
        let mut view = empty_view();
        view.description = vec![
            lifthrasir_ui::rich_text::parse_color_codes(
                "Restores ^000088HP^000000.",
                lifthrasir_ui::theme::TEXT_DIM,
            ),
            vec![(lifthrasir_ui::theme::TEXT_DIM, "Weight: 10".to_string())],
        ];
        app.world_mut().spawn_scene(scene(view, None)).unwrap();
        app.update();

        assert_eq!(text_color(&mut app, "Restores "), ro_dark::BODY);
        assert_eq!(text_color(&mut app, "Weight: 10"), ro_dark::BODY);
        let spans: Vec<Color> = app
            .world_mut()
            .query::<(&TextSpan, &TextColor)>()
            .iter(app.world())
            .map(|(_, color)| color.0)
            .collect();
        assert_eq!(spans.len(), 2);
        let body = ro_dark::WINDOW_BODY.to_linear().luminance();
        for color in spans {
            let contrast = (color.to_linear().luminance() + 0.05) / (body + 0.05);
            assert!(contrast >= 4.5, "{color:?}: contrast {contrast}");
        }
    }

    #[test]
    fn illustration_sits_on_a_ringed_parchment_plate() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(plate(
                "items/red_potion_illustration.png".to_string(),
                true,
                false,
            ))
            .expect("scene spawns");
        app.update();

        let mut plate_query = app
            .world_mut()
            .query_filtered::<(&Node, &BorderColor), With<ItemIllustration>>();
        let (frame, ring) = plate_query.single(app.world()).expect("one plate");
        assert_eq!(frame.width, px(PLATE.x + 2.0 * PLATE_RING));
        assert_eq!(frame.height, px(PLATE.y + 2.0 * PLATE_RING));
        assert_eq!(*ring, BorderColor::all(ro_dark::TAB_STROKE));

        let mut image_query = app
            .world_mut()
            .query_filtered::<(&ImageNode, &ChildOf), With<ItemIllustrationImage>>();
        let (image, parchment) = image_query.single(app.world()).expect("one illustration");
        assert_eq!(image.color, PARCHMENT);
        assert_eq!(
            app.world()
                .get::<BackgroundColor>(parchment.parent())
                .unwrap()
                .0,
            PARCHMENT
        );
    }

    #[test]
    fn rare_item_gets_a_gold_ring_and_name() {
        let mut app = test_app();
        let mut view = full_view();
        view.edge = shell::EdgeGrade::Rare;
        app.world_mut().spawn_scene(scene(view, None)).unwrap();
        app.update();

        assert_eq!(
            text_color(&mut app, "+4 Red Potion [2]"),
            ro_dark::RARE_RING
        );
        let ring = app
            .world_mut()
            .query_filtered::<&BorderColor, With<ItemIllustration>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*ring, BorderColor::all(ro_dark::RARE_RING));
    }

    #[test]
    fn unidentified_item_fades_its_art_and_says_so() {
        let mut app = test_app();
        let mut view = full_view();
        view.identified = false;
        app.world_mut().spawn_scene(scene(view, None)).unwrap();
        app.update();

        let image = app
            .world_mut()
            .query_filtered::<&ImageNode, With<ItemIllustrationImage>>()
            .single(app.world())
            .unwrap();
        assert_eq!(image.color, PARCHMENT.with_alpha(0.22));
        let texts = texts(&mut app);
        assert!(texts.contains(&"?".to_string()), "{texts:?}");
        assert!(texts.contains(&"Unidentified".to_string()), "{texts:?}");
    }

    #[test]
    fn empty_view_renders_no_optional_sections() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(empty_view(), None))
            .expect("scene spawns");
        app.update();

        let texts = texts(&mut app);
        assert!(texts.contains(&"Junk".to_string()), "{texts:?}");
        assert_eq!(plates(&mut app), 0);
        for absent in [
            "CARD SLOTS",
            "Empty slot",
            "RANDOM OPTIONS",
            "Weight",
            "Unidentified",
            "Favorite",
            "Use",
            "Equip",
            "Unequip",
        ] {
            assert!(
                !texts.iter().any(|t| t == absent),
                "unexpected {absent:?} in {texts:?}"
            );
        }
        assert!(footer_actions(&mut app).is_empty());
    }

    #[test]
    fn no_action_renders_no_footer_button() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(full_view(), None))
            .expect("scene spawns");
        app.update();

        assert!(footer_actions(&mut app).is_empty());
    }

    #[test]
    fn equip_button_is_disabled_when_unidentified() {
        let mut app = test_app();
        let mut view = full_view();
        view.identified = false;
        app.world_mut()
            .spawn_scene(scene(view, action(ItemActionKind::Equip)))
            .expect("scene spawns");
        app.update();

        let actions = footer_actions(&mut app);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].kind, ItemActionKind::Equip);
        assert_eq!(actions[0].item_id, 501);
        assert_eq!(actions[0].index, 1);
        assert!(!footer_is_hoverable(&mut app));
    }

    #[test]
    fn identified_equip_button_is_not_disabled() {
        let mut app = test_app();
        app.world_mut()
            .spawn_scene(scene(full_view(), action(ItemActionKind::Equip)))
            .expect("scene spawns");
        app.update();

        let actions = footer_actions(&mut app);
        assert_eq!(actions.len(), 1);
        assert!(footer_is_hoverable(&mut app));
    }

    #[test]
    fn favorite_flag_shows_only_for_a_favorited_item() {
        for (favorite, shown) in [(Some(true), true), (Some(false), false), (None, false)] {
            let mut app = test_app();
            let mut view = full_view();
            view.favorite = favorite;
            app.world_mut()
                .spawn_scene(scene(view, None))
                .expect("scene spawns");
            app.update();

            assert_eq!(
                texts(&mut app).contains(&"Favorite".to_string()),
                shown,
                "{favorite:?}"
            );
        }
    }

    fn drag_event(target: Entity, window: Entity, delta: Vec2) -> PointerDrag {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::pointer::{Location, PointerId};
        use bevy::window::WindowRef;
        PointerDrag {
            entity: target,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Primary.normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
            delta,
        }
    }

    #[test]
    fn dragging_the_title_bar_moves_the_window_and_remembers_the_offset() {
        let mut app = App::new();
        app.insert_resource(UiScale(2.0));
        app.init_resource::<ItemInfoOffset>();
        let window = app
            .world_mut()
            .spawn((ItemInfoWindow, Node::default()))
            .id();
        let bar = app
            .world_mut()
            .spawn(ItemInfoTitlebar)
            .observe(drag_item_window)
            .id();
        let close = app.world_mut().spawn_empty().observe(drag_item_window).id();
        let screen = app.world_mut().spawn_empty().id();

        app.world_mut()
            .trigger(drag_event(bar, screen, Vec2::new(30.0, -10.0)));
        app.world_mut()
            .trigger(drag_event(close, screen, Vec2::new(500.0, 500.0)));

        let node = app.world().get::<Node>(window).unwrap();
        assert_eq!((node.left, node.top), (px(15.0), px(-5.0)));
        assert_eq!(
            app.world().resource::<ItemInfoOffset>().0,
            Vec2::new(15.0, -5.0)
        );
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
                index: 5,
                item_id: 501,
                kind: ItemActionKind::Use,
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
                index: 5,
                item_id: 501,
                kind: ItemActionKind::Use,
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
