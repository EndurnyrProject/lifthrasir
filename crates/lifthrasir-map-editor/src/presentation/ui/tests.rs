use super::*;
use crate::presentation::{MapAssetPath, terrain_grid::TerrainGridStyle};

#[derive(Resource, Default)]
struct StyleChanges(usize);

fn count_style_changes(style: Res<TerrainGridStyle>, mut changes: ResMut<StyleChanges>) {
    if style.is_changed() {
        changes.0 += 1;
    }
}

#[test]
fn editor_ui_runs_without_a_renderer_and_does_not_change_grid_style_while_idle() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        WindowPlugin::default(),
    ))
    .init_resource::<TerrainGridStyle>()
    .init_resource::<StyleChanges>()
    .init_resource::<EditorNotice>()
    .add_systems(Update, count_style_changes)
    .insert_resource(MapAssetPath(None))
    .add_plugins(EditorUiPlugin);
    // Tests must not read or write the user's saved window layout.
    let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
    let primary = contexts.primary_id().unwrap().unwrap();
    contexts
        .configure(primary, |context| {
            context
                .set_ini_filename::<std::path::PathBuf>(None)
                .unwrap();
        })
        .unwrap();
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<EditorUiState>().settings_open);
    assert_eq!(app.world().resource::<StyleChanges>().0, 1);
    assert_eq!(app.world().resource::<TerrainGridStyle>().line_width, 0.08);
    assert_eq!(
        app.world().resource::<TerrainGridStyle>().color,
        [0.2, 0.9, 1.0]
    );
    app.world_mut()
        .resource_mut::<EditorUiState>()
        .settings_open = true;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<StyleChanges>().0, 1);
    assert_eq!(app.world().resource::<TerrainGridStyle>().line_width, 0.08);
    assert_eq!(
        app.world().resource::<TerrainGridStyle>().color,
        [0.2, 0.9, 1.0]
    );
}
