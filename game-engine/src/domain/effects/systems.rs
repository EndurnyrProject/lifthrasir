use super::components::{
    ActiveEffect, EffectAnchor, EffectFrameTimer, EffectLayer, EffectLifetime,
};
use crate::domain::entities::billboard::{create_sprite_quad_mesh, Billboard};
use crate::infrastructure::effect::{LoadedEffectAsset, LoadedFrame, LoadedLayer};
use crate::presentation::rendering::effect_material::{alpha_mode_for, EffectMaterial};
use bevy::prelude::*;

/// Pixel-to-world scale for STR quad corners. STR `xy` corners are authored in
/// screen pixels; this maps them into Bevy world units. Tunable — start in the
/// same ballpark as `SPRITE_WORLD_SCALE` (0.2) and adjust once effects render.
pub const STR_WORLD_SCALE: f32 = 0.2;

/// STR raw angle units per full turn: `angle / (1024/360)` gives degrees
/// (korangar's effect loader). We keep angles raw through interpolation and
/// convert here.
const STR_ANGLE_UNITS_PER_DEGREE: f32 = 1024.0 / 360.0;

/// The interpolated, ready-to-render frame for a single layer at one key:
/// four quad corners (2D, pre-scale), four UVs, an RGBA colour (0..1), a
/// rotation in radians and the texture slice to sample.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    pub corners: [Vec2; 4],
    pub uvs: [Vec2; 4],
    pub color: [f32; 4],
    pub angle_radians: f32,
    pub texture_index: usize,
}

/// Resolve the layer's frame active at `current_frame`, interpolating towards
/// the frame two slots ahead (korangar's `interpolate_frame`). `None` when no
/// frame is active at this key (the layer hides this frame).
pub fn interpolate_layer_frame(layer: &LoadedLayer, current_frame: usize) -> Option<RenderFrame> {
    let frame_index = (*layer.frame_index_map.get(current_frame)?)?;
    let first = layer.frames.get(frame_index)?;

    let interpolated = match layer.frames.get(frame_index + 2) {
        Some(second) => interpolate(first, second, current_frame),
        None => loaded_to_render(first),
    };

    Some(interpolated)
}

/// Linearly interpolate two STR frames at `current_frame` (korangar's
/// `Layer::interpolate`). `time` is the fraction between the two declared
/// keys; angle/xy lerp with no easing bias, colour/uv lerp linearly.
fn interpolate(first: &LoadedFrame, second: &LoadedFrame, current_frame: usize) -> RenderFrame {
    let span = (second.frame_index as f32 - first.frame_index as f32).max(f32::EPSILON);
    let time = (current_frame as f32 - first.frame_index as f32) / span;

    let lerp = |a: f32, b: f32| (b - a) * time + a;

    let color: [f32; 4] = std::array::from_fn(|i| lerp(first.color[i], second.color[i]) / 255.0);
    let xy: [f32; 8] = std::array::from_fn(|i| lerp(first.xy[i], second.xy[i]));
    let uv: [f32; 8] = std::array::from_fn(|i| lerp(first.uv[i], second.uv[i]));
    let angle = lerp(first.angle, second.angle);

    RenderFrame {
        corners: corners_from_xy(&xy),
        uvs: uvs_from_uv(&uv),
        color,
        angle_radians: (angle / STR_ANGLE_UNITS_PER_DEGREE).to_radians(),
        // Texture index is held by the leading frame (no interpolation), matching korangar.
        texture_index: first.texture_index,
    }
}

/// Convert a single (non-interpolated) loaded frame into a render frame.
fn loaded_to_render(frame: &LoadedFrame) -> RenderFrame {
    RenderFrame {
        corners: corners_from_xy(&frame.xy),
        uvs: uvs_from_uv(&frame.uv),
        color: [
            frame.color[0] / 255.0,
            frame.color[1] / 255.0,
            frame.color[2] / 255.0,
            frame.color[3] / 255.0,
        ],
        angle_radians: (frame.angle / STR_ANGLE_UNITS_PER_DEGREE).to_radians(),
        texture_index: frame.texture_index,
    }
}

