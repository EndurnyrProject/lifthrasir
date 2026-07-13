//! The Console's Character tab: the paperdoll (equipment slots either side of the live
//! render preview) over the attributes rail + combat readout.
//!
//! [`rebuild_character_body`] projects the tab's structure into the shell's
//! [`CharacterTabBody`] container once, when that container is first mounted. Unlike
//! the Bag tab, the Character body is NOT respawned on data change: its three parts all
//! update in place via marker-keyed systems — [`equip::sync_console_equipment_slots`]
//! patches the slot wells, [`attributes::update_console_attributes`] patches the stat /
//! combat text, and [`preview::manage_console_preview`] owns the render-to-texture rig.
//! Respawning would destroy the captured slot-part entities and the preview `ImageNode`
//! binding, so build-once + patch-in-place is the correct shape (it mirrors the old
//! equipment/status windows' static chrome).
//!
//! [`register`] wires this tab's resources + systems into `CharacterWindowPlugin`; it is
//! called from the plugin's `build`.

use bevy::prelude::*;
use game_engine::core::state::GameState;
use game_engine::domain::entities::character::events::forward_character_sprite_events;

use crate::widgets::chrome::ignore_picking;

use super::CharacterTabBody;

pub mod attributes;
pub mod equip;
pub mod preview;

/// Build the Character tab body once, the frame its container is first mounted. After
/// that the per-part sync systems patch it in place (see the module docs).
pub fn rebuild_character_body(
    mut commands: Commands,
    bodies: Query<(Entity, Ref<CharacterTabBody>)>,
) {
    let Ok((entity, body_ref)) = bodies.single() else {
        return;
    };
    if !body_ref.is_added() {
        return;
    }
    commands.spawn_scene(body()).insert(ChildOf(entity));
}

fn body() -> impl Scene {
    bsn! {
        Node { flex_direction: FlexDirection::Column, row_gap: px(14) }
        ignore_picking()
        Children [ paperdoll_row(), attributes::attributes_panel() ]
    }
}

/// Left slot column, center render preview, right slot column.
fn paperdoll_row() -> impl Scene {
    bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(12),
        }
        ignore_picking()
        Children [
            equip::slot_column(&equip::LEFT_SLOTS),
            preview::preview_frame(),
            equip::slot_column(&equip::RIGHT_SLOTS),
        ]
    }
}

/// Register the Character tab's resources + systems into `CharacterWindowPlugin`.
pub fn register(app: &mut App) {
    app.init_resource::<attributes::CharStatStaging>();
    app.init_resource::<equip::CharLastSlotClick>();
    app.init_resource::<preview::ConsolePreviewState>();
    app.init_resource::<preview::ConsoleLocalHeadgear>();

    app.add_systems(
        Update,
        rebuild_character_body.run_if(in_state(GameState::InGame)),
    );
    app.add_systems(
        Update,
        (
            equip::sync_console_equipment_slots,
            attributes::update_console_attributes.run_if(attributes::console_attributes_changed),
            preview::cache_local_headgear,
            preview::manage_console_preview.after(forward_character_sprite_events),
            preview::forward_preview_headgear,
            preview::tag_preview_billboards,
        )
            .run_if(in_state(GameState::InGame)),
    );
    app.add_systems(OnExit(GameState::InGame), preview::cleanup_preview);
}
