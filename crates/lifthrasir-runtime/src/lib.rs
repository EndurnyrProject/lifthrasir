//! Content presentation independent of client state and networking.

#[cfg(test)]
#[path = "../tests/support/map_fixtures.rs"]
mod map_fixtures;

pub mod actors;
pub use actors::ActorRenderPlugin;
pub mod audio;
pub use audio::ContentAudioPlugin;
pub mod effects;
pub use effects::EffectsPlaybackPlugin;
pub mod rendering;
pub mod world;
pub use world::WorldRenderPlugin;

/// Full content presentation after renderer, content assets and catalogs.
/// Disable ContentAudioPlugin explicitly for silent or device-free hosts.
pub struct ContentRuntimePlugins;

impl PluginGroup for ContentRuntimePlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        bevy::app::PluginGroupBuilder::start::<Self>()
            .add(RuntimeCorePlugin)
            .add(ContentAudioPlugin)
            .add(ActorRenderPlugin)
            .add(EffectsPlaybackPlugin)
            .add(WorldRenderPlugin)
    }
}

use audio::events::*;
use bevy::prelude::*;
use effects::events::PlayProceduralVfx;
use rendering::{SpriteRenderingSystems, WaterRenderingSystems};

/// Common presentation messages, billboard resources, and ungated rendering phases.
/// Requires mesh asset storage; no audio device or client resources are installed.
pub struct RuntimeCorePlugin;

impl Plugin for RuntimeCorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(rendering::billboard::BillboardPlugin)
            .add_message::<PlayProceduralVfx>()
            .add_message::<PlayBgmEvent>()
            .add_message::<StopBgmEvent>()
            .add_message::<SetBgmVolumeEvent>()
            .add_message::<MuteBgmEvent>()
            .add_message::<PlayMobSfx>()
            .add_message::<PlaySkillSfx>()
            .add_message::<PlaySoundEffect>()
            .add_message::<SetSfxVolumeEvent>()
            .add_message::<MuteSfxEvent>()
            .add_message::<SetAmbienceVolumeEvent>()
            .add_message::<MuteAmbienceEvent>()
            .configure_sets(
                Update,
                (
                    SpriteRenderingSystems::HierarchySpawn,
                    SpriteRenderingSystems::AssetPopulation,
                    SpriteRenderingSystems::AnimationEvents,
                    SpriteRenderingSystems::AnimationSync,
                    SpriteRenderingSystems::AnimationMarkers,
                    SpriteRenderingSystems::AnimationAdvance,
                    SpriteRenderingSystems::TransformUpdate,
                    SpriteRenderingSystems::AnimationPlayback,
                    SpriteRenderingSystems::OrphanCleanup,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                (
                    WaterRenderingSystems::WaterLoading,
                    WaterRenderingSystems::WaterFinalization,
                    WaterRenderingSystems::WaterAnimation,
                )
                    .chain(),
            );
    }
}
