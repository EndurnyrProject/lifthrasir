//! A color stroke is one undoable step that changes vertex colors only within
//! the brush radius.

mod common;

use bevy::prelude::*;
use lifthrasir_editor::{
    map::{
        document::collect_document,
        tools::color::{ColorBrush, begin_stroke, color_step, end_stroke},
    },
    shell::undo::{UndoStack, undo},
};

/// Every terrain vertex as (native XZ, color), in document order.
fn vertex_colors(
    app: &mut App,
    layout: &lifthrasir_editor::map::layout::MapLayout,
) -> Vec<(Vec2, [f32; 4])> {
    collect_document(app.world_mut(), layout, false)
        .unwrap()
        .terrain
        .iter()
        .flat_map(|primitive| {
            primitive
                .positions
                .iter()
                .zip(&primitive.colors)
                .map(|(p, c)| (Vec2::new(p[0], p[2]), *c))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn a_stroke_recolors_only_inside_its_radius_and_undoes_exactly() {
    let data_folder = tempfile::tempdir().unwrap();
    let mut app = common::editor_app(data_folder.path());
    let layout = common::open_fixture(&mut app);
    let before = vertex_colors(&mut app, &layout);
    let brush = ColorBrush {
        color: [0.1, 0.2, 0.3, 1.0],
        radius: 12.0,
        strength: 2.0,
    };
    // World (4, y, -4) is native (4, 4): it reaches the corners at (0, 0),
    // (10, 0) and (0, 10), not (10, 10) or beyond.
    let hit = Vec3::new(4.0, 0.0, -4.0);
    let native = Vec2::new(4.0, 4.0);

    begin_stroke(app.world_mut());
    for _ in 0..3 {
        color_step(app.world_mut(), hit, brush, 0.1);
    }
    end_stroke(app.world_mut());

    assert!(!app.world().resource::<UndoStack>().is_pristine());
    let after = vertex_colors(&mut app, &layout);
    assert_eq!(after.len(), before.len());
    let mut changed = 0;
    for ((position, old), (_, new)) in before.iter().zip(&after) {
        if position.distance(native) > brush.radius {
            assert_eq!(old, new, "vertex at {position} is outside the brush");
        } else if old != new {
            changed += 1;
        }
    }
    assert!(changed > 0);

    undo(app.world_mut());
    assert!(app.world().resource::<UndoStack>().is_pristine());
    let restored = vertex_colors(&mut app, &layout);
    let bits = |colors: &[(Vec2, [f32; 4])]| -> Vec<[u32; 4]> {
        colors.iter().map(|(_, c)| c.map(f32::to_bits)).collect()
    };
    assert_eq!(bits(&restored), bits(&before));
}
