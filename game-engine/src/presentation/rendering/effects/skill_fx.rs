use super::VfxSystems;
use super::impact::{
    FactorMaterial, FactorRamp, ImpactAssets, LIGHT_PEAK, LightFade, drive_factor,
    spark_garnish_bundle,
};
use crate::domain::audio::events::PlaySkillSfx;
use crate::domain::effects::{PlayProceduralVfx, SightOrbit};
use crate::infrastructure::effect::{ShaderFxEntry, ShaderFxTravel, TextureFrames};
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// Generic unlit additive billboard material for procedural skill effects. The
/// `kind` uniform selects the per-skill fragment function in the uber-shader, so
/// a new effect is one WGSL fragment plus one `shader_fx` entry in
/// `effects.ron`, zero Rust.
/// Animated by `FactorRamp` via `factor`. See `assets/data/effects/skill_fx.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SkillFxMaterial {
    #[uniform(0)]
    pub params: SkillFxParams,
    /// Optional classic GRF effect texture the fragment may sample (kinds that
    /// don't sample leave it `None`, which binds Bevy's fallback image). Loaded
    /// from `ShaderFxEntry::texture` in `spawn_shader_fx`.
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
}

/// Packed skill-fx parameters. Field order and types must match the
/// `SkillFxParams` struct in `skill_fx.wgsl`.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct SkillFxParams {
    pub kind: u32,
    pub primary: Vec4,
    pub secondary: Vec4,
    /// Per-kind scalars; meaning documented in each fragment function header.
    pub shape: Vec4,
    pub factor: f32,
}

