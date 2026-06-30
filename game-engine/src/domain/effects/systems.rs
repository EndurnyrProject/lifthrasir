use super::components::{
    ActiveEffect, EffectAnchor, EffectFrameTimer, EffectLayer, EffectLifetime,
};
use crate::domain::entities::billboard::Billboard;
use crate::infrastructure::effect::{EffectBlend, LoadedEffectAsset, LoadedFrame, LoadedLayer};
use crate::presentation::rendering::effect_material::{alpha_mode_for, EffectMaterial};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Pixel-to-world scale for STR quad corners. STR `xy` corners are authored in
/// screen pixels; this maps them into Bevy world units. Tunable, start in the
/// same ballpark as `SPRITE_WORLD_SCALE` (0.2) and adjust once effects render.
pub const STR_WORLD_SCALE: f32 = 0.2;

/// STR raw angle units per full turn: `angle / (1024/360)` gives degrees
/// We keep angles raw through interpolation and convert here.
const STR_ANGLE_UNITS_PER_DEGREE: f32 = 1024.0 / 360.0;

/// STR layer `offset`s are authored relative to this screen-space origin.
/// Subtracting it re-centres each layer on the
/// effect's world anchor and gives animated layers the correct rotation pivot.
const EFFECT_ORIGIN: Vec2 = Vec2::new(319.0, 291.0);

/// Depth step (world units, along the camera axis) between consecutive effect
/// layers. STR layers are authored in paint order (0 = back … N = front), but
/// our per-layer billboards are coplanar at the anchor, so the transparent pass
/// would otherwise composite them in an arbitrary order, stacking pieces wrong
/// (e.g. a wing or a flanking sprite over the angel's face). Biasing each layer's
/// depth by its index restores the authored order; the offset is purely along the
/// view axis, so nothing moves on screen.
const EFFECT_LAYER_DEPTH_STEP: f32 = 0.01;

/// Forward depth bias (world units, toward the camera) applied to solid
/// (alpha-blended / multiply) layers so they all draw in front of the additive
/// glow layers. Without this, additive wings/sparkles sitting on a solid figure
/// wash its face out, additive brightens regardless of draw order, so plain
/// layer ordering cannot fix it. Larger than the whole `index * STEP` span so the
/// two tiers never interleave.
const EFFECT_SOLID_TIER_DEPTH: f32 = 1.0;

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
    /// Layer offset (STR screen space, pre-`EFFECT_ORIGIN`), interpolated.
    pub offset: Vec2,
}

/// Resolve the layer's frame active at `current_frame`, interpolating towards
/// the frame two slots ahead. `None` when no frame is active at this key (the layer hides this frame).
pub fn interpolate_layer_frame(layer: &LoadedLayer, current_frame: usize) -> Option<RenderFrame> {
    let frame_index = (*layer.frame_index_map.get(current_frame)?)?;
    let first = layer.frames.get(frame_index)?;

    let interpolated = match layer.frames.get(frame_index + 2) {
        Some(second) => interpolate(first, second, current_frame),
        None => loaded_to_render(first),
    };

    Some(interpolated)
}

/// Linearly interpolate two STR frames at `current_frame`. `
/// time` is the fraction between the two declared
/// keys; angle/xy lerp with no easing bias, colour/uv lerp linearly.
fn interpolate(first: &LoadedFrame, second: &LoadedFrame, current_frame: usize) -> RenderFrame {
    let span = (second.frame_index as f32 - first.frame_index as f32).max(f32::EPSILON);
    let time = (current_frame as f32 - first.frame_index as f32) / span;

    let lerp = |a: f32, b: f32| (b - a) * time + a;

    let color: [f32; 4] = std::array::from_fn(|i| lerp(first.color[i], second.color[i]) / 255.0);
    let xy: [f32; 8] = std::array::from_fn(|i| lerp(first.xy[i], second.xy[i]));
    let uv: [f32; 8] = std::array::from_fn(|i| lerp(first.uv[i], second.uv[i]));
    let angle = lerp(first.angle, second.angle);
    let offset = first.offset.lerp(second.offset, time);

    RenderFrame {
        corners: corners_from_xy(&xy),
        uvs: uvs_from_uv(&uv),
        color,
        angle_radians: (angle / STR_ANGLE_UNITS_PER_DEGREE).to_radians(),
        offset,
        // Texture index is held by the leading frame (no interpolation)
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
        offset: frame.offset,
        texture_index: frame.texture_index,
    }
}