/// Map the STR `xy[8]` deformable quad into 4 corners (korangar's `Effect::render`
/// corner ordering): each corner is `(x_i, y_{i+4})`.
fn corners_from_xy(xy: &[f32; 8]) -> [Vec2; 4] {
    [
        Vec2::new(xy[0], xy[4]),
        Vec2::new(xy[1], xy[5]),
        Vec2::new(xy[3], xy[7]),
        Vec2::new(xy[2], xy[6]),
    ]
}

/// Map the STR `uv[8]` into 4 corner UVs (korangar's `Effect::render`).
fn uvs_from_uv(uv: &[f32; 8]) -> [Vec2; 4] {
    [
        Vec2::new(uv[0] + uv[2], uv[3] + uv[1]),
        Vec2::new(uv[0] + uv[2], uv[1]),
        Vec2::new(uv[0], uv[1]),
        Vec2::new(uv[0], uv[3] + uv[1]),
    ]
}

/// Tick each playing effect's frame timer; wrap repeating effects, mark
/// non-repeating effects finished once they pass `max_key`.
pub fn advance_effect_timers(time: Res<Time>, mut effects: Query<&mut ActiveEffect>) {
    let delta = time.delta_secs();
    for mut effect in &mut effects {
        if effect.finished {
            continue;
        }
        let still_running = effect.timer.update(delta);
        if !still_running && !effect.repeating {
            effect.finished = true;
        }
    }
}

/// Copy `Entity` anchors' world position onto the effect parent each frame so
/// effects track moving units. `Position` anchors are written once at spawn and
/// left untouched.
pub fn follow_effect_anchor(
    anchored: Query<&GlobalTransform>,
    mut effects: Query<(&EffectAnchor, &mut Transform)>,
) {
    for (anchor, mut transform) in &mut effects {
        if let EffectAnchor::Entity(target) = anchor {
            if let Ok(global) = anchored.get(*target) {
                transform.translation = global.translation();
            }
        }
    }
}

type EffectInitQuery<'w, 's> = Query<'w, 's, (Entity, &'static mut ActiveEffect)>;

/// Lazily create the per-layer child entities once the `LoadedEffectAsset` is
/// available. Each layer is its own billboarded `Mesh3d` + `EffectMaterial`
/// child with a *fresh* mutable quad mesh (rewritten per frame). Chosen over
/// requiring the asset up front because effects can be triggered before the
/// `.str` finishes loading (async asset load).
pub fn initialize_effect_layers(
    mut commands: Commands,
    mut effects: EffectInitQuery,
    loaded: Res<Assets<LoadedEffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EffectMaterial>>,
) {
    for (entity, mut effect) in &mut effects {
        if effect.layers_initialized {
            continue;
        }
        let Some(asset) = loaded.get(&effect.effect) else {
            continue;
        };

        // The timer's fps/max_key live in the asset, which may not have been
        // loaded when the effect was spawned, so set them here once it is.
        effect.timer.fps = asset.fps;
        effect.timer.max_key = asset.max_key;

        for layer_index in 0..asset.layers.len() {
            let mesh = meshes.add(create_sprite_quad_mesh());
            let material = materials.add(EffectMaterial {
                base_color_texture: Handle::default(),
                alpha_mode: AlphaMode::Add,
            });

            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Billboard,
                EffectLayer { layer_index },
                Transform::default(),
                Visibility::Hidden,
                ChildOf(entity),
            ));
        }

        effect.layers_initialized = true;
    }
}

type LayerRebuildQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EffectLayer,
        &'static ChildOf,
        &'static Mesh3d,
        &'static MeshMaterial3d<EffectMaterial>,
        &'static mut Visibility,
    ),
>;