impl Material for SkillFxMaterial {
    fn vertex_shader() -> ShaderRef {
        "ro://effects/skill_fx.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "ro://effects/skill_fx.wgsl".into()
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

impl FactorMaterial for SkillFxMaterial {
    fn set_factor(&mut self, factor: f32) {
        self.params.factor = factor;
    }
}

/// `SkillFxParams::kind` value selecting the projectile fragment in the
/// uber-shader (see `projectile_fragment` in `skill_fx.wgsl`). Kept far above the
/// per-skill burst kinds so new bursts can grow contiguously from 0.
const PROJECTILE_KIND: u32 = 100;

/// Flipbook animator for a `SkillFxMaterial`'s bound texture. Sits on the quad
/// entity alongside the `MeshMaterial3d<SkillFxMaterial>` and cycles the material's
/// `texture` through `frames` at `fps`, looping. Works for burst or projectile —
/// whichever binds an animated `TextureFrames`.
#[derive(Component)]
pub struct FxFlipbook {
    frames: Vec<Handle<Image>>,
    fps: f32,
    elapsed: f32,
}

/// Resolve a burst/projectile texture slot: an animated `frames` series (frame 0
/// bound now, a `FxFlipbook` to cycle the rest) wins over a single `texture`;
/// either may be absent, binding Bevy's fallback image.
fn resolve_fx_texture(
    asset_server: &AssetServer,
    single: Option<&String>,
    frames: Option<&TextureFrames>,
) -> (Option<Handle<Image>>, Option<FxFlipbook>) {
    if let Some(frames) = frames.filter(|f| !f.paths.is_empty()) {
        let handles: Vec<Handle<Image>> = frames
            .paths
            .iter()
            .map(|path| asset_server.load(format!("ro://{path}")))
            .collect();
        let first = handles[0].clone();
        return (
            Some(first),
            Some(FxFlipbook {
                frames: handles,
                fps: frames.fps,
                elapsed: 0.0,
            }),
        );
    }
    let single = single.map(|path| asset_server.load(format!("ro://{path}")));
    (single, None)
}

/// Advance every `FxFlipbook` and swap its material's bound texture to the current
/// frame. The bound-handle equality guard means the material only mutates on an
/// actual frame change, not every tick (avoids change-detection spam).
pub fn animate_flipbook(
    time: Res<Time>,
    mut books: Query<(&mut FxFlipbook, &MeshMaterial3d<SkillFxMaterial>)>,
    mut materials: ResMut<Assets<SkillFxMaterial>>,
) {
    for (mut book, handle) in &mut books {
        if book.frames.is_empty() {
            continue;
        }
        book.elapsed += time.delta_secs();
        let idx = (book.elapsed * book.fps) as usize % book.frames.len();
        let frame = book.frames[idx].clone();
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        if material.texture.as_ref() != Some(&frame) {
            material.texture = Some(frame);
        }
    }
}

/// The classic Sight flame's flicker frames (`fire-1..3.bmp`, magenta-keyed
/// small flame sprites — the loader zeroes keyed pixels, so they are
/// additive-safe), cycled by the shared `FxFlipbook`.
const SIGHT_FLAME_FRAMES: [&str; 3] = [
    "data/texture/effect/fire-1.bmp",
    "data/texture/effect/fire-2.bmp",
    "data/texture/effect/fire-3.bmp",
];
const SIGHT_FLAME_FPS: f32 = 10.0;
/// Uniform world-space scale of the orbiting flame billboard. The flame art
/// fills only ~1/6 of its 128px frame (the rest is keyed away), so the quad
/// must be several times the intended flame size: 14 units of quad ≈ a
/// 2-unit visible flame next to a ~5-unit-tall character.
const SIGHT_FLAME_SCALE: f32 = 14.0;

/// Dress every freshly spawned [`SightOrbit`] anchor (domain spawns it bare —
/// see `status_visuals.rs`) with the classic orbiting fireball: a flame-frame
/// billboard on the projectile fragment (kind 100 — steady look, no
/// `FactorRamp`) plus a warm point light on the anchor so the flame lights its
/// surroundings. The anchor's despawn (sight bit cleared) takes the child
/// quad with it.
pub fn dress_sight_orbits(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    assets: Res<ImpactAssets>,
    mut materials: ResMut<Assets<SkillFxMaterial>>,
    orbits: Query<Entity, Added<SightOrbit>>,
) {
    for orbit in &orbits {
        let frames = TextureFrames {
            paths: SIGHT_FLAME_FRAMES.map(String::from).to_vec(),
            fps: SIGHT_FLAME_FPS,
        };
        let (texture, flipbook) = resolve_fx_texture(&asset_server, None, Some(&frames));

        let material = materials.add(SkillFxMaterial {
            params: SkillFxParams {
                kind: PROJECTILE_KIND,
                primary: Vec4::new(3.2, 1.9, 0.8, 1.0),
                secondary: Vec4::new(2.4, 0.9, 0.25, 1.0),
                shape: Vec4::ZERO,
                factor: 0.0,
            },
            texture,
        });

        commands.entity(orbit).insert(PointLight {
            color: Color::srgb(1.0, 0.6, 0.3),
            intensity: LIGHT_PEAK * 0.5,
            range: 30.0,
            shadow_maps_enabled: false,
            ..default()
        });

        let mut quad = commands.spawn((
            Mesh3d(assets.quad.clone()),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(SIGHT_FLAME_SCALE)),
            NotShadowCaster,
            ChildOf(orbit),
        ));
        if let Some(flipbook) = flipbook {
            quad.insert(flipbook);
        }
    }
}

/// A projectile billboard flying from the caster to `target`. Moved by
/// `advance_traveling_vfx`, which despawns it on arrival and replays the effect's
/// burst at the target via a fresh `PlayProceduralVfx` (with no `source`).
#[derive(Component)]
pub struct TravelingVfx {
    /// Catalog key to replay as the burst on arrival.
    key: String,
    target: Vec3,
    /// World units per second.
    speed: f32,
    /// Seconds this bolt waits at the caster before launching (its stagger slot).
    /// It stays hidden until the delay elapses, so a level-N bolt reads as N orbs
    /// leaving in sequence rather than a burst of overlapping ones.
    delay: f32,
    /// Whoosh played once, from this orb, the frame it launches. Each bolt gets
    /// its own so a level-N bolt fires N staggered sounds off one server packet.
    sound: Option<String>,
    /// Set once the launch (reveal + sound) has happened, so it fires exactly once.
    launched: bool,
    color: Color,
}

/// Spawn a data-driven shader effect from a `ShaderFxCatalog` entry. When the
/// entry declares `travel` and a caster `source` is known, this launches the
/// skill's own projectile from `source` toward `position` (the burst plays later,
/// on arrival, via `advance_traveling_vfx`). Otherwise it spawns the burst
/// directly at `position`: a `FactorRamp` parent carrying a billboard quad with a
/// `SkillFxMaterial` built from the entry, plus an optional point-light pop and an
/// optional tintable spark garnish when the entry declares them. The light `range`
/// (45.0) is the one light knob the entry does not carry.
#[allow(clippy::too_many_arguments)]
pub fn spawn_shader_fx(
    commands: &mut Commands,
    materials: &mut Assets<SkillFxMaterial>,
    asset_server: &AssetServer,
    assets: &ImpactAssets,
    entry: &ShaderFxEntry,
    position: Vec3,
    source: Option<Vec3>,
    hits: u32,
    sound: Option<String>,
    key: &str,
    color: Color,
) {
    if let (Some(travel), Some(source)) = (&entry.travel, source) {
        // Bolts throw one orb per hit in sequence; a single-ball skill throws one
        // regardless of how many times it hits.
        let count = if travel.per_hit { hits.max(1) } else { 1 };
        for i in 0..count {
            spawn_projectile(
                commands,
                materials,
                asset_server,
                assets,
                entry,
                travel,
                source,
                position,
                i as f32 * travel.stagger,
                sound.clone(),
                key,
                color,
            );
        }
        return;
    }

    let (texture, flipbook) =
        resolve_fx_texture(asset_server, entry.texture.as_ref(), entry.frames.as_ref());

    let material = materials.add(SkillFxMaterial {
        params: SkillFxParams {
            kind: entry.kind,
            primary: entry.primary.into(),
            secondary: entry.secondary.into(),
            shape: entry.shape.into(),
            factor: 0.0,
        },
        texture,
    });

    commands
        .spawn((
            FactorRamp::new(entry.duration),
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .with_children(|parent| {
            let mut quad = parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(entry.scale)),
                NotShadowCaster,
            ));
            if let Some(flipbook) = flipbook {
                quad.insert(flipbook);
            }
            if let Some(light) = &entry.light {
                let peak = light.intensity_scale * LIGHT_PEAK;
                parent.spawn((
                    PointLight {
                        color: Color::srgb(light.color.0, light.color.1, light.color.2),
                        intensity: peak,
                        range: 45.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    LightFade::new(light.fade, peak),
                ));
            }
            if let Some(garnish) = &entry.garnish {
                parent.spawn(spark_garnish_bundle(assets, garnish.tint.into()));
            }
        });
}

/// Spawn the traveling projectile: the entry's own colors on the projectile
/// fragment, sampling `travel.texture` (falling back to the entry's burst
/// texture), plus a steady point light so the orb lights its path. Tagged
/// `TravelingVfx` for `advance_traveling_vfx` to move and detonate.
#[allow(clippy::too_many_arguments)]
fn spawn_projectile(
    commands: &mut Commands,
    materials: &mut Assets<SkillFxMaterial>,
    asset_server: &AssetServer,
    assets: &ImpactAssets,
    entry: &ShaderFxEntry,
    travel: &ShaderFxTravel,
    source: Vec3,
    target: Vec3,
    delay: f32,
    sound: Option<String>,
    key: &str,
    color: Color,
) {
    // The projectile prefers its own `travel` art; only when travel declares
    // neither frames nor a single texture does it fall back to the burst's.
    let (single, frames) = if travel.frames.is_some() || travel.texture.is_some() {
        (travel.texture.as_ref(), travel.frames.as_ref())
    } else {
        (entry.texture.as_ref(), entry.frames.as_ref())
    };
    let (texture, flipbook) = resolve_fx_texture(asset_server, single, frames);

    let material = materials.add(SkillFxMaterial {
        params: SkillFxParams {
            kind: PROJECTILE_KIND,
            primary: entry.primary.into(),
            secondary: entry.secondary.into(),
            shape: entry.shape.into(),
            factor: 0.0,
        },
        texture,
    });

    // A staggered bolt waits hidden at the caster until its launch slot.
    let visibility = if delay > 0.0 {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    commands
        .spawn((
            TravelingVfx {
                key: key.to_string(),
                target,
                speed: travel.speed,
                delay,
                sound,
                launched: false,
                color,
            },
            Transform::from_translation(source),
            visibility,
        ))
        .with_children(|parent| {
            let mut quad = parent.spawn((
                Mesh3d(assets.quad.clone()),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(travel.scale)),
                NotShadowCaster,
            ));
            if let Some(flipbook) = flipbook {
                quad.insert(flipbook);
            }
            if let Some(light) = &entry.light {
                parent.spawn(PointLight {
                    color: Color::srgb(light.color.0, light.color.1, light.color.2),
                    intensity: light.intensity_scale * LIGHT_PEAK * 0.5,
                    range: 45.0,
                    shadow_maps_enabled: false,
                    ..default()
                });
            }
        });
}

/// Advance every `TravelingVfx` toward its target. On arrival (this frame's step
/// covers the remaining distance) it despawns the projectile and writes a fresh
/// `PlayProceduralVfx` at the target with no `source`, so the normal burst path
/// plays the detonation there. Runs in `VfxSystems`; the one-frame gap before the
/// burst reader picks the message up is imperceptible.
pub fn advance_traveling_vfx(
    time: Res<Time>,
    mut commands: Commands,
    mut travelers: Query<(Entity, &mut Transform, &mut Visibility, &mut TravelingVfx)>,
    mut proc_vfx: MessageWriter<PlayProceduralVfx>,
    mut sfx: MessageWriter<PlaySkillSfx>,
) {
    let step = time.delta_secs();
    for (entity, mut transform, mut visibility, mut travel) in &mut travelers {
        if travel.delay > 0.0 {
            travel.delay -= step;
            continue;
        }
        // First active frame: reveal the orb and fire its whoosh from itself, so
        // the sound travels with the bolt. Runs once per orb (`launched` gate).
        if !travel.launched {
            travel.launched = true;
            *visibility = Visibility::Inherited;
            if let Some(sound) = &travel.sound {
                sfx.write(PlaySkillSfx {
                    emitter: entity,
                    sound: sound.clone(),
                });
            }
        }
        let to_target = travel.target - transform.translation;
        let dist = to_target.length();
        let advance = travel.speed * step;
        if dist <= advance || dist < 1e-4 {
            // The arrival burst is not a travel effect: no source, no sound (the
            // whoosh already played on launch), just the impact at the target.
            proc_vfx.write(PlayProceduralVfx {
                key: travel.key.clone(),
                position: travel.target,
                source: None,
                hits: 1,
                sound: None,
                color: travel.color,
            });
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation += to_target / dist * advance;
    }
}

/// Registers the `SkillFxMaterial` `MaterialPlugin` and its factor driver.
/// `HanabiPlugin` is owned by the parent `VfxPlugin`, not here.
pub struct SkillFxPlugin;

impl Plugin for SkillFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkillFxMaterial>::default())
            .add_systems(
                Update,
                (
                    drive_factor::<SkillFxMaterial>,
                    advance_traveling_vfx,
                    animate_flipbook,
                    dress_sight_orbits,
                )
                    .in_set(VfxSystems),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_factor_writes_factor_uniform() {
        let mut material = SkillFxMaterial {
            params: SkillFxParams {
                kind: 0,
                primary: Vec4::ONE,
                secondary: Vec4::ONE,
                shape: Vec4::ZERO,
                factor: 0.0,
            },
            texture: None,
        };
        material.set_factor(0.6);
        assert_eq!(material.params.factor, 0.6);
    }

    fn asset_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Image>();
        app
    }

