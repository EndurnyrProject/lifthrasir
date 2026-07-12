use super::VfxSystems;
use crate::domain::effects::PlayProceduralVfx;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy_hanabi::{
    Attribute, ColorOverLifetimeModifier, EffectAsset, ExprWriter, Gradient, LinearDragModifier,
    ParticleEffect, SetAttributeModifier, SetPositionSphereModifier, SetVelocitySphereModifier,
    ShapeDimension, SizeOverLifetimeModifier, SpawnerSettings,
};

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

/// Tick every `FactorRamp` once per frame and despawn parents whose ramp
/// finished. Kept separate from `drive_factor<M>` so the timer advances exactly
/// once regardless of how many material driver systems are registered.
///
/// The finished check runs BEFORE the tick, so a ramp is despawned the frame
/// AFTER it completes: the completing frame ticks to `fraction() == 1.0` and the
/// `drive_factor<M>` readers write that final 1.0 into the materials (and render
/// it); the next frame despawns the parent (recursively taking its children).
/// Decoupling despawn from the completing frame keeps this correct regardless of
/// its ordering relative to the readers and of auto-inserted sync points.
pub fn advance_ramps(
    time: Res<Time>,
    mut commands: Commands,
    mut ramps: Query<(Entity, &mut FactorRamp)>,
) {
    for (entity, mut ramp) in &mut ramps {
        if ramp.timer.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        ramp.timer.tick(time.delta());
    }
}

/// Read each ramp's 0→1 fraction and write it into the child materials of type
/// `M`. Read-only w.r.t. the timer: ticking and despawn live in `advance_ramps`,
/// so registering this once per material type does not double-advance the ramp.
pub fn drive_factor<M: FactorMaterial + Material>(
    ramps: Query<(&FactorRamp, &Children)>,
    handles: Query<&MeshMaterial3d<M>>,
    mut materials: ResMut<Assets<M>>,
) {
    for (ramp, children) in &ramps {
        let factor = ramp.timer.fraction();
        for child in children.iter() {
            let Ok(handle) = handles.get(child) else {
                continue;
            };
            if let Some(mut material) = materials.get_mut(&handle.0) {
                material.set_factor(factor);
            }
        }
    }
}

/// Shared assets for procedural impact effects. Holds a single unit-quad mesh
/// reused by every billboard layer (camera-facing is done in the vertex shader)
/// and the one-shot hanabi dust/spark burst, both built once.
#[derive(Resource)]
pub struct ImpactAssets {
    pub quad: Handle<Mesh>,
    pub burst: Handle<EffectAsset>,
}

impl FromWorld for ImpactAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Rectangle::from_size(Vec2::ONE)));
        let burst = world
            .resource_mut::<Assets<EffectAsset>>()
            .add(bash_burst_effect());
        Self { quad, burst }
    }
}

/// One-shot radial dust/spark burst: ~10 particles fired outward from the impact
/// point, dragged to a quick stop, fading warm-white to orange as they shrink.
fn bash_burst_effect() -> EffectAsset {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.5).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(6.0).uniform(writer.lit(12.0)).expr(),
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.25).uniform(writer.lit(0.4)).expr(),
    );
    let update_drag = LinearDragModifier::new(writer.lit(5.0).expr());

    let mut color = Gradient::new();
    color.add_key(0.0, Vec4::new(3.0, 2.0, 0.6, 1.0));
    color.add_key(1.0, Vec4::new(2.0, 0.6, 0.1, 0.0));

    let mut size = Gradient::new();
    size.add_key(0.0, Vec3::splat(1.0));
    size.add_key(1.0, Vec3::splat(0.0));

    EffectAsset::new(16, SpawnerSettings::once(10.0.into()), writer.finish())
        .with_name("bash_burst")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_drag)
        .render(ColorOverLifetimeModifier::new(color))
        .render(SizeOverLifetimeModifier {
            gradient: size,
            screen_space_size: false,
        })
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

/// Peak intensity of the impact point-light pop, in lumens.
const LIGHT_PEAK: f32 = 130_000.0;

/// Short intensity ramp for the impact `PointLight`. Fades the light from its
/// `peak` to dark over the timer; the whole tree despawns with the `FactorRamp`.
#[derive(Component)]
pub struct LightFade {
    timer: Timer,
    peak: f32,
}

impl LightFade {
    fn new(seconds: f32, peak: f32) -> Self {
        Self {
            timer: Timer::from_seconds(seconds, TimerMode::Once),
            peak,
        }
    }
}

/// Drive each `LightFade`: ramp its `PointLight.intensity` down from `peak` to 0.
pub fn fade_light(time: Res<Time>, mut lights: Query<(&mut LightFade, &mut PointLight)>) {
    for (mut fade, mut light) in &mut lights {
        fade.timer.tick(time.delta());
        light.intensity = fade.peak * (1.0 - fade.timer.fraction());
    }
}