/// Map the STR `xy[8]` deformable quad into 4 corners: each corner is `(x_i, y_{i+4})`.
fn corners_from_xy(xy: &[f32; 8]) -> [Vec2; 4] {
    [
        Vec2::new(xy[0], xy[4]),
        Vec2::new(xy[1], xy[5]),
        Vec2::new(xy[3], xy[7]),
        Vec2::new(xy[2], xy[6]),
    ]
}

/// Map the STR `uv[8]` into 4 corner UVs, paired to `corners_from_xy`'s order.
fn uvs_from_uv(uv: &[f32; 8]) -> [Vec2; 4] {
    [
        Vec2::new(uv[0], uv[1]),                 // corner 0 <- texcoord[2]
        Vec2::new(uv[0] + uv[2], uv[1]),         // corner 1 <- texcoord[1]
        Vec2::new(uv[0], uv[3] + uv[1]),         // corner 2 <- texcoord[3]
        Vec2::new(uv[0] + uv[2], uv[3] + uv[1]), // corner 3 <- texcoord[0]
    ]
}

/// Tick each playing effect's frame timer; wrap repeating effects, mark
/// non-repeating effects finished once they pass `max_key`.
///
/// Skips effects whose layers are not yet initialized: their timer still holds
/// the `new(0, 0)` placeholder `fps`/`max_key`, and accumulating wall-clock time
/// during the async `.str` load would make the first real tick jump deep into
/// (or past) the animation. `initialize_effect_layers` fills the real timing in.
pub fn advance_effect_timers(time: Res<Time>, mut effects: Query<&mut ActiveEffect>) {
    let delta = time.delta_secs();
    for mut effect in &mut effects {
        if effect.finished || !effect.layers_initialized {
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

/// Order the coplanar effect-layer billboards by their STR layer index so they
/// composite back-to-front like the original client, instead of being sorted
/// arbitrarily by the transparent pass. Each layer is nudged an
/// index-proportional amount along the camera axis (into depth only, so it does
/// not move on screen); higher indices sit closer to the camera and draw on top.
pub fn order_effect_layers_by_depth(
    camera: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<crate::domain::entities::billboard::EquipmentPreviewCamera>,
        ),
    >,
    mut layers: Query<(&EffectLayer, &mut Transform)>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let toward_camera = camera.back();
    for (layer, mut transform) in &mut layers {
        // Solid figures sit in a forward tier so additive glows can't wash them.
        let tier = if layer.additive {
            0.0
        } else {
            EFFECT_SOLID_TIER_DEPTH
        };
        transform.translation =
            toward_camera * (tier + layer.layer_index as f32 * EFFECT_LAYER_DEPTH_STEP);
    }
}

/// Build a fresh quad mesh for one effect layer. Its index topology matches the
/// corner order `write_frame_to_mesh` writes split into two triangles along the TL->BR
/// diagonal. The positions/UVs/colors are placeholders rewritten every frame;
/// only the indices and normals persist. This is deliberately *not* the shared
/// billboard quad: that mesh's indices assume a perimeter vertex order, and
/// pairing them with the corner order here tiles the four corners into a
/// degenerate, half-missing quad.
fn create_effect_layer_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0, 0.0, 0.0]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4]);
    // 0 = top-left, 1 = top-right, 2 = bottom-left, 3 = bottom-right.
    mesh.insert_indices(Indices::U32(vec![0, 1, 3, 0, 3, 2]));
    mesh
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

        for (layer_index, layer) in asset.layers.iter().enumerate() {
            let mesh = meshes.add(create_effect_layer_mesh());
            let material = materials.add(EffectMaterial {
                base_color_texture: Handle::default(),
                // Specialize the blend pipeline at creation: mutating `alpha_mode`
                // on an existing material does not reliably re-specialize, which
                // left non-additive layers (e.g. magnus's alpha-blended angel
                // sprites) stuck on the initial additive pipeline and rendering
                // faint/translucent. Blend is layer-constant, so set it once here.
                alpha_mode: alpha_mode_for(layer.blend),
            });

            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                NotShadowCaster,
                Billboard,
                EffectLayer {
                    layer_index,
                    additive: layer.blend == EffectBlend::Add,
                },
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

        let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
            continue;
        };
        write_frame_to_mesh(&mut mesh, &frame, effect.tint);

        if let Some(mut material) = materials.get_mut(&material3d.0) {
            // Blend is layer-constant and specialized when the material is created
            // (initialize_effect_layers); only the per-frame texture changes here.
            material.base_color_texture = loaded_layer
                .textures
                .get(frame.texture_index)
                .cloned()
                .unwrap_or_default();
        }

        *visibility = Visibility::Visible;
    }
}

