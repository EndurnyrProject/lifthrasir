use bevy::prelude::*;
use game_engine::{core::state::GameState, domain::world::navigation::NavigationSystems};

use crate::widgets::minimap::MinimapStateSync;

pub mod feedback;
mod minimap_path;
pub mod panel;
pub mod slash;

pub struct NavigationUiPlugin;

impl Plugin for NavigationUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<minimap_path::NavigationSegmentKey>()
            .add_message::<slash::NaviSlash>()
            .add_systems(
                Update,
                (
                    minimap_path::reconcile_navigation_segments
                        .in_set(NavigationSystems::ViewSync)
                        .after(MinimapStateSync),
                    panel::sync_navigation_panel.in_set(NavigationSystems::ViewSync),
                    slash::dispatch_navi_slash,
                    feedback::ingest_navigation_feedback,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                minimap_path::reset_navigation_segments,
            );
    }
}