    fn travel_entry(per_hit: bool) -> ShaderFxEntry {
        ShaderFxEntry {
            kind: 2,
            primary: (1.0, 1.0, 1.0, 1.0),
            secondary: (1.0, 1.0, 1.0, 1.0),
            shape: (1.0, 0.0, 0.0, 0.0),
            duration: 0.5,
            scale: 18.0,
            light: None,
            garnish: None,
            texture: None,
            frames: None,
            travel: Some(ShaderFxTravel {
                speed: 100.0,
                scale: 8.0,
                texture: Some("data/texture/effect/waterorb.bmp".into()),
                frames: None,
                per_hit,
                stagger: 0.12,
            }),
        }
    }

    fn travel_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Image>()
            .init_asset::<Mesh>()
            .init_asset::<bevy_hanabi::EffectAsset>()
            .init_asset::<SkillFxMaterial>()
            .init_resource::<ImpactAssets>();
        app
    }

    fn traveling_count(app: &mut App) -> usize {
        app.world_mut()
            .query::<&TravelingVfx>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn per_hit_travel_launches_one_orb_per_hit() {
        let mut app = travel_app();
        app.add_systems(
            Update,
            |mut commands: Commands,
             mut mats: ResMut<Assets<SkillFxMaterial>>,
             server: Res<AssetServer>,
             assets: Res<ImpactAssets>| {
                spawn_shader_fx(
                    &mut commands,
                    &mut mats,
                    &server,
                    &assets,
                    &travel_entry(true),
                    Vec3::new(20.0, 0.0, 0.0),
                    Some(Vec3::ZERO),
                    3,
                    None,
                    "cold_bolt",
                    Color::WHITE,
                );
            },
        );
        app.update();
        assert_eq!(traveling_count(&mut app), 3, "3 hits launch 3 bolts");
    }

    #[test]
    fn non_per_hit_travel_launches_one_orb_regardless_of_hits() {
        let mut app = travel_app();
        app.add_systems(
            Update,
            |mut commands: Commands,
             mut mats: ResMut<Assets<SkillFxMaterial>>,
             server: Res<AssetServer>,
             assets: Res<ImpactAssets>| {
                spawn_shader_fx(
                    &mut commands,
                    &mut mats,
                    &server,
                    &assets,
                    &travel_entry(false),
                    Vec3::new(20.0, 0.0, 0.0),
                    Some(Vec3::ZERO),
                    5,
                    None,
                    "jupitel_thunder",
                    Color::WHITE,
                );
            },
        );
        app.update();
        assert_eq!(
            traveling_count(&mut app),
            1,
            "a single-ball skill launches one orb even for 5 hits"
        );
    }

    #[test]
    fn resolve_prefers_frames_over_single_texture() {
        let app = asset_app();
        let server = app.world().resource::<AssetServer>();
        let frames = TextureFrames {
            paths: vec![
                "data/texture/effect/thunder_ball_0001.bmp".into(),
                "data/texture/effect/thunder_ball_0002.bmp".into(),
            ],
            fps: 12.0,
        };
        let single = "data/texture/effect/thunder_pang.bmp".to_string();

        let (texture, flipbook) = resolve_fx_texture(server, Some(&single), Some(&frames));
        assert!(texture.is_some(), "frame 0 is bound immediately");
        let flipbook = flipbook.expect("a series yields a flipbook");
        assert_eq!(flipbook.frames.len(), 2);
        assert_eq!(flipbook.fps, 12.0);

        let (texture, flipbook) = resolve_fx_texture(server, Some(&single), None);
        assert!(texture.is_some());
        assert!(flipbook.is_none(), "a single texture yields no flipbook");
    }

    #[test]
    fn sight_orbit_anchor_gets_flame_quad_and_light() {
        let mut app = travel_app();
        app.add_systems(Update, dress_sight_orbits);

        let unit = app.world_mut().spawn_empty().id();
        let orbit = app
            .world_mut()
            .spawn((
                SightOrbit { unit },
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<PointLight>(orbit).is_some(),
            "the anchor carries the flame's light"
        );
        let children = app.world().get::<Children>(orbit).expect("flame child");
        let flame = children
            .iter()
            .find(|child| {
                app.world()
                    .get::<MeshMaterial3d<SkillFxMaterial>>(*child)
                    .is_some()
            })
            .expect("a flame quad child");
        assert!(
            app.world().get::<FxFlipbook>(flame).is_some(),
            "the flame cycles its flicker frames"
        );

        // Dressing runs once: another update must not stack a second quad.
        app.update();
        let count = app
            .world()
            .get::<Children>(orbit)
            .map(|children| children.iter().count())
            .unwrap_or(0);
        assert_eq!(count, 1, "Added-filter dresses each anchor exactly once");
    }

    #[test]
    fn traveling_vfx_reaches_target_then_replays_burst() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<PlayProceduralVfx>()
            .add_message::<PlaySkillSfx>()
            .add_systems(Update, advance_traveling_vfx);

        let entity = app
            .world_mut()
            .spawn((
                TravelingVfx {
                    key: "jupitel_thunder".into(),
                    target: Vec3::new(10.0, 0.0, 0.0),
                    speed: 100.0,
                    delay: 0.0,
                    sound: Some("effect/ef_lightbolt.wav".into()),
                    launched: false,
                    color: Color::WHITE,
                },
                Transform::from_translation(Vec3::ZERO),
                Visibility::default(),
            ))
            .id();

        // 0.05s * 100 u/s = 5 units < 10: still traveling, no burst yet.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.05,
        )));
        app.update();
        assert!(
            app.world().get::<TravelingVfx>(entity).is_some(),
            "projectile still in flight before arrival"
        );
        let pos = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(pos.x > 0.0 && pos.x < 10.0, "advanced toward target");

        // The whoosh fired once, from the orb, when it launched.
        let sfx = app.world().resource::<Messages<PlaySkillSfx>>();
        let mut sfx_cursor = sfx.get_cursor();
        let whooshes: Vec<_> = sfx_cursor.read(sfx).collect();
        assert_eq!(whooshes.len(), 1, "one launch whoosh");
        assert_eq!(whooshes[0].emitter, entity, "whoosh anchored to the orb");
        assert_eq!(whooshes[0].sound, "effect/ef_lightbolt.wav");

        // Another 0.1s * 100 = 10 units covers the rest: arrives, despawns, emits.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.1,
        )));
        app.update();
        assert!(
            app.world().get::<TravelingVfx>(entity).is_none(),
            "projectile despawns on arrival"
        );
        let messages = app.world().resource::<Messages<PlayProceduralVfx>>();
        let mut cursor = messages.get_cursor();
        let emitted: Vec<_> = cursor.read(messages).collect();
        assert_eq!(emitted.len(), 1, "burst replayed once on arrival");
        assert_eq!(emitted[0].key, "jupitel_thunder");
        assert_eq!(emitted[0].position, Vec3::new(10.0, 0.0, 0.0));
        assert!(
            emitted[0].source.is_none(),
            "the replayed burst does not travel again"
        );
    }
}
