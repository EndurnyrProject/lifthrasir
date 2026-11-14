//! The toast stack in the screen's lower right corner, after the
//! `Announcements & Toasts` artboard.
//!
//! Any system raises a toast by writing a [`ShowToast`] message. At most
//! [`MAX_TOASTS`] show at once, newest at the bottom; each drains its life bar and
//! leaves, and the stack holds still while the pointer is on it. [`feeds`] raises the
//! toasts the game events call for.
//!
//! NOTE: the artboard's quest toasts are not raised: the client has no quest data.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lifthrasir_client::core::state::GameState;
use lifthrasir_ui::theme::ro_dark;
use lifthrasir_ui::widgets::chrome::ignore_picking;

use crate::widgets::chat_box::ChatUi;
use crate::widgets::inventory_window::InventoryUi;

mod feeds;
mod scene;

#[cfg(test)]
mod tests;

/// How many toasts show at once. A newer one pushes the oldest out.
pub const MAX_TOASTS: usize = 4;
/// Over the HUD and its windows, under every modal.
const STACK_Z: i32 = 1400;

/// What a toast is about: it sets the glyph, the tones, the kicker and how long the
/// toast stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    RareItem,
    Party,
    Whisper,
    Weight,
    Zeny,
}

impl ToastKind {
    fn kicker(self) -> &'static str {
        match self {
            Self::RareItem => "RARE ITEM",
            Self::Party => "PARTY",
            Self::Whisper => "WHISPER",
            Self::Weight => "WEIGHT",
            Self::Zeny => "ZENY",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::RareItem => "gem",
            Self::Party => "members",
            Self::Whisper => "whisper",
            Self::Weight => "bag",
            Self::Zeny => "coin",
        }
    }

    /// The ring and kicker tone, then the title's colour.
    fn tones(self) -> (Color, Color) {
        match self {
            Self::RareItem => (ro_dark::RARE_RING, Color::srgb_u8(0xf7, 0xec, 0xd2)),
            Self::Party => (ro_dark::TONE_OK, ro_dark::STRONG),
            Self::Whisper => (ro_dark::TONE_WHISPER, ro_dark::STRONG),
            Self::Weight => (ro_dark::STATUS_BUSY, ro_dark::STRONG),
            Self::Zeny => (ro_dark::BUTTON_STROKE, ro_dark::STRONG),
        }
    }

    /// Seconds the toast stays up.
    fn life(self) -> f32 {
        match self {
            Self::RareItem | Self::Weight => 6.0,
            Self::Party => 5.0,
            Self::Whisper => 8.0,
            Self::Zeny => 4.0,
        }
    }
}

/// What a toast's button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToastAction {
    /// Aims the chat input at a whisper to this hero.
    Reply(String),
    OpenInventory,
}

impl ToastAction {
    /// The button's label and its width.
    fn button(&self) -> (&'static str, f32) {
        match self {
            Self::Reply(_) => ("Reply", 56.0),
            Self::OpenInventory => ("Open Inventory", 108.0),
        }
    }
}

/// Raises a toast. `sub` may be empty.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct ShowToast {
    pub kind: ToastKind,
    pub title: String,
    pub sub: String,
    pub action: Option<ToastAction>,
}

/// A toast on screen and the time it has left.
struct Toast {
    id: u32,
    show: ShowToast,
    life: Timer,
}

/// The toasts on screen, oldest first.
#[derive(Resource, Default)]
pub struct Toasts {
    next_id: u32,
    live: Vec<Toast>,
}

impl Toasts {
    fn push(&mut self, show: ShowToast) {
        self.next_id = self.next_id.wrapping_add(1);
        let life = Timer::from_seconds(show.kind.life(), TimerMode::Once);
        self.live.push(Toast {
            id: self.next_id,
            show,
            life,
        });
        if self.live.len() > MAX_TOASTS {
            self.live.remove(0);
        }
    }

    fn dismiss(&mut self, id: u32) -> Option<ShowToast> {
        let at = self.live.iter().position(|toast| toast.id == id)?;
        Some(self.live.remove(at).show)
    }

