//! The floating Settings window: per-user editor preferences.

use super::session::{EditorSession, persist_preferences};
use crate::shell::ui::EditorUiSet;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

/// Whether the Settings window is shown.
#[derive(Resource, Default)]
pub struct PreferencesWindow {
    pub open: bool,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PreferencesWindow>().add_systems(
        EguiPrimaryContextPass,
        preferences_window.in_set(EditorUiSet::Windows),
    );
}

fn preferences_window(
    mut contexts: EguiContexts,
    mut window: ResMut<PreferencesWindow>,
    mut session: ResMut<EditorSession>,
) -> Result {
    if !window.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut changed = false;
    egui::Window::new("Settings")
        .open(&mut window.open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let preferences = &mut session.preferences;
            ui.heading("Session");
            changed |= ui
                .checkbox(
                    &mut preferences.reopen_last_session,
                    "Reopen last map at startup",
                )
                .changed();
            ui.separator();
            ui.heading("Autosave");
            changed |= ui
                .checkbox(
                    &mut preferences.autosave_enabled,
                    "Write recovery snapshots of unsaved edits",
                )
                .changed();
            ui.horizontal(|ui| {
                ui.add_enabled_ui(preferences.autosave_enabled, |ui| {
                    ui.label("Interval");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut preferences.autosave_interval_secs)
                                .range(5..=3600)
                                .suffix(" s"),
                        )
                        .changed();
                });
            });
        });
    if changed {
        persist_preferences(&session);
    }
    Ok(())
}
