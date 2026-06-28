// Portal surface ported from BinbunVFX's portal.gdshader. A layered polar swirl:
// each depth layer samples a seamless cellular-fractal noise texture in polar
// (angle, radius) space, scrolling inward and spinning over time, accumulated
// into the brightest strand. Time comes from the view globals; shape, motion
// and colour are data-driven via PortalParams. Dropped from the original:
// view-parallax, screen-warp refraction and the stencil pass (RO's camera angle
// is fixed, so the per-layer depth scroll carries the tunnel look on its own).
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct PortalParams {
    primary_color: vec4<f32>,
    secondary_color: vec4<f32>,
    shape: vec4<f32>,   // x=open_amount y=density z=edge_softness w=emission_strength
    depth: vec4<f32>,   // x=depth_amount y=shrink_amount z=fade_amount w=layers
    motion: vec4<f32>,  // x=speed_scale y=spin z=inward w=base_motion
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: PortalParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var noise_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var noise_sampler: sampler;

const PI: f32 = 3.14159265;
const SHAPE_WARP: f32 = 0.1;

fn to_polar(uv: vec2<f32>) -> vec2<f32> {
    let radius = length(uv);
    let angle = atan2(uv.x, uv.y);
    return vec2<f32>(angle / 2.0, radius) / PI;
}

fn blend_overlay(a: f32, b: f32) -> f32 {
    let limit = step(0.5, a);
    return mix(2.0 * a * b, 1.0 - 2.0 * (1.0 - a) * (1.0 - b), limit);
}

// Radial gradient: 0 at centre, 1 at the rim (the source's GradientTexture2D
// radial fill), narrowing with depth so deeper layers read as a tighter tunnel.
fn sample_shape(uv: vec2<f32>, depth: f32) -> f32 {
    let dist = clamp(length(uv - vec2<f32>(0.5)) * 2.0, 0.0, 1.0);
    let open = material.shape.x;
    let shrink = material.depth.y;
    return clamp(dist / max(open - depth * shrink, 0.001), 0.0, 1.0);
}

// Seamless noise sampled in polar space so the swirl wraps around the ring;
// `depth` scrolls the radius, time spins the angle and drifts inward.
fn sample_noise(uv: vec2<f32>, depth: f32, scale: f32) -> f32 {
    var p = uv * (2.0 * scale) - vec2<f32>(scale);
    let base = material.motion.w;
    var motion = vec2<f32>(globals.time) * vec2<f32>(depth + base)
        * vec2<f32>(material.motion.y, material.motion.z);
    motion = motion * material.motion.x;
    var pol = to_polar(p) + vec2<f32>(depth, 0.0) + motion;
    pol.x = pol.x * scale;
    return textureSample(noise_tex, noise_sampler, pol).r;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let density = material.shape.y;
    let edge_softness = material.shape.z;
    let emission = material.shape.w;
    let depth_amount = material.depth.x;
    let fade_amount = material.depth.z;
    let layers = max(i32(material.depth.w), 1);

    var value = 0.0;
    for (var i = 0; i < layers; i = i + 1) {
        let t = f32(i) / f32(layers);

        var circle = sample_shape(uv, t);
        circle = clamp((circle - (0.5 - density)) / (1.5 - density), 0.0, 1.0);

        let noise_mult = sample_noise(uv, t, 2.0);
        let n = sample_noise(uv * (1.0 - noise_mult * SHAPE_WARP), t, 1.0);

        var shaped = blend_overlay(circle, n);
        let edge_min = mix(0.4, 0.8, 1.0 - edge_softness);
        let edge_max = mix(1.0, 0.8, 1.0 - edge_softness);
        shaped = smoothstep(edge_min, edge_max, shaped);

        let fade = pow(1.0 - t, fade_amount);
        value = max(value, shaped * fade);
    }

    let circle_value = sample_shape(uv, 0.0);
    let color = mix(material.secondary_color.rgb, material.primary_color.rgb, value);

    var alpha = smoothstep(0.0, 0.5, 1.0 - circle_value);
    alpha = step(0.2, alpha);

    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)) * emission, alpha);
}
