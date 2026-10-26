//! Box selection: dragging from empty space in the Select tool selects the
//! object nodes whose origin lands inside the rectangle. Shift/Ctrl adds to
//! the current selection; a click without a drag clears it.

use super::{
    gizmo::TransformGizmo,
    node::EditorNodeId,
    pick::Hovered,
    selection::{Selection, extends_selection},
    ui::{EditorUiSet, ViewportCursor},
};
use crate::map::{layout::MapLayout, open::EditorScreen, tools::ToolMode};
use bevy::{prelude::*, window::PrimaryWindow};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

/// Screen distance (logical px) a press must travel to count as a drag.
const DRAG_THRESHOLD: f32 = 4.0;

/// The box being dragged, in window pixels.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Marquee(pub Option<(Vec2, Vec2)>);

impl Marquee {
    fn rect(&self) -> Option<Rect> {
        self.0.map(|(start, end)| Rect::from_corners(start, end))
    }

    fn is_drag(&self) -> bool {
        self.0
            .is_some_and(|(start, end)| start.distance(end) > DRAG_THRESHOLD)
    }
}

/// The nodes whose screen position lies inside `rect`, in the given order.
pub fn nodes_in_rect(
    rect: Rect,
    points: impl IntoIterator<Item = (EditorNodeId, Vec2)>,
) -> Vec<EditorNodeId> {
    points
        .into_iter()
        .filter(|(_, screen)| rect.contains(*screen))
        .map(|(id, _)| id)
        .collect()
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Marquee>()
        .add_systems(OnExit(EditorScreen::Editing), reset)
        .add_systems(
            Update,
            drag_marquee.run_if(
                in_state(EditorScreen::Editing).and_then(resource_equals(ToolMode::Select)),
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            draw_marquee
                .in_set(EditorUiSet::Windows)
                .run_if(in_state(EditorScreen::Editing)),
        );
}

fn reset(mut marquee: ResMut<Marquee>) {
    marquee.0 = None;
}

/// Starts on a press over empty viewport space (not on a node or the gizmo),
/// follows the cursor, and applies the box on release.
#[allow(clippy::too_many_arguments)]
fn drag_marquee(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<Hovered>,
    gizmo: Option<Res<TransformGizmo>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cursor: ViewportCursor,
    layout: Option<Res<MapLayout>>,
    children: Query<&Children>,
    nodes: Query<(&EditorNodeId, &GlobalTransform)>,
    mut marquee: ResMut<Marquee>,
    mut selection: ResMut<Selection>,
) {
    let position = windows.single().ok().and_then(Window::cursor_position);
    if mouse.just_pressed(MouseButton::Left) {
        let free = hovered.0.is_none() && !gizmo.is_some_and(|gizmo| gizmo.is_focused());
        marquee.0 = position
            .filter(|_| free && cursor.ray().is_some())
            .map(|position| (position, position));
        return;
    }
    let Some((start, _)) = marquee.0 else {
        return;
    };
    if let Some(position) = position {
        marquee.0 = Some((start, position));
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (dragged, rect) = (marquee.is_drag(), marquee.rect());
    marquee.0 = None;
    let extend = extends_selection(&keys);
    let Some(rect) = rect.filter(|_| dragged) else {
        if !extend {
            selection.clear();
        }
        return;
    };
    let Some(layout) = layout else {
        return;
    };
    let points = children
        .get(layout.map_node)
        .into_iter()
        .flatten()
        .filter(|&&node| Some(node) != layout.terrain_node)
        .filter_map(|&node| {
            let (id, global) = nodes.get(node).ok()?;
            let (screen, _) = cursor.to_screen(global.translation())?;
            Some((*id, screen))
        });
    let inside = nodes_in_rect(rect, points);
    if extend {
        selection.extend(inside);
    } else {
        selection.replace(inside);
    }
}

fn draw_marquee(mut contexts: EguiContexts, marquee: Res<Marquee>) -> Result {
    let Some(rect) = marquee.rect().filter(|_| marquee.is_drag()) else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x, rect.min.y),
        egui::pos2(rect.max.x, rect.max.y),
    );
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("marquee"),
    ));
    let color = egui::Color32::from_rgb(255, 153, 26);
    painter.rect_filled(rect, 0.0, color.gamma_multiply(0.15));
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_points_inside_the_box_are_picked_in_order() {
        let rect = Rect::from_corners(Vec2::new(10.0, 10.0), Vec2::new(50.0, 50.0));
        let points = [
            (EditorNodeId(1), Vec2::new(20.0, 20.0)),
            (EditorNodeId(2), Vec2::new(80.0, 20.0)),
            (EditorNodeId(3), Vec2::new(40.0, 45.0)),
        ];

        assert_eq!(
            nodes_in_rect(rect, points),
            [EditorNodeId(1), EditorNodeId(3)]
        );
    }
}
