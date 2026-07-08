use super::VfxSystems;
use crate::domain::effects::MapAmbientVfx;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_hanabi::{
    AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, EffectMaterial, ExprWriter,
    Gradient, ImageSampleMapping, ParticleEffect, ParticleTextureModifier, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SizeOverLifetimeModifier,
    SpawnerSettings,
};

/// Floor for a degenerate (zero) RSW `emit_speed`/`params` magnitude, so a
/// `EF_EMITTER` object with no authored values still spawns a visible effect
/// instead of a dead/invisible one (design: Error Handling & Edge Cases).
const MIN_MAGNITUDE: f32 = 1.0;

/// Floor a possibly-zero (or negative) magnitude to a visible minimum.
fn floor_magnitude(value: f32, floor: f32) -> f32 {
    let magnitude = value.abs();
    if magnitude < f32::EPSILON {
        floor
    } else {
        magnitude
    }
}

/// Shared assets for map ambient particles. The smoke `EffectAsset` and the
/// soft particle texture are built exactly once here; every `"smoke"`
/// `MapAmbientVfx` clones these handles. The reference map carries 152 smoke
/// objects — a per-object asset would be 152 separate GPU pipeline compiles.
#[derive(Resource)]
pub struct MapAmbientAssets {
    smoke: Handle<EffectAsset>,
    /// Soft round alpha mask bound to the smoke effect's texture slot so
    /// particles read as feathered gas puffs instead of hard opaque quads.
    particle: Handle<Image>,
}

impl FromWorld for MapAmbientAssets {
    fn from_world(world: &mut World) -> Self {
        let particle = world
            .resource_mut::<Assets<Image>>()
            .add(soft_particle_image());
        let smoke = world
            .resource_mut::<Assets<EffectAsset>>()
            .add(smoke_effect());
        Self { smoke, particle }
    }
}

/// A radial alpha falloff (opaque center → transparent edge), used as the
/// particle sprite. `ImageSampleMapping::ModulateOpacityFromR` reads the red
/// channel into the particle's opacity, so the smoke's quads are masked into
/// soft round puffs. Generated in-code to avoid shipping/loading a PNG.
fn soft_particle_image() -> Image {
    const SIZE: u32 = 64;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let dist = (dx * dx + dy * dy).sqrt();
            // Squared falloff for a soft, gassy edge rather than a hard disc.
            let falloff = (1.0 - dist).clamp(0.0, 1.0);
            let v = (falloff * falloff * 255.0) as u8;
            data.extend_from_slice(&[v, v, v, v]);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    )
}

/// A persistent, position-anchored smoke column: particles drift upward
/// (world up is `-Y`, per project convention) while fading in, then out, and
/// growing. Baked grey tint — `MapAmbientVfx` does not currently carry the
/// catalog color.
fn smoke_effect() -> EffectAsset {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.6).expr(),
        dimension: ShapeDimension::Volume,
    };
    // Velocity is `normalize(position - center) * speed`. Offsetting the center
    // below the spawn sphere (+Y is down; up is -Y) biases emission upward and
    // keeps `position - center` nonzero, guarding against a `normalize(0)` = NaN
    // if a particle ever spawns exactly at the center.
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 2.0, 0.0)).expr(),
        speed: writer.lit(0.3).uniform(writer.lit(0.9)).expr(),
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(4.0).uniform(writer.lit(7.0)).expr(),
    );
    let update_rise = AccelModifier::new(writer.lit(Vec3::new(0.0, -0.7, 0.0)).expr());

    // Texture slot #0 for the soft particle mask; bound per-entity via
    // `EffectMaterial` in the attach system.
    let texture_slot = writer.lit(0u32).expr();

    // Low per-puff opacity so overlapping soft particles accumulate into a
    // volumetric haze rather than reading as individual cards.
    let mut color = Gradient::new();
    color.add_key(0.0, Vec4::new(0.5, 0.5, 0.5, 0.0));
    color.add_key(0.15, Vec4::new(0.55, 0.55, 0.55, 0.35));
    color.add_key(0.5, Vec4::new(0.5, 0.5, 0.5, 0.25));
    color.add_key(1.0, Vec4::new(0.45, 0.45, 0.45, 0.0));

    let mut size = Gradient::new();
    size.add_key(0.0, Vec3::splat(0.8));
    size.add_key(0.4, Vec3::splat(2.2));
    size.add_key(1.0, Vec3::splat(3.6));

    let mut module = writer.finish();
    module.add_texture_slot("particle");

    EffectAsset::new(256, SpawnerSettings::rate(14.0.into()), module)
        .with_name("map_ambient_smoke")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_rise)
        .render(ParticleTextureModifier {
            texture_slot,
            sample_mapping: ImageSampleMapping::ModulateOpacityFromR,
        })
        .render(ColorOverLifetimeModifier::new(color))
        .render(SizeOverLifetimeModifier {
            gradient: size,
            screen_space_size: false,
        })
}

