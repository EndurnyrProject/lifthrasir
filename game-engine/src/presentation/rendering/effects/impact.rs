use super::VfxSystems;
use super::skill_fx::{SkillFxMaterial, spawn_shader_fx};
use crate::domain::effects::PlayProceduralVfx;
use crate::infrastructure::effect::ShaderFxCatalog;
use bevy::prelude::*;
use bevy_hanabi::{
    Attribute, ColorBlendMode, ColorOverLifetimeModifier, EffectAsset, EffectProperties,
    ExprWriter, Gradient, LinearDragModifier, ParticleEffect, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SizeOverLifetimeModifier,
    SpawnerSettings,
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
/// and the one-shot hanabi spark garnish, both built once.
#[derive(Resource)]
pub struct ImpactAssets {
    pub quad: Handle<Mesh>,
    /// Single tintable one-shot spark garnish, shared by every shader-fx
    /// caller. Color is not baked in: each spawn supplies its own tint via the
    /// `spark_tint` hanabi property (see `spark_garnish_bundle`).
    pub spark: Handle<EffectAsset>,
}

impl FromWorld for ImpactAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Rectangle::from_size(Vec2::ONE)));
        let mut effects = world.resource_mut::<Assets<EffectAsset>>();
        let spark = effects.add(spark_effect());
        Self { quad, spark }
    }
}

/// Hanabi effect property name carrying `ImpactAssets::spark`'s per-instance
/// HDR tint. Declared once in `spark_effect` and set per spawn by
/// `spark_garnish_bundle` via `EffectProperties`.
const SPARK_TINT_PROPERTY: &str = "spark_tint";

/// One-shot tintable spark garnish: ~16 particles fired outward from the
/// impact point, dragged to a quick stop, shrinking to nothing. Shape (sphere
/// spawn, drag, size fade) mirrors `burst_effect`, but color is never baked
/// into the asset — each particle reads its base HDR color from the
/// `spark_tint` property at init, then a neutral white-to-transparent
/// gradient modulates (multiplies) that tint down over its lifetime.
fn spark_effect() -> EffectAsset {
    let writer = ExprWriter::new();

    let tint = writer.add_property(SPARK_TINT_PROPERTY, Vec4::ONE.into());

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.4).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(8.0).uniform(writer.lit(16.0)).expr(),
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.2).uniform(writer.lit(0.35)).expr(),
    );
    let init_color = SetAttributeModifier::new(Attribute::HDR_COLOR, writer.prop(tint).expr());
    let update_drag = LinearDragModifier::new(writer.lit(5.0).expr());

    let mut alpha = Gradient::new();
    alpha.add_key(0.0, Vec4::ONE);
    alpha.add_key(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0));

    let mut size = Gradient::new();
    size.add_key(0.0, Vec3::splat(1.0));
    size.add_key(1.0, Vec3::ZERO);

    EffectAsset::new(32, SpawnerSettings::once(16.0.into()), writer.finish())
        .with_name("spark_garnish")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .init(init_color)
        .update(update_drag)
        .render(ColorOverLifetimeModifier {
            gradient: alpha,
            blend: ColorBlendMode::Modulate,
            mask: default(),
        })
        .render(SizeOverLifetimeModifier {
            gradient: size,
            screen_space_size: false,
        })
}

/// Child bundle spawning the tintable spark garnish under a `FactorRamp` tree
/// (the child-spawn slot `spawn_shader_fx` uses for its optional garnish): one
/// `ParticleEffect` referencing the shared `spark` asset plus an
/// `EffectProperties` setting `spark_tint` to `tint` for this instance only.
/// No new `EffectAsset` is built per call.
pub fn spark_garnish_bundle(assets: &ImpactAssets, tint: Vec4) -> impl Bundle + use<> {
    (
        ParticleEffect::new(assets.spark.clone()),
        EffectProperties::default()
            .with_properties([(SPARK_TINT_PROPERTY.to_string(), tint.into())]),
    )
}

/// Peak intensity of the impact point-light pop, in lumens.
pub(super) const LIGHT_PEAK: f32 = 130_000.0;

