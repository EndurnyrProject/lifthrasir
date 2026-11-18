use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use crate::domain::camera::components::CameraFollowSettings;
use crate::domain::entities::character::states::status_icons::{
    LocalStatuses, expire_local_statuses, track_local_status_icons,
};

const EFST_ILLUSION: u32 = 34;

#[derive(Component, Default, Clone, Copy, ExtractComponent, ShaderType)]
#[extract_app(RenderApp)]
struct Hallucination {
    time: f32,
}

impl FullscreenMaterial for Hallucination {
    fn fragment_shader() -> ShaderRef {
        "ro://shaders/hallucination.wesl".into()
    }
}

pub struct HallucinationPlugin;

impl Plugin for HallucinationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<Hallucination>::default())
            .add_systems(
                Update,
                sync_hallucination
                    .after(track_local_status_icons)
                    .after(expire_local_statuses),
            );
    }
}

fn sync_hallucination(
    mut commands: Commands,
    statuses: Res<LocalStatuses>,
    time: Res<Time>,
    cameras: Query<(Entity, Option<&Hallucination>), With<CameraFollowSettings>>,
) {
    let active = statuses.active.contains_key(&EFST_ILLUSION);
    for (entity, current) in &cameras {
        if active {
            commands.entity(entity).insert(Hallucination {
                time: time.elapsed_secs(),
            });
        } else if current.is_some() {
            commands.entity(entity).remove::<Hallucination>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::character::states::status_icons::ActiveStatus;

    #[test]
    fn hallucination_follows_local_status_and_only_affects_follow_camera() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<LocalStatuses>()
            .add_systems(Update, sync_hallucination);
        let camera = app
            .world_mut()
            .spawn((Camera3d::default(), CameraFollowSettings::default()))
            .id();
        let other = app.world_mut().spawn(Camera3d::default()).id();
        app.world_mut()
            .resource_mut::<LocalStatuses>()
            .active
            .insert(
                EFST_ILLUSION,
                ActiveStatus {
                    total_ms: 0,
                    expires_at: None,
                    permanent: true,
                    source_skill_id: None,
                },
            );
        app.update();
        assert!(app.world().entity(camera).contains::<Hallucination>());
        assert!(!app.world().entity(other).contains::<Hallucination>());
        app.world_mut()
            .resource_mut::<LocalStatuses>()
            .active
            .clear();
        app.update();
        assert!(!app.world().entity(camera).contains::<Hallucination>());
    }
}
