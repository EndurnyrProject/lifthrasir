use super::skill_fx::{spawn_shader_fx, SkillFxMaterial};
use super::VfxSystems;
use crate::domain::effects::PlayProceduralVfx;
use crate::infrastructure::effect::ShaderFxCatalog;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy_hanabi::{
    Attribute, ColorBlendMode, ColorOverLifetimeModifier, EffectAsset, EffectProperties,
    ExprWriter, Gradient, LinearDragModifier, ParticleEffect, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SizeOverLifetimeModifier,
    SpawnerSettings,
};
use std::collections::HashMap;

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
    /// One built `EffectAsset` per procedural burst preset key (mage/wizard skill
    /// placeholders), paired with the entity time-to-live that outlives its
    /// particles. Looked up by `PlayProceduralVfx.key`.
    pub bursts: HashMap<&'static str, (Handle<EffectAsset>, f32)>,
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
        let burst = effects.add(bash_burst_effect());
        let bursts = BURST_PRESETS
            .iter()
            .map(|(key, preset)| {
                let ttl = preset.lifetime.1 + 0.15;
                (*key, (effects.add(burst_effect(key, *preset)), ttl))
            })
            .collect();
        let spark = effects.add(spark_effect());
        Self {
            quad,
            burst,
            bursts,
            spark,
        }
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
pub fn spark_garnish_bundle(assets: &ImpactAssets, tint: Vec4) -> impl Bundle {
    (
        ParticleEffect::new(assets.spark.clone()),
        EffectProperties::default()
            .with_properties([(SPARK_TINT_PROPERTY.to_string(), tint.into())]),
    )
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

/// Whether a burst throws its particles out in every direction (spell impacts)
/// or biases them along world-up (`-Y`) for a ground eruption/spike.
#[derive(Clone, Copy)]
enum BurstVel {
    Radial,
    Upward,
}

/// One procedural burst placeholder, parameterized so the whole mage/wizard
/// roster is a preset table over a single emitter instead of bespoke effects.
/// `color` is an HDR linear tint (values may exceed 1.0 for glow); the gradient
/// fades from it to a dimmer, transparent tail.
#[derive(Clone, Copy)]
struct BurstPreset {
    color: Vec3,
    count: f32,
    size: f32,
    speed: (f32, f32),
    lifetime: (f32, f32),
    radius: f32,
    vel: BurstVel,
}

/// Build a one-shot hanabi burst from a preset. Mirrors `bash_burst_effect`'s
/// shape (sphere spawn, drag to a stop, color+size fade) with the preset's tint,
/// count, and velocity mode. Upward mode packs a `-Y`-biased velocity with a
/// little horizontal jitter; radial mode reuses the sphere-velocity modifier.
fn burst_effect(name: &str, p: BurstPreset) -> EffectAsset {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(p.radius).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer
            .lit(p.lifetime.0)
            .uniform(writer.lit(p.lifetime.1))
            .expr(),
    );
    let update_drag = LinearDragModifier::new(writer.lit(5.0).expr());

    let vel_center = writer.lit(Vec3::ZERO).expr();
    let radial_speed = writer.lit(p.speed.0).uniform(writer.lit(p.speed.1)).expr();
    let spread = p.speed.1 * 0.35;
    let up_vel = writer
        .lit(-spread)
        .uniform(writer.lit(spread))
        .vec3(
            // Negative Y is up in this world; bias the vertical component up.
            writer.lit(-p.speed.1).uniform(writer.lit(-p.speed.0)),
            writer.lit(-spread).uniform(writer.lit(spread)),
        )
        .expr();

    let mut color = Gradient::new();
    color.add_key(0.0, p.color.extend(1.0));
    color.add_key(1.0, (p.color * 0.5).extend(0.0));

    let mut size = Gradient::new();
    size.add_key(0.0, Vec3::splat(p.size));
    size.add_key(1.0, Vec3::ZERO);

    let capacity = (p.count as u32).max(1) * 2;
    let effect = EffectAsset::new(
        capacity,
        SpawnerSettings::once(p.count.into()),
        writer.finish(),
    )
    .with_name(name.to_string())
    .init(init_pos)
    .init(init_age)
    .init(init_lifetime)
    .update(update_drag)
    .render(ColorOverLifetimeModifier::new(color))
    .render(SizeOverLifetimeModifier {
        gradient: size,
        screen_space_size: false,
    });

    match p.vel {
        BurstVel::Radial => effect.init(SetVelocitySphereModifier {
            center: vel_center,
            speed: radial_speed,
        }),
        BurstVel::Upward => effect.init(SetAttributeModifier::new(Attribute::VELOCITY, up_vel)),
    }
}

