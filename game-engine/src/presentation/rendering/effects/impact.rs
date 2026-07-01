use bevy::prelude::*;

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
}