/// A generic upward/outward emitter, one `EffectAsset` per distinct
/// `emit_speed`/`params` set (acceptable at the reference map's 3 objects —
/// see design "Emitter (974) fidelity"). `emit_speed` drives the spawn rate,
/// `params[0]` drives outward speed; both are floored so an object with
/// all-zero fields still emits visibly. Exact param semantics are unverified
/// (open question); tune against the `debug!`-logged real values.
fn emitter_effect(emit_speed: f32, params: [f32; 4]) -> EffectAsset {
    let spawn_rate = floor_magnitude(emit_speed, MIN_MAGNITUDE);
    let speed = floor_magnitude(params[0], MIN_MAGNITUDE);

    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.3).expr(),
        dimension: ShapeDimension::Volume,
    };
    // Offset center below the spawn sphere: biases the emitter upward and
    // guards against `normalize(0)` = NaN (see smoke_effect).
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 2.0, 0.0)).expr(),
        speed: writer.lit(speed * 0.5).uniform(writer.lit(speed)).expr(),
    };
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.8).uniform(writer.lit(1.5)).expr(),
    );
    let update_rise = AccelModifier::new(writer.lit(Vec3::new(0.0, -speed * 0.5, 0.0)).expr());

    let texture_slot = writer.lit(0u32).expr();

    let mut color = Gradient::new();
    color.add_key(0.0, Vec4::new(1.0, 1.0, 0.85, 0.0));
    color.add_key(0.2, Vec4::new(1.0, 1.0, 0.85, 0.5));
    color.add_key(1.0, Vec4::new(1.0, 1.0, 0.85, 0.0));

    // Grow rather than shrink: a shrinking-to-0.1 particle reads as a tiny
    // stuck speck at this emitter's fixed map position.
    let mut size = Gradient::new();
    size.add_key(0.0, Vec3::splat(0.6));
    size.add_key(1.0, Vec3::splat(1.4));

    let mut module = writer.finish();
    module.add_texture_slot("particle");

    EffectAsset::new(64, SpawnerSettings::rate(spawn_rate.into()), module)
        .with_name("map_ambient_emitter")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_rise)
        .render(ParticleTextureModifier {
            texture_slot,
            sample_mapping: ImageSampleMapping::ModulateOpacityFromR,
        })
        .render(ColorOverLifetimeModifier::new(color))
        .render(SizeOverLifetimeModifier {
            gradient: size,
            screen_space_size: false,
        })
}

/// On `MapAmbientVfx` spawn, attach the matching persistent hanabi
/// `ParticleEffect`. `"smoke"` clones the one shared handle; `"emitter"`
/// builds its own asset from the object's `emit_speed`/`params` (design
/// Part B — Presentation). Unknown keys are `debug!`-logged and skipped
/// (effects are non-critical, design D6).
pub fn attach_map_ambient_vfx(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    assets: Res<MapAmbientAssets>,
    query: Query<(Entity, &MapAmbientVfx), Added<MapAmbientVfx>>,
) {
    for (entity, vfx) in &query {
        match vfx.key.as_str() {
            "smoke" => {
                commands.entity(entity).insert((
                    ParticleEffect::new(assets.smoke.clone()),
                    EffectMaterial {
                        images: vec![assets.particle.clone()],
                    },
                ));
            }
            "emitter" => {
                debug!(
                    "EF_EMITTER attach: emit_speed={} params={:?}",
                    vfx.emit_speed, vfx.params
                );
                let handle = effects.add(emitter_effect(vfx.emit_speed, vfx.params));
                commands.entity(entity).insert((
                    ParticleEffect::new(handle),
                    EffectMaterial {
                        images: vec![assets.particle.clone()],
                    },
                ));
            }
            other => debug!("unknown map ambient vfx key {other}"),
        }
    }
}

/// Registers the shared smoke asset and the attach system. `HanabiPlugin` is
/// owned by the parent `VfxPlugin`, not here.
pub struct MapAmbientVfxPlugin;

impl Plugin for MapAmbientVfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapAmbientAssets>()
            .add_systems(Update, attach_map_ambient_vfx.in_set(VfxSystems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_magnitude_replaces_zero_with_floor() {
        assert_eq!(floor_magnitude(0.0, 2.0), 2.0);
    }

    #[test]
    fn floor_magnitude_keeps_nonzero_value() {
        assert_eq!(floor_magnitude(5.0, 2.0), 5.0);
    }

    #[test]
    fn floor_magnitude_takes_absolute_value_of_negatives() {
        assert_eq!(floor_magnitude(-3.0, 2.0), 3.0);
    }

    fn dispatch_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<EffectAsset>()
            .init_asset::<Image>()
            .init_resource::<MapAmbientAssets>()
            .add_systems(Update, attach_map_ambient_vfx);
        app
    }

    fn particle_effect_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&ParticleEffect>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn smoke_key_attaches_particle_effect() {
        let mut app = dispatch_app();
        app.world_mut().spawn(MapAmbientVfx {
            key: "smoke".into(),
            emit_speed: 0.0,
            params: [0.0; 4],
        });
        app.update();
        assert_eq!(particle_effect_count(&mut app), 1);
    }

    #[test]
    fn emitter_key_with_zero_params_still_attaches_particle_effect() {
        let mut app = dispatch_app();
        app.world_mut().spawn(MapAmbientVfx {
            key: "emitter".into(),
            emit_speed: 0.0,
            params: [0.0; 4],
        });
        app.update();
        assert_eq!(
            particle_effect_count(&mut app),
            1,
            "the degenerate-param floor still yields an attached effect"
        );
    }

    #[test]
    fn unknown_key_attaches_nothing() {
        let mut app = dispatch_app();
        app.world_mut().spawn(MapAmbientVfx {
            key: "not-a-real-effect".into(),
            emit_speed: 1.0,
            params: [1.0; 4],
        });
        app.update();
        assert_eq!(particle_effect_count(&mut app), 0);
    }
}