    fn ids(&self) -> Vec<u32> {
        self.live.iter().map(|toast| toast.id).collect()
    }
}

/// The corner the toasts stack in.
#[derive(Component, Default, Clone)]
struct ToastStack;

/// A toast's card, by its id. Its `Hovered` holds the stack still.
#[derive(Component, Default, Clone, Copy)]
struct ToastCard;

/// The draining bar along a card's lower edge.
#[derive(Component, Default, Clone, Copy)]
struct ToastLife(u32);

/// A button of a card: the dismiss mark, or the toast's own action.
#[derive(Component, Default, Clone, Copy)]
struct ToastButton {
    id: u32,
    act: bool,
}

pub struct ToastsPlugin;

impl Plugin for ToastsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Toasts>();
        app.add_message::<ShowToast>();
        app.add_systems(
            Update,
            (
                (
                    feeds::rare_items,
                    feeds::party_joins,
                    feeds::whispers,
                    feeds::purse_and_load,
                ),
                ingest,
                age,
                rebuild,
                drain_life_bars,
            )
                .chain()
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(OnExit(GameState::InGame), clear);
    }
}

/// Mounts the empty stack on the HUD.
pub fn spawn_toast_stack(commands: &mut Commands, hud: Entity) {
    commands.spawn_scene(stack()).insert(ChildOf(hud));
}

fn stack() -> impl Scene {
    bsn! {
        ToastStack
        Node {
            position_type: PositionType::Absolute,
            right: px(16),
            bottom: px(16),
            flex_direction: FlexDirection::ColumnReverse,
            row_gap: px(8),
        }
        GlobalZIndex(STACK_Z)
        @ignore_picking()
    }
}

fn ingest(mut shown: MessageReader<ShowToast>, mut toasts: ResMut<Toasts>) {
    for show in shown.read() {
        toasts.push(show.clone());
    }
}

/// Ages the toasts and drops the spent ones, unless the pointer rests on the stack.
fn age(time: Res<Time>, cards: Query<&Hovered, With<ToastCard>>, mut toasts: ResMut<Toasts>) {
    if toasts.live.is_empty() || cards.iter().any(Hovered::get) {
        return;
    }
    for toast in &mut toasts.live {
        toast.life.tick(time.delta());
    }
    toasts.live.retain(|toast| !toast.life.is_finished());
}

/// Redraws the stack when a toast arrives or leaves.
fn rebuild(
    mut commands: Commands,
    toasts: Res<Toasts>,
    stacks: Query<(Entity, Option<&Children>), With<ToastStack>>,
    mut drawn: Local<Vec<u32>>,
) {
    let Ok((stack, children)) = stacks.single() else {
        drawn.clear();
        return;
    };
    let ids = toasts.ids();
    if *drawn == ids {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    // The stack grows upwards, so the newest toast is spawned first.
    for toast in toasts.live.iter().rev() {
        commands
            .spawn_scene(scene::card(toast.id, &toast.show))
            .insert(ChildOf(stack));
    }
    *drawn = ids;
}

fn drain_life_bars(toasts: Res<Toasts>, mut bars: Query<(&ToastLife, &mut Node)>) {
    for (bar, mut node) in &mut bars {
        let Some(toast) = toasts.live.iter().find(|toast| toast.id == bar.0) else {
            continue;
        };
        node.width = Val::Percent(toast.life.fraction_remaining() * 100.0);
    }
}

/// Dismisses the toast and, for its own button, does what the toast offers.
fn on_toast_button(
    activate: On<Activate>,
    buttons: Query<&ToastButton>,
    mut toasts: ResMut<Toasts>,
    chat: Option<ResMut<ChatUi>>,
    inventory: Option<ResMut<InventoryUi>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(show) = toasts.dismiss(button.id) else {
        return;
    };
    if !button.act {
        return;
    }
    match (show.action, chat, inventory) {
        (Some(ToastAction::Reply(name)), Some(mut chat), _) => chat.aim_whisper(&name),
        (Some(ToastAction::OpenInventory), _, Some(mut inventory)) => inventory.open = true,
        _ => {}
    }
}

fn clear(mut toasts: ResMut<Toasts>) {
    toasts.live.clear();
}