/// Rewrite every effect layer's mesh + material for the current key: quad
/// corners / UV / colour into the mesh attributes, texture + blend onto the
/// material. Layers with no frame at this key are hidden.
pub fn rebuild_effect_layers(
    effects: Query<&ActiveEffect>,
    mut layers: LayerRebuildQuery,
    loaded: Res<Assets<LoadedEffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EffectMaterial>>,
) {
    for (layer, child_of, mesh3d, material3d, mut visibility) in &mut layers {
        let Ok(effect) = effects.get(child_of.parent()) else {
            continue;
        };
        let Some(asset) = loaded.get(&effect.effect) else {
            continue;
        };
        let Some(loaded_layer) = asset.layers.get(layer.layer_index) else {
            continue;
        };

        let Some(frame) = interpolate_layer_frame(loaded_layer, effect.timer.current_frame) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let Some(mesh) = meshes.get_mut(&mesh3d.0) else {
            continue;
        };
        write_frame_to_mesh(mesh, &frame, effect.tint);

        if let Some(material) = materials.get_mut(&material3d.0) {
            material.base_color_texture = loaded_layer
                .textures
                .get(frame.texture_index)
                .cloned()
                .unwrap_or_default();
            material.alpha_mode = alpha_mode_for(loaded_layer.blend);
        }

        *visibility = Visibility::Visible;
    }
}

/// Write a render frame's geometry into the layer's quad mesh: rotated, scaled
/// corner positions, per-corner UVs, and the frame colour multiplied by the
/// effect tint as the vertex colour.
fn write_frame_to_mesh(mesh: &mut Mesh, frame: &RenderFrame, tint: Color) {
    let rotation = Mat2::from_angle(frame.angle_radians);
    let tint = tint.to_linear();

    let positions: Vec<[f32; 3]> = frame
        .corners
        .iter()
        .map(|corner| {
            let rotated = rotation * *corner * STR_WORLD_SCALE;
            [rotated.x, rotated.y, 0.0]
        })
        .collect();

    let uvs: Vec<[f32; 2]> = frame.uvs.iter().map(|uv| [uv.x, uv.y]).collect();

    let vertex_color = [
        frame.color[0] * tint.red,
        frame.color[1] * tint.green,
        frame.color[2] * tint.blue,
        frame.color[3] * tint.alpha,
    ];
    let colors: Vec<[f32; 4]> = vec![vertex_color; 4];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}

