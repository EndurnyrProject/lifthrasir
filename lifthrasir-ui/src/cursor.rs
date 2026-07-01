//! Native RO cursor.
//!
//! The Ragnarok cursors are static single-frame PNGs (one per [`CursorType`]), so
//! Bevy's built-in custom cursor does the whole job — no sprite-sheet crate, no
//! hand-drawn UI node, no hiding the OS cursor (the custom image *is* the OS
//! cursor). The engine drives [`CurrentCursorType`] from terrain/hover state.
//!
//! We do **not** insert `CursorIcon` on the window directly: `bevy_feathers`'
//! `CursorIconPlugin` runs its own `update_cursor` every `PreUpdate` and would
//! overwrite our cursor back to the OS arrow on the next frame, leaving the RO
//! cursor visible only for the single frame its type changed. Instead we feed the
//! RO cursor into Feathers' `OverrideCursor` resource, making Feathers the single
//! authority that mirrors it onto the window (with its own change detection) and
//! ensuring the RO cursor wins even over Feathers widgets that set their own.

use bevy::prelude::*;
use bevy::window::{CustomCursor, CustomCursorImage};
use bevy_feathers::cursor::{EntityCursor, OverrideCursor};
use game_engine::domain::input::{CurrentCursorType, CursorType};

/// `AssetServer` path (relative to `assets/`) holding the extracted cursor PNGs.
const CURSOR_DIR: &str = "data/textures/ui/cursors";

pub struct NativeCursorPlugin;

impl Plugin for NativeCursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AppliedCursor>();
        app.add_systems(Startup, load_cursor_textures);
        app.add_systems(Update, apply_cursor);
    }
}

#[derive(Resource)]
struct CursorTextures {
    default: Handle<Image>,
    add: Handle<Image>,
    attack: Handle<Image>,
    impossible: Handle<Image>,
    talk: Handle<Image>,
}

impl CursorTextures {
    fn handle(&self, cursor: CursorType) -> Handle<Image> {
        match cursor {
            CursorType::Default => self.default.clone(),
            CursorType::Add => self.add.clone(),
            CursorType::Attack => self.attack.clone(),
            CursorType::Impossible => self.impossible.clone(),
            CursorType::Talk => self.talk.clone(),
        }
    }
}

/// Last cursor type pushed to the window, so we only re-insert on a real change.
#[derive(Resource, Default)]
struct AppliedCursor(Option<CursorType>);

/// Click point within the cursor image, in image pixels. Every RO cursor here is
/// the same gold arrowhead whose pointing tip sits in the top-left corner (the
/// crosshair/plus badges are decoration), so the hotspot is the tip for all of
/// them. Aligning it with the tip is what makes clicks land where the cursor
/// points; a centered hotspot offsets every click by ~16px and makes small
/// targets (window buttons) unclickable.
fn hotspot(_cursor: CursorType) -> (u16, u16) {
    (1, 1)
}

fn load_cursor_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |name: &str| asset_server.load(format!("{CURSOR_DIR}/{name}"));
    commands.insert_resource(CursorTextures {
        default: load("cursor_default.png"),
        add: load("cursor_add.png"),
        attack: load("cursor_attack.png"),
        impossible: load("cursor_impossible.png"),
        talk: load("cursor_talk.png"),
    });
}

/// Feeds the current cursor image into Feathers' `OverrideCursor` once its PNG has
/// loaded. Gating on load avoids winit's per-frame "image not loaded yet" warning,
/// and the `AppliedCursor` guard rebuilds the override only when the type changes.
fn apply_cursor(
    current: Res<CurrentCursorType>,
    textures: Res<CursorTextures>,
    images: Res<Assets<Image>>,
    mut applied: ResMut<AppliedCursor>,
    mut override_cursor: ResMut<OverrideCursor>,
) {
    let desired = current.get();
    if applied.0 == Some(desired) {
        return;
    }
    let handle = textures.handle(desired);
    if images.get(&handle).is_none() {
        return;
    }
    override_cursor.0 = Some(EntityCursor::Custom(CustomCursor::Image(
        CustomCursorImage {
            handle,
            texture_atlas: None,
            flip_x: false,
            flip_y: false,
            rect: None,
            hotspot: hotspot(desired),
        },
    )));
    applied.0 = Some(desired);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspot_is_the_arrow_tip_for_every_cursor() {
        for cursor in [
            CursorType::Default,
            CursorType::Add,
            CursorType::Attack,
            CursorType::Impossible,
            CursorType::Talk,
        ] {
            assert_eq!(hotspot(cursor), (1, 1));
        }
    }
}