/// Spawn the Bash impact burst: a `FactorRamp` parent at `position` carrying an
/// impact-core flash, two non-uniformly-scaled star glints (the in-shader
/// billboard ignores rotation, so the stars are differentiated by scale), a
/// fading point light, and the hanabi dust/spark burst. The tree self-despawns
/// when the ramp finishes (design §6).
pub fn spawn_bash_burst(
    commands: &mut Commands,
    core_materials: &mut Assets<ImpactCoreMaterial>,
    star_materials: &mut Assets<StarMaterial>,
    assets: &ImpactAssets,
    position: Vec3,
    color: Color,
) {
    let c = color.to_linear();
    let primary = Vec4::new(c.red, c.green, c.blue, 1.0);
    let warm = Vec4::new(1.0, 0.5, 0.15, 1.0);

    let core = core_materials.add(ImpactCoreMaterial {
        params: ImpactParams {
            primary_color: primary,
            secondary_color: warm,
            shape: Vec4::new(2.0, 8.0, 0.6, 0.5),
            factor: 0.0,
        },
    });
    let star = |mats: &mut Assets<StarMaterial>| {
        mats.add(StarMaterial {
            params: StarParams {
                primary_color: primary,
                secondary_color: warm,
                shape: Vec4::new(3.0, 6.0, 0.7, 0.0),
                factor: 0.0,
            },
        })
    };
    let star_wide = star(star_materials);
    let star_tall = star(star_materials);

    commands
        .spawn((
            FactorRamp::new(0.35),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(core),
                Transform::from_scale(Vec3::splat(6.0)),
            ));
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(star_wide),
                Transform::from_scale(Vec3::new(7.0, 2.5, 1.0)),
            ));
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(star_tall),
                Transform::from_scale(Vec3::new(2.5, 7.0, 1.0)),
            ));
            parent.spawn((
                PointLight {
                    color,
                    intensity: LIGHT_PEAK,
                    range: 40.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                LightFade::new(0.15, LIGHT_PEAK),
            ));
            parent.spawn(ParticleEffect::new(assets.burst.clone()));
        });
}

/// Read `PlayProceduralVfx` and dispatch to the matching burst spawner. Unknown
/// keys are logged and ignored (design §D6, non-critical).
pub fn on_play_procedural_vfx(
    mut reader: MessageReader<PlayProceduralVfx>,
    mut commands: Commands,
    mut core_materials: ResMut<Assets<ImpactCoreMaterial>>,
    mut star_materials: ResMut<Assets<StarMaterial>>,
    assets: Res<ImpactAssets>,
) {
    for msg in reader.read() {
        match msg.key.as_str() {
            "bash" => spawn_bash_burst(
                &mut commands,
                &mut core_materials,
                &mut star_materials,
                &assets,
                msg.position,
                msg.color,
            ),
            other => debug!("unknown procedural vfx key {other}"),
        }
    }
}

/// Registers the impact `MaterialPlugin`s, the shared assets, and the driver +
/// dispatch systems. `HanabiPlugin` is owned by the parent `VfxPlugin`, not here.
pub struct ImpactVfxPlugin;

impl Plugin for ImpactVfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ImpactCoreMaterial>::default())
            .add_plugins(MaterialPlugin::<StarMaterial>::default())
            .init_resource::<ImpactAssets>()
            .add_systems(
                Update,
                (
                    advance_ramps,
                    drive_factor::<ImpactCoreMaterial>,
                    drive_factor::<StarMaterial>,
                    fade_light,
                )
                    .chain()
                    .in_set(VfxSystems),
            )
            .add_systems(Update, on_play_procedural_vfx.in_set(VfxSystems));
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
            .add_systems(
                Update,
                (advance_ramps, drive_factor::<StubMaterial>).chain(),
            );

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
        assert!(
            (factor - 1.0).abs() < 1e-4,
            "factor reached 1.0, got {factor}"
        );
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

    fn dispatch_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<EffectAsset>()
            .init_asset::<ImpactCoreMaterial>()
            .init_asset::<StarMaterial>()
            .init_resource::<ImpactAssets>()
            .add_message::<PlayProceduralVfx>()
            .add_systems(Update, on_play_procedural_vfx);
        app
    }

    fn ramp_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&FactorRamp>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn bash_key_spawns_one_ramp() {
        let mut app = dispatch_app();
        app.world_mut().write_message(PlayProceduralVfx {
            key: "bash".into(),
            position: Vec3::new(1.0, 2.0, 3.0),
            color: Color::WHITE,
        });
        app.update();
        assert_eq!(ramp_count(&mut app), 1, "bash spawns exactly one ramp tree");
    }

    #[test]
    fn unknown_key_spawns_nothing() {
        let mut app = dispatch_app();
        app.world_mut().write_message(PlayProceduralVfx {
            key: "not-a-real-effect".into(),
            position: Vec3::ZERO,
            color: Color::WHITE,
        });
        app.update();
        assert_eq!(ramp_count(&mut app), 0, "unknown key spawns no ramp");
    }
}
