//! Reusable drag-by-titlebar behavior for in-game windows.
//!
//! Attach `make_draggable` to a window's titlebar; dragging it offsets the window
//! root `Node`'s `left`/`top` by the pointer delta. This is the pattern future
//! Inventory/Skill/etc. windows reuse.

use bevy::prelude::*;

/// Marks a titlebar as a drag handle, carrying the window-root `Entity` to move.
#[derive(Component)]
pub struct DraggableWindow {
    pub window_root: Entity,
}

/// Make `titlebar` drag `window_root` by attaching a `Pointer<Drag>` observer.
///
/// Both the titlebar and the window root must be `Pickable` (NOT `Pickable::IGNORE`)
/// for drag pickups to register, and the window root must be spawned with explicit
/// `Val::Px` `left`/`top` so the delta accumulates against a real offset (a non-px
/// value is treated as `0.0`).
pub fn make_draggable(commands: &mut Commands, titlebar: Entity, window_root: Entity) {
    commands
        .entity(titlebar)
        .insert(DraggableWindow { window_root })
        .observe(on_drag);
}

fn on_drag(
    drag: On<Pointer<Drag>>,
    handles: Query<&DraggableWindow>,
    mut nodes: Query<&mut Node>,
) {
    let Ok(handle) = handles.get(drag.entity) else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(handle.window_root) else {
        return;
    };
    node.left = Val::Px(px_or_zero(node.left) + drag.delta.x);
    node.top = Val::Px(px_or_zero(node.top) + drag.delta.y);
}

fn px_or_zero(val: Val) -> f32 {
    match val {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_or_zero_treats_non_px_as_zero() {
        assert_eq!(px_or_zero(Val::Px(12.0)), 12.0);
        assert_eq!(px_or_zero(Val::Auto), 0.0);
        assert_eq!(px_or_zero(Val::Percent(50.0)), 0.0);
    }

    #[test]
    fn make_draggable_attaches_handle_to_titlebar() {
        let mut world = World::new();
        let window_root = world.spawn(Node::default()).id();
        let titlebar = world.spawn(Node::default()).id();

        let mut commands = world.commands();
        make_draggable(&mut commands, titlebar, window_root);
        world.flush();

        let handle = world.get::<DraggableWindow>(titlebar).unwrap();
        assert_eq!(handle.window_root, window_root);
    }
}
