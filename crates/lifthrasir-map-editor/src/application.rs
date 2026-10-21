//! Editor session and document commands. No renderer or asset-source policy.

use bevy::prelude::*;
use lifthrasir_map_editor::MapDocument;

#[derive(Resource)]
pub(crate) struct EditorDocument(pub MapDocument);

#[derive(Resource, Default)]
pub(crate) struct EditorNotice {
    pub message: String,
    pub expires: Option<Timer>,
}

impl EditorNotice {
    pub fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.expires = Some(Timer::from_seconds(3.0, TimerMode::Once));
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) enum EditorSystems {
    Edit,
    History,
    Save,
    Present,
}

pub(crate) struct EditorSessionPlugin;

impl Plugin for EditorSessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorNotice>()
            .configure_sets(
                Update,
                (
                    EditorSystems::Edit,
                    EditorSystems::History,
                    EditorSystems::Save,
                    EditorSystems::Present,
                )
                    .chain(),
            )
            .add_systems(Update, undo_redo.in_set(EditorSystems::History))
            .add_systems(Update, save_document.in_set(EditorSystems::Save));
    }
}

fn undo_redo(keys: Res<ButtonInput<KeyCode>>, mut document: ResMut<EditorDocument>) {
    if !control_pressed(&keys) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        document.0.undo();
    } else if keys.just_pressed(KeyCode::KeyY) {
        document.0.redo();
    }
}

/// The only application command that writes the document to disk.
fn save_document(
    keys: Res<ButtonInput<KeyCode>>,
    mut document: ResMut<EditorDocument>,
    mut notice: ResMut<EditorNotice>,
) {
    if !control_pressed(&keys) || !keys.just_pressed(KeyCode::KeyS) {
        return;
    }
    match document.0.save() {
        Ok(()) => notice.show("Saved map GLB"),
        Err(error) => {
            error!("saving map: {error:#}");
            notice.show(format!("Save failed: {error}"));
        }
    }
}

pub(crate) fn control_pressed(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn only_control_s_publishes_edits() {
        let (_directory, mut document) = test_support::document();
        let path = document.source_path.clone();
        let original_bytes = std::fs::read(&path).unwrap();
        document
            .update_prop_transform(0, [3.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .unwrap();
        document.commit_pending_prop_transform();
        let mut app = App::new();
        app.insert_resource(EditorDocument(document))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(EditorSessionPlugin);

        for _ in 0..20 {
            app.update();
        }
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
        assert!(app.world().resource::<EditorDocument>().0.is_dirty());

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
        app.update();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original_bytes,
            "S alone is not save"
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyS);
        }
        app.update();
        let saved = MapDocument::open(&path).unwrap();
        assert_eq!(saved.prop_transform(0).unwrap().0, [3.0, 0.0, 0.0]);
        assert!(!app.world().resource::<EditorDocument>().0.is_dirty());
    }

    #[test]
    fn history_commands_do_not_require_viewport_entities_or_write_files() {
        let (_directory, mut document) = test_support::document();
        let path = document.source_path.clone();
        let original_bytes = std::fs::read(&path).unwrap();
        document
            .set_prop_transform(0, [3.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3])
            .unwrap();
        let mut app = App::new();
        app.insert_resource(EditorDocument(document))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(EditorSessionPlugin);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyZ);
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<EditorDocument>()
                .0
                .prop_transform(0)
                .unwrap()
                .0,
            [0.0; 3]
        );
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(KeyCode::ControlRight);
            keys.press(KeyCode::KeyY);
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<EditorDocument>()
                .0
                .prop_transform(0)
                .unwrap()
                .0,
            [3.0, 0.0, 0.0]
        );
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
    }
}