/// Short intensity ramp for the impact `PointLight`. Fades the light from its
/// `peak` to dark over the timer; the whole tree despawns with the `FactorRamp`.
#[derive(Component)]
pub struct LightFade {
    timer: Timer,
    peak: f32,
}

impl LightFade {
    pub(super) fn new(seconds: f32, peak: f32) -> Self {
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

/// Read `PlayProceduralVfx` and dispatch to the shader-fx catalog. Unknown
/// keys are logged and ignored (design §D6, non-critical).
pub fn on_play_procedural_vfx(
    mut reader: MessageReader<PlayProceduralVfx>,
    mut commands: Commands,
    mut skill_fx_materials: ResMut<Assets<SkillFxMaterial>>,
    shader_fx: Option<Res<ShaderFxCatalog>>,
    asset_server: Res<AssetServer>,
    assets: Res<ImpactAssets>,
) {
    for msg in reader.read() {
        let Some(entry) = shader_fx.as_ref().and_then(|catalog| catalog.get(&msg.key)) else {
            debug!("unknown procedural vfx key {}", msg.key);
            continue;
        };
        spawn_shader_fx(
            &mut commands,
            &mut skill_fx_materials,
            &asset_server,
            &assets,
            entry,
            msg.position,
            msg.source,
            msg.hits,
            msg.sound.clone(),
            &msg.key,
            msg.color,
        );
    }
}

/// Registers the shared assets and the driver + dispatch systems.
/// `HanabiPlugin` is owned by the parent `VfxPlugin`, not here.
pub struct ImpactVfxPlugin;

impl Plugin for ImpactVfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImpactAssets>()
            .add_systems(
                Update,
                (advance_ramps, fade_light).chain().in_set(VfxSystems),
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
    fn spark_garnish_bundle_carries_its_own_tint() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<EffectAsset>>();
        let assets = ImpactAssets::from_world(&mut world);

        let tint = Vec4::new(0.2, 0.8, 3.0, 1.0);
        let entity = world.spawn(spark_garnish_bundle(&assets, tint)).id();

        let stored = world
            .get::<EffectProperties>(entity)
            .expect("bundle carries EffectProperties")
            .get_stored(SPARK_TINT_PROPERTY)
            .expect("spark_tint was set on spawn");
        assert_eq!(stored, tint.into(), "tint is per-instance, not baked in");
    }

    fn dispatch_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<EffectAsset>()
            .init_asset::<SkillFxMaterial>()
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
    fn unknown_key_spawns_nothing() {
        let mut app = dispatch_app();
        app.world_mut().write_message(PlayProceduralVfx {
            key: "not-a-real-effect".into(),
            position: Vec3::ZERO,
            source: None,
            hits: 1,
            sound: None,
            color: Color::WHITE,
        });
        app.update();
        assert_eq!(ramp_count(&mut app), 0, "unknown key spawns no ramp");
    }

    #[test]
    fn catalog_key_routes_to_shader_path_not_bursts() {
        use crate::infrastructure::effect::ShaderFxEntry;
        use std::collections::BTreeMap;

        let mut app = dispatch_app();
        // Key would otherwise hit the unknown-key path; the catalog lookup wins.
        let mut entries = BTreeMap::new();
        entries.insert(
            "fire_bolt".to_string(),
            ShaderFxEntry {
                kind: 1,
                primary: (1.0, 1.0, 1.0, 1.0),
                secondary: (1.0, 1.0, 1.0, 1.0),
                shape: (0.0, 0.0, 0.0, 0.0),
                duration: 0.5,
                scale: 10.0,
                light: None,
                garnish: None,
                texture: None,
                frames: None,
                travel: None,
            },
        );
        app.insert_resource(ShaderFxCatalog::from_entries(entries));

        app.world_mut().write_message(PlayProceduralVfx {
            key: "fire_bolt".into(),
            position: Vec3::ZERO,
            source: None,
            hits: 1,
            sound: None,
            color: Color::WHITE,
        });
        app.update();

        let shader_quads = app
            .world_mut()
            .query::<&MeshMaterial3d<SkillFxMaterial>>()
            .iter(app.world())
            .count();
        assert_eq!(
            shader_quads, 1,
            "a catalog key spawns the SkillFxMaterial shader quad, not a burst"
        );
    }
}