/// Despawn finished one-shot effects (recursively, taking their layer children)
/// and expired repeating effects whose `EffectLifetime` has elapsed.
pub fn despawn_finished_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &ActiveEffect, Option<&mut EffectLifetime>)>,
) {
    for (entity, effect, lifetime) in &mut effects {
        if effect.finished {
            commands.entity(entity).despawn();
            continue;
        }

        if let Some(mut lifetime) = lifetime {
            if lifetime.0.tick(time.delta()).just_finished() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Spawn a playing effect: the parent instance entity carrying `ActiveEffect`
/// and an `EffectAnchor`. Layer children are created lazily by
/// `initialize_effect_layers` once the `LoadedEffectAsset` is available, so this
/// is safe to call before the `.str` has finished loading and needs no mutable
/// `Assets` access. `lifetime`, when given, despawns a repeating effect after
/// it elapses (ground effects have no removal packet). The frame timer's
/// `fps`/`max_key` are filled in from the asset by `initialize_effect_layers`.
pub fn spawn_effect(
    commands: &mut Commands,
    effect: Handle<LoadedEffectAsset>,
    anchor: EffectAnchor,
    repeating: bool,
    tint: Color,
    lifetime: Option<Timer>,
) -> Entity {
    let initial_translation = match anchor {
        EffectAnchor::Position(position) => position,
        EffectAnchor::Entity(_) => Vec3::ZERO,
    };

    let mut entity = commands.spawn((
        ActiveEffect {
            effect,
            timer: EffectFrameTimer::new(0, 0),
            repeating,
            tint,
            layers_initialized: false,
            finished: false,
        },
        anchor,
        Transform::from_translation(initial_translation),
        Visibility::default(),
    ));

    if let Some(timer) = lifetime {
        entity.insert(EffectLifetime(timer));
    }

    entity.id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::effect::EffectBlend;

    fn frame(frame_index: usize, value: f32, angle: f32, color_r: f32) -> LoadedFrame {
        LoadedFrame {
            frame_index,
            xy: [value; 8],
            uv: [value; 8],
            texture_index: 0,
            color: [color_r, 0.0, 0.0, 255.0],
            angle,
            blend: EffectBlend::Add,
        }
    }

    #[test]
    fn interpolate_lerps_corner_uv_color_angle_at_midpoint() {
        // Two declared keys (0 and 4); a frame two slots ahead drives interpolation.
        // The frame_index_map points key 2 at frame slot 0; slot 2 is the second key.
        let layer = LoadedLayer {
            textures: vec![],
            // key 2 -> frame slot 0 (the leading frame); slot 0+2 = slot 2 (second key).
            frame_index_map: vec![None, None, Some(0), None, None],
            frames: vec![
                frame(0, 0.0, 0.0, 0.0),        // slot 0: leading
                frame(0, 0.0, 0.0, 0.0),        // slot 1: filler (korangar lerps slot i -> i+2)
                frame(4, 100.0, 1024.0, 255.0), // slot 2: trailing
            ],
            blend: EffectBlend::Add,
        };

        let rendered = interpolate_layer_frame(&layer, 2).expect("frame active at key 2");

        // time = (2 - 0) / (4 - 0) = 0.5, so every lerped quantity is the midpoint.
        // xy 0..100 -> 50 (raw; STR_WORLD_SCALE is applied later at mesh write).
        // corner 0 = (xy[0], xy[4]) = (50, 50).
        assert!((rendered.corners[0].x - 50.0).abs() < 1e-4);
        assert!((rendered.corners[0].y - 50.0).abs() < 1e-4);

        // uv corner 2 = (uv[0], uv[1]) = (50, 50) at the midpoint.
        assert!((rendered.uvs[2].x - 50.0).abs() < 1e-4);
        assert!((rendered.uvs[2].y - 50.0).abs() < 1e-4);

        // colour lerps 0..255 -> 127.5, then /255 -> 0.5.
        assert!((rendered.color[0] - 0.5).abs() < 1e-4);
        // alpha is 255 on both -> 1.0.
        assert!((rendered.color[3] - 1.0).abs() < 1e-4);

        // angle lerps 0..1024 raw -> 512; 512 / (1024/360) = 180 degrees = PI.
        assert!((rendered.angle_radians - std::f32::consts::PI).abs() < 1e-3);
    }

    #[test]
    fn interpolate_layer_frame_hidden_when_no_frame() {
        let layer = LoadedLayer {
            textures: vec![],
            frame_index_map: vec![None, None],
            frames: vec![frame(0, 0.0, 0.0, 0.0)],
            blend: EffectBlend::Add,
        };
        assert!(interpolate_layer_frame(&layer, 1).is_none());
    }

    #[test]
    fn frame_timer_finishes_after_max_key() {
        let mut timer = EffectFrameTimer::new(10, 5); // 10 fps, 5 keys -> 0.5s total.
        assert!(timer.update(0.1)); // key 1
        assert_eq!(timer.current_frame, 1);
        assert!(timer.update(0.3)); // total 0.4s -> key 4, still < 5
        assert_eq!(timer.current_frame, 4);
        assert!(!timer.update(0.2)); // total 0.6s -> key 6 >= 5 -> wraps, returns false
        assert_eq!(timer.current_frame, 0);
    }

    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn stub_effect(layer_count: usize, max_key: u32) -> LoadedEffectAsset {
        let layers = (0..layer_count)
            .map(|_| LoadedLayer {
                textures: vec![],
                frame_index_map: vec![Some(0); max_key as usize],
                frames: vec![frame(0, 1.0, 0.0, 255.0)],
                blend: EffectBlend::Add,
            })
            .collect();
        LoadedEffectAsset {
            fps: 10,
            max_key,
            layers,
        }
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<LoadedEffectAsset>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<EffectMaterial>();
        app
    }

    /// Advance virtual time by `seconds`, stepping in 0.2s chunks so each
    /// update stays under `Time<Virtual>`'s default 0.25s `max_delta` clamp
    /// (otherwise a large jump is silently truncated).
    fn advance(app: &mut App, seconds: f32) {
        let step = 0.2;
        let mut remaining = seconds;
        while remaining > 0.0 {
            let dt = remaining.min(step);
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                dt,
            )));
            app.update();
            remaining -= dt;
        }
    }

    /// The first `Time` update only establishes the baseline (zero delta), so
    /// tests that rely on `delta_secs()` need one warm-up update before advancing.
    fn warm_up(app: &mut App) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
    }

    #[test]
    fn spawn_effect_then_init_creates_one_layer_child_per_layer() {
        let mut app = test_app();
        app.add_systems(Update, initialize_effect_layers);

        let handle = app
            .world_mut()
            .resource_mut::<Assets<LoadedEffectAsset>>()
            .add(stub_effect(3, 4));

        app.world_mut().commands().queue(move |world: &mut World| {
            let mut commands = world.commands();
            spawn_effect(
                &mut commands,
                handle,
                EffectAnchor::Position(Vec3::ZERO),
                false,
                Color::WHITE,
                None,
            );
        });
        app.world_mut().flush();

        // One parent with ActiveEffect right after spawn.
        let parents = app
            .world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count();
        assert_eq!(parents, 1);

        // Run init so the layer children are created.
        app.update();

        let layer_children = app
            .world_mut()
            .query::<&EffectLayer>()
            .iter(app.world())
            .count();
        assert_eq!(layer_children, 3);
    }

    #[test]
    fn non_repeating_effect_despawns_after_max_key() {
        let mut app = test_app();
        app.add_systems(
            Update,
            (
                initialize_effect_layers,
                advance_effect_timers,
                despawn_finished_effects,
            )
                .chain(),
        );

        let handle = app
            .world_mut()
            .resource_mut::<Assets<LoadedEffectAsset>>()
            .add(stub_effect(1, 4)); // 4 keys @ 10 fps -> finishes at 0.4s.

        app.world_mut().commands().queue(move |world: &mut World| {
            let mut commands = world.commands();
            spawn_effect(
                &mut commands,
                handle,
                EffectAnchor::Position(Vec3::ZERO),
                false,
                Color::WHITE,
                None,
            );
        });
        app.world_mut().flush();
        warm_up(&mut app);

        // Past max_key / fps (0.4s): the one-shot finishes and despawns.
        advance(&mut app, 0.5);
        let alive = app
            .world_mut()
            .query::<&ActiveEffect>()
            .iter(app.world())
            .count();
        assert_eq!(alive, 0, "non-repeating effect should despawn past max_key");
    }

    #[test]
    fn repeating_effect_survives_until_lifetime_expires() {
        let mut app = test_app();
        app.add_systems(
            Update,
            (
                initialize_effect_layers,
                advance_effect_timers,
                despawn_finished_effects,
            )
                .chain(),
        );

        let handle = app
            .world_mut()
            .resource_mut::<Assets<LoadedEffectAsset>>()
            .add(stub_effect(1, 4));

        app.world_mut().commands().queue(move |world: &mut World| {
            let mut commands = world.commands();
            spawn_effect(
                &mut commands,
                handle,
                EffectAnchor::Position(Vec3::ZERO),
                true, // repeating
                Color::WHITE,
                Some(Timer::from_seconds(1.0, TimerMode::Once)),
            );
        });
        app.world_mut().flush();
        warm_up(&mut app);

        // Past one loop but before the 1s lifetime: still alive.
        advance(&mut app, 0.5);
        assert_eq!(
            app.world_mut()
                .query::<&ActiveEffect>()
                .iter(app.world())
                .count(),
            1,
            "repeating effect should survive its first loop"
        );

        // Past the lifetime: despawned.
        advance(&mut app, 0.6);
        assert_eq!(
            app.world_mut()
                .query::<&ActiveEffect>()
                .iter(app.world())
                .count(),
            0,
            "repeating effect should despawn once lifetime expires"
        );
    }
}