/// Element-appropriate placeholder tuning for every mage/wizard `vfx` key that
/// dispatches to a procedural burst (catalog Tasks 9/10). Deliberately readable
/// stand-ins, not authentic effects: warm reds/oranges for fire, ice-blue for
/// water/ice, purple for psychic, yellow-white for lightning, brown+upward for
/// earth. `ice_wall` is absent — it is a persistent per-cell wall handled in
/// the skill-units spawn path, not a one-shot burst.
const BURST_PRESETS: &[(&str, BurstPreset)] = &[
    // Mage
    (
        "sight",
        BurstPreset {
            color: Vec3::new(3.0, 1.4, 0.3),
            count: 8.0,
            size: 1.2,
            speed: (4.0, 8.0),
            lifetime: (0.2, 0.35),
            radius: 0.4,
            vel: BurstVel::Radial,
        },
    ),
    (
        "napalm_beat",
        BurstPreset {
            color: Vec3::new(1.6, 0.3, 2.6),
            count: 12.0,
            size: 1.4,
            speed: (5.0, 10.0),
            lifetime: (0.3, 0.45),
            radius: 0.5,
            vel: BurstVel::Radial,
        },
    ),
    (
        "soul_strike",
        BurstPreset {
            color: Vec3::new(1.6, 2.0, 3.0),
            count: 6.0,
            size: 1.8,
            speed: (3.0, 6.0),
            lifetime: (0.4, 0.6),
            radius: 0.5,
            vel: BurstVel::Radial,
        },
    ),
    (
        "cold_bolt",
        BurstPreset {
            color: Vec3::new(0.6, 1.6, 3.0),
            count: 12.0,
            size: 1.0,
            speed: (6.0, 12.0),
            lifetime: (0.25, 0.4),
            radius: 0.4,
            vel: BurstVel::Radial,
        },
    ),
    (
        "frost_diver",
        BurstPreset {
            color: Vec3::new(0.7, 1.7, 3.0),
            count: 14.0,
            size: 1.1,
            speed: (5.0, 10.0),
            lifetime: (0.3, 0.45),
            radius: 0.6,
            vel: BurstVel::Radial,
        },
    ),
    (
        "fireball",
        BurstPreset {
            color: Vec3::new(3.0, 1.1, 0.2),
            count: 20.0,
            size: 2.0,
            speed: (7.0, 14.0),
            lifetime: (0.3, 0.5),
            radius: 0.6,
            vel: BurstVel::Radial,
        },
    ),
    (
        "fire_bolt",
        BurstPreset {
            color: Vec3::new(3.0, 0.8, 0.15),
            count: 16.0,
            size: 0.9,
            speed: (6.0, 13.0),
            lifetime: (0.25, 0.4),
            radius: 0.4,
            vel: BurstVel::Radial,
        },
    ),
    (
        "lightning_bolt",
        BurstPreset {
            color: Vec3::new(3.0, 3.0, 1.6),
            count: 14.0,
            size: 0.9,
            speed: (8.0, 16.0),
            lifetime: (0.2, 0.35),
            radius: 0.4,
            vel: BurstVel::Radial,
        },
    ),
    // Wizard
    (
        "sightrasher",
        BurstPreset {
            color: Vec3::new(3.0, 1.0, 0.2),
            count: 24.0,
            size: 1.6,
            speed: (8.0, 15.0),
            lifetime: (0.3, 0.5),
            radius: 0.8,
            vel: BurstVel::Radial,
        },
    ),
    (
        "meteor",
        BurstPreset {
            color: Vec3::new(3.0, 0.6, 0.1),
            count: 28.0,
            size: 2.4,
            speed: (8.0, 16.0),
            lifetime: (0.35, 0.6),
            radius: 0.9,
            vel: BurstVel::Radial,
        },
    ),
    (
        "jupitel_thunder",
        BurstPreset {
            color: Vec3::new(1.8, 2.4, 4.5),
            count: 32.0,
            size: 1.1,
            speed: (11.0, 22.0),
            lifetime: (0.25, 0.5),
            radius: 0.5,
            vel: BurstVel::Radial,
        },
    ),
    (
        "water_ball",
        BurstPreset {
            color: Vec3::new(0.3, 0.9, 3.0),
            count: 18.0,
            size: 1.4,
            speed: (6.0, 12.0),
            lifetime: (0.3, 0.5),
            radius: 0.6,
            vel: BurstVel::Radial,
        },
    ),
    (
        "frost_nova",
        BurstPreset {
            color: Vec3::new(0.7, 1.7, 3.0),
            count: 26.0,
            size: 1.4,
            speed: (9.0, 16.0),
            lifetime: (0.3, 0.5),
            radius: 0.8,
            vel: BurstVel::Radial,
        },
    ),
    (
        "earth_spike",
        BurstPreset {
            color: Vec3::new(1.2, 0.7, 0.3),
            count: 12.0,
            size: 1.2,
            speed: (8.0, 14.0),
            lifetime: (0.3, 0.5),
            radius: 0.3,
            vel: BurstVel::Upward,
        },
    ),
    (
        "heavens_drive",
        BurstPreset {
            color: Vec3::new(1.1, 0.65, 0.3),
            count: 22.0,
            size: 1.4,
            speed: (9.0, 15.0),
            lifetime: (0.35, 0.55),
            radius: 0.7,
            vel: BurstVel::Upward,
        },
    ),
    (
        "sight_blaster",
        BurstPreset {
            color: Vec3::new(3.0, 1.2, 0.3),
            count: 16.0,
            size: 1.5,
            speed: (7.0, 13.0),
            lifetime: (0.3, 0.45),
            radius: 0.5,
            vel: BurstVel::Radial,
        },
    ),
];