/// Write a render frame's geometry into the layer's quad mesh: rotated, scaled
/// corner positions, per-corner UVs, and the frame colour multiplied by the
/// effect tint as the vertex colour.
fn write_frame_to_mesh(mesh: &mut Mesh, frame: &RenderFrame, tint: Color) {
    let rotation = Mat2::from_angle(frame.angle_radians);
    let offset = frame.offset - EFFECT_ORIGIN;
    let tint = tint.to_linear();

    let positions: Vec<[f32; 3]> = frame
        .corners
        .iter()
        .map(|corner| {
            let position = (rotation * *corner + offset) * STR_WORLD_SCALE;
            [position.x, -position.y, 0.0]
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
            offset: Vec2::splat(value),
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
                frame(0, 0.0, 0.0, 0.0),        // slot 1: filler
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

        // uv corner 0 pairs with texture-top-left = (uv[0], uv[1]) = (50, 50)
        // at the midpoint.
        assert!((rendered.uvs[0].x - 50.0).abs() < 1e-4);
        assert!((rendered.uvs[0].y - 50.0).abs() < 1e-4);

        // colour lerps 0..255 -> 127.5, then /255 -> 0.5.
        assert!((rendered.color[0] - 0.5).abs() < 1e-4);
        // alpha is 255 on both -> 1.0.
        assert!((rendered.color[3] - 1.0).abs() < 1e-4);

        // angle lerps 0..1024 raw -> 512; 512 / (1024/360) = 180 degrees = PI.
        assert!((rendered.angle_radians - std::f32::consts::PI).abs() < 1e-3);

        // offset lerps 0..100 -> 50 at the midpoint (carried, not dropped).
        assert!((rendered.offset.x - 50.0).abs() < 1e-4);
        assert!((rendered.offset.y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn effect_layer_mesh_indices_tile_corner_order_without_degenerate_winding() {
        // The mesh indices must triangulate `write_frame_to_mesh`'s corner order
        // [TL, TR, BL, BR] into a proper quad. The earlier bug reused the shared
        // billboard indices (authored for a perimeter vertex order), which split
        // these corners into two opposite-winding triangles that overlap and
        // leave a wedge uncovered — dropping half of every effect sprite.
        let corners = [
            [-0.5_f32, 0.5], // 0: top-left
            [0.5, 0.5],      // 1: top-right
            [-0.5, -0.5],    // 2: bottom-left
            [0.5, -0.5],     // 3: bottom-right
        ];

        let mesh = create_effect_layer_mesh();
        let Some(bevy::mesh::Indices::U32(indices)) = mesh.indices() else {
            panic!("expected U32 indices");
        };
        assert_eq!(indices.len(), 6, "two triangles");

        let signed_area = |tri: &[u32]| {
            let [a, b, c] = [
                corners[tri[0] as usize],
                corners[tri[1] as usize],
                corners[tri[2] as usize],
            ];
            0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]))
        };

        let first = signed_area(&indices[0..3]);
        let second = signed_area(&indices[3..6]);

        // Same-sign, non-zero areas => consistent winding (proper tiling, not a
        // bowtie). Their magnitudes must sum to the full unit-quad area (1.0):
        // the degenerate split overlaps, so it would fall short.
        assert!(first.abs() > f32::EPSILON && second.abs() > f32::EPSILON);
        assert_eq!(
            first.signum(),
            second.signum(),
            "triangles must share a winding"
        );
        assert!(
            (first.abs() + second.abs() - 1.0).abs() < 1e-4,
            "covers the quad"
        );
    }

    #[test]
    fn uvs_from_uv_pairs_each_corner_with_permuted_texcoord() {
        // Distinct, binary-exact u/v so a regression in the permutation is caught.
        // uv[0..4] = u0,u1,u2,u3; the rest is unused padding.
        let uv = [0.25, 0.5, 0.5, 0.25, 0.0, 0.0, 0.0, 0.0];
        let uvs = uvs_from_uv(&uv);

        // corners_from_xy order [0,1,2,3] must pair with texcoord [2,1,3,0].
        assert_eq!(uvs[0], Vec2::new(0.25, 0.5)); // texcoord[2] = (u0, u1)
        assert_eq!(uvs[1], Vec2::new(0.75, 0.5)); // texcoord[1] = (u0+u2, u1)
        assert_eq!(uvs[2], Vec2::new(0.25, 0.75)); // texcoord[3] = (u0, u3+u1)
        assert_eq!(uvs[3], Vec2::new(0.75, 0.75)); // texcoord[0] = (u0+u2, u3+u1)
    }

    #[test]
    fn order_effect_layers_by_depth_orders_by_index() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, order_effect_layers_by_depth);

        // Identity camera: back() = +Z, so the depth bias lands on translation.z.
        app.world_mut()
            .spawn((Camera3d::default(), GlobalTransform::default()));

        let layers: Vec<Entity> = (0..3usize)
            .map(|i| {
                app.world_mut()
                    .spawn((
                        EffectLayer {
                            layer_index: i,
                            additive: true,
                        },
                        Transform::default(),
                    ))
                    .id()
            })
            .collect();

        // A solid (non-additive) layer at index 0 must end up in the forward tier.
        let solid = app
            .world_mut()
            .spawn((
                EffectLayer {
                    layer_index: 0,
                    additive: false,
                },
                Transform::default(),
            ))
            .id();

        app.update();

        let depth = |e: Entity, app: &App| app.world().get::<Transform>(e).unwrap().translation.z;
        let depths: Vec<f32> = layers.iter().map(|e| depth(*e, &app)).collect();

        // Additive layer 0 sits at the anchor; higher indices are pushed toward the camera.
        assert!(depths[0].abs() < 1e-6);
        assert!(depths[0] < depths[1] && depths[1] < depths[2]);
        // The solid layer draws in front of every additive layer despite index 0.
        assert!(depth(solid, &app) > depths[2]);
    }

    #[test]
    fn order_effect_layers_ignores_equipment_preview_camera() {
        // The depth sort must keep working when the equipment-window preview camera
        // adds a second Camera3d; otherwise `single()` fails and effect layers
        // z-fight while the window is open.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, order_effect_layers_by_depth);

        app.world_mut()
            .spawn((Camera3d::default(), GlobalTransform::default()));
        app.world_mut().spawn((
            Camera3d::default(),
            GlobalTransform::default(),
            crate::domain::entities::billboard::EquipmentPreviewCamera,
        ));

        let lower = app
            .world_mut()
            .spawn((
                EffectLayer {
                    layer_index: 0,
                    additive: true,
                },
                Transform::default(),
            ))
            .id();
        let higher = app
            .world_mut()
            .spawn((
                EffectLayer {
                    layer_index: 2,
                    additive: true,
                },
                Transform::default(),
            ))
            .id();

        app.update();

        let depth = |e: Entity| app.world().get::<Transform>(e).unwrap().translation.z;
        // A failed single() would leave both at the origin; the ordering proves the
        // world camera was resolved despite the preview camera being present.
        assert!(depth(lower) < depth(higher));
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
    fn write_frame_to_mesh_translates_by_offset_minus_origin_and_flips_y() {
        // A zero-sized quad isolates the translation: every corner lands at
        // (offset - EFFECT_ORIGIN) * STR_WORLD_SCALE, with Y flipped (STR is
        // Y-down). This is what scatters a multi-layer effect across its area
        // instead of stacking every layer on the anchor cell.
        let frame = RenderFrame {
            corners: [Vec2::ZERO; 4],
            uvs: [Vec2::ZERO; 4],
            color: [1.0; 4],
            angle_radians: 0.0,
            texture_index: 0,
            offset: EFFECT_ORIGIN + Vec2::new(10.0, 20.0),
        };

        let mut mesh = create_effect_layer_mesh();
        write_frame_to_mesh(&mut mesh, &frame, Color::WHITE);

        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).expect("positions")
        else {
            panic!("expected Float32x3 positions");
        };

        let expected = [10.0 * STR_WORLD_SCALE, -20.0 * STR_WORLD_SCALE, 0.0];
        for position in positions {
            assert!((position[0] - expected[0]).abs() < 1e-4, "x {position:?}");
            assert!((position[1] - expected[1]).abs() < 1e-4, "y {position:?}");
            assert!((position[2] - expected[2]).abs() < 1e-4, "z {position:?}");
        }
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
