use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// One-shot factor ramp. Lives on the parent of a procedural-effect tree and
/// drives each child `FactorMaterial`'s 0→1 `factor` over its lifetime; the tree
/// self-despawns when the timer finishes. This is the ECS equivalent of the
/// Godot `AnimationPlayer` ramping a shader's `grow_factor`/`animation_factor`.
#[derive(Component)]
pub struct FactorRamp {
    pub timer: Timer,
}

impl FactorRamp {
    pub fn new(seconds: f32) -> Self {
        Self {
            timer: Timer::from_seconds(seconds, TimerMode::Once),
        }
    }
}

/// A material animated by a single 0..1 `factor` supplied by a `FactorRamp`.
pub trait FactorMaterial: Asset {
    fn set_factor(&mut self, factor: f32);
}

/// Advance each ramp, write its 0→1 fraction into the child materials of type
/// `M`, and despawn the finished parent (recursively taking its children).
pub fn drive_factor<M: FactorMaterial + Material>(
    time: Res<Time>,
    mut commands: Commands,
    mut ramps: Query<(Entity, &mut FactorRamp, &Children)>,
    handles: Query<&MeshMaterial3d<M>>,
    mut materials: ResMut<Assets<M>>,
) {
    for (entity, mut ramp, children) in &mut ramps {
        ramp.timer.tick(time.delta());
        let factor = ramp.timer.fraction();
        for child in children.iter() {
            let Ok(handle) = handles.get(child) else {
                continue;
            };
            if let Some(mut material) = materials.get_mut(&handle.0) {
                material.set_factor(factor);
            }
        }
        if ramp.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Shared assets for procedural impact effects. Holds a single unit-quad mesh
/// reused by every billboard layer (camera-facing is done in the vertex shader).
#[derive(Resource)]
pub struct ImpactAssets {
    pub quad: Handle<Mesh>,
}

impl FromWorld for ImpactAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Rectangle::from_size(Vec2::ONE)));
        Self { quad }
    }
}

/// Unlit radial hit-flash material. Grows and streaks with `params.factor`,
/// camera-facing done in the vertex stage. See `assets/data/effects/impact_core.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ImpactCoreMaterial {
    #[uniform(0)]
    pub params: ImpactParams,
}

/// Packed impact-core parameters. Field order and types must match the
/// `ImpactParams` struct in `impact_core.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct ImpactParams {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    /// x=emission y=streak_amount z=edge_hardness w=edge_position
    pub shape: Vec4,
    pub factor: f32,
}

impl Material for ImpactCoreMaterial {
    fn vertex_shader() -> ShaderRef {
        "ro://effects/impact_core.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "ro://effects/impact_core.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

impl FactorMaterial for ImpactCoreMaterial {
    fn set_factor(&mut self, factor: f32) {
        self.params.factor = factor;
    }
}

/// Unlit additive four-point glint material. Fades with `params.factor`,
/// camera-facing done in the vertex stage. See `assets/data/effects/four_point_star.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarMaterial {
    #[uniform(0)]
    pub params: StarParams,
}

/// Packed four-point-star parameters. Field order and types must match the
/// `StarParams` struct in `four_point_star.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct StarParams {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    /// x=emission y=star_shape z=star_smoothness
    pub shape: Vec4,
    pub factor: f32,
}

impl Material for StarMaterial {
    fn vertex_shader() -> ShaderRef {
        "ro://effects/four_point_star.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "ro://effects/four_point_star.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

impl FactorMaterial for StarMaterial {
    fn set_factor(&mut self, factor: f32) {
        self.params.factor = factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::AsBindGroup;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Asset, TypePath, AsBindGroup, Clone)]
    struct StubMaterial {
        factor: f32,
    }

    impl Material for StubMaterial {}

    impl FactorMaterial for StubMaterial {
        fn set_factor(&mut self, factor: f32) {
            self.factor = factor;
        }
    }

    #[test]
    fn ramp_drives_factor_to_one_then_despawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<StubMaterial>()
            .add_systems(Update, drive_factor::<StubMaterial>);

        let handle = app
            .world_mut()
            .resource_mut::<Assets<StubMaterial>>()
            .add(StubMaterial { factor: 0.0 });

        let parent = app.world_mut().spawn(FactorRamp::new(0.3)).id();
        app.world_mut()
            .spawn((MeshMaterial3d(handle.clone()), ChildOf(parent)));

        // Warm-up establishes the time baseline (zero delta).
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();

        // Advance past the 0.3s ramp in sub-max_delta chunks.
        for _ in 0..3 {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.2,
            )));
            app.update();
        }

        let factor = app
            .world()
            .resource::<Assets<StubMaterial>>()
            .get(&handle)
            .expect("material asset survives the tree despawn")
            .factor;
        assert!((factor - 1.0).abs() < 1e-4, "factor reached 1.0, got {factor}");
        assert!(
            app.world().get::<FactorRamp>(parent).is_none(),
            "the ramp parent despawns on completion"
        );
    }

    #[test]
    fn factor_materials_write_factor() {
        let bank = Vec4::ONE;
        let mut core = ImpactCoreMaterial {
            params: ImpactParams {
                primary_color: bank,
                secondary_color: bank,
                shape: bank,
                factor: 0.0,
            },
        };
        let mut star = StarMaterial {
            params: StarParams {
                primary_color: bank,
                secondary_color: bank,
                shape: bank,
                factor: 0.0,
            },
        };

        core.set_factor(0.5);
        star.set_factor(0.5);

        assert_eq!(core.params.factor, 0.5);
        assert_eq!(star.params.factor, 0.5);
    }
}