/// Spawn a procedural burst: a `FactorRamp` parent at `position` (whose `ttl`
/// outlives the particles, then despawns the tree via `advance_ramps`) carrying
/// the preset's one-shot hanabi emitter. No materials/light — that heavier tree
/// is Bash-specific; these are lightweight element placeholders.
fn spawn_procedural_burst(
    commands: &mut Commands,
    effect: Handle<EffectAsset>,
    ttl: f32,
    position: Vec3,
) {
    commands
        .spawn((
            FactorRamp::new(ttl),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn(ParticleEffect::new(effect));
        });
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

/// Palette and shape tuning for one composite flash tree (`spawn_flash_burst`).
/// Star glint scales derive from `scale` with Bash's original 7:2.5 proportions.
struct FlashStyle {
    primary: Vec4,
    secondary: Vec4,
    /// Streak count of the radial core; higher reads as jagged/electric.
    streaks: f32,
    scale: f32,
    ramp: f32,
    light_color: Color,
    light_peak: f32,
}

/// Bash's original tuning: caster-tinted core with a warm orange fringe.
fn bash_style(color: Color) -> FlashStyle {
    let c = color.to_linear();
    FlashStyle {
        primary: Vec4::new(c.red, c.green, c.blue, 1.0),
        secondary: Vec4::new(1.0, 0.5, 0.15, 1.0),
        streaks: 8.0,
        scale: 6.0,
        ramp: 0.35,
        light_color: color,
        light_peak: LIGHT_PEAK,
    }
}

/// Spawn a composite impact flash: a `FactorRamp` parent at `position` carrying
/// an impact-core flash, two non-uniformly-scaled star glints (the in-shader
/// billboard ignores rotation, so the stars are differentiated by scale), a
/// fading point light, and a hanabi spark burst. The tree self-despawns when
/// the ramp finishes (design §6).
fn spawn_flash_burst(
    commands: &mut Commands,
    core_materials: &mut Assets<ImpactCoreMaterial>,
    star_materials: &mut Assets<StarMaterial>,
    assets: &ImpactAssets,
    particles: Handle<EffectAsset>,
    position: Vec3,
    style: FlashStyle,
) {
    let core = core_materials.add(ImpactCoreMaterial {
        params: ImpactParams {
            primary_color: style.primary,
            secondary_color: style.secondary,
            shape: Vec4::new(2.0, style.streaks, 0.6, 0.5),
            factor: 0.0,
        },
    });
    let star = |mats: &mut Assets<StarMaterial>| {
        mats.add(StarMaterial {
            params: StarParams {
                primary_color: style.primary,
                secondary_color: style.secondary,
                shape: Vec4::new(3.0, 6.0, 0.7, 0.0),
                factor: 0.0,
            },
        })
    };
    let star_wide = star(star_materials);
    let star_tall = star(star_materials);
    let long = style.scale * 7.0 / 6.0;
    let short = style.scale * 2.5 / 6.0;

    commands
        .spawn((
            FactorRamp::new(style.ramp),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(core),
                Transform::from_scale(Vec3::splat(style.scale)),
            ));
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(star_wide),
                Transform::from_scale(Vec3::new(long, short, 1.0)),
            ));
            parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(star_tall),
                Transform::from_scale(Vec3::new(short, long, 1.0)),
            ));
            parent.spawn((
                PointLight {
                    color: style.light_color,
                    intensity: style.light_peak,
                    range: 40.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                LightFade::new(0.15, style.light_peak),
            ));
            parent.spawn(ParticleEffect::new(particles));
        });
}

/// Read `PlayProceduralVfx` and dispatch to the matching burst spawner. Unknown
/// keys are logged and ignored (design §D6, non-critical).
pub fn on_play_procedural_vfx(
    mut reader: MessageReader<PlayProceduralVfx>,
    mut commands: Commands,
    mut core_materials: ResMut<Assets<ImpactCoreMaterial>>,
    mut star_materials: ResMut<Assets<StarMaterial>>,
    mut skill_fx_materials: ResMut<Assets<SkillFxMaterial>>,
    shader_fx: Option<Res<ShaderFxCatalog>>,
    assets: Res<ImpactAssets>,
) {
    for msg in reader.read() {
        if let Some(entry) = shader_fx.as_ref().and_then(|catalog| catalog.get(&msg.key)) {
            spawn_shader_fx(
                &mut commands,
                &mut skill_fx_materials,
                &assets,
                entry,
                msg.position,
            );
            continue;
        }

        match msg.key.as_str() {
            "bash" => spawn_flash_burst(
                &mut commands,
                &mut core_materials,
                &mut star_materials,
                &assets,
                assets.burst.clone(),
                msg.position,
                bash_style(msg.color),
            ),
            key => {
                let Some((effect, ttl)) = assets.bursts.get(key) else {
                    debug!("unknown procedural vfx key {key}");
                    continue;
                };
                spawn_procedural_burst(&mut commands, effect.clone(), *ttl, msg.position);
            }
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
            .init_asset::<ImpactCoreMaterial>()
            .init_asset::<StarMaterial>()
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
    fn every_burst_preset_key_spawns_one_ramp() {
        for (key, _) in BURST_PRESETS {
            let mut app = dispatch_app();
            app.world_mut().write_message(PlayProceduralVfx {
                key: (*key).into(),
                position: Vec3::new(1.0, 2.0, 3.0),
                color: Color::WHITE,
            });
            app.update();
            assert_eq!(
                ramp_count(&mut app),
                1,
                "preset key {key} spawns exactly one burst tree (not the unknown-key path)"
            );
        }
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

    #[test]
    fn catalog_key_routes_to_shader_path_not_bursts() {
        use crate::infrastructure::effect::ShaderFxEntry;
        use std::collections::BTreeMap;

        let mut app = dispatch_app();
        // Key collides with a burst preset to prove the catalog lookup wins.
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
            },
        );
        app.insert_resource(ShaderFxCatalog::from_entries(entries));

        app.world_mut().write_message(PlayProceduralVfx {
            key: "fire_bolt".into(),
            position: Vec3::ZERO,
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
