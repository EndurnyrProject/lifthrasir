// Generic skill-fx uber-shader. One additive billboard whose fragment stage
// switches on `params.kind` to a per-skill function, so a new bolt effect is a
// fragment function plus a `shader_fx.ron` entry, zero Rust. The `factor` uniform
// (0->1 over the effect, driven by the ECS FactorRamp) shapes the envelopes;
// crackle/flicker jitter comes from the view globals clock. Unlit, additive
// blend, camera-facing built in the vertex stage.
//
// Per-skill fragment functions are folded into this single file (the pre-approved
// fallback): cross-file `ro://` shader imports are unproven in this codebase, and
// this keeps the whole dispatch in one place. `shape: vec4<f32>` carries per-kind
// scalars whose meaning is documented in each fragment function's header.
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::{view, globals}

const TAU: f32 = 6.28318530718;

// Field order must match `SkillFxParams` in skill_fx.rs exactly.
struct SkillFxParams {
    kind: u32,
    primary: vec4<f32>,
    secondary: vec4<f32>,
    shape: vec4<f32>,
    factor: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: SkillFxParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let model = get_world_from_local(v.instance_index);
    let origin = model[3].xyz;
    let scale_x = length(model[0].xyz);
    let scale_y = length(model[1].xyz);
    let cam_right = view.world_from_view[0].xyz;
    let cam_up = view.world_from_view[1].xyz;
    let world_pos = origin + cam_right * (v.position.x * scale_x) + cam_up * (v.position.y * scale_y);

    var out: VertexOutput;
    out.clip_position = view.clip_from_world * vec4<f32>(world_pos, 1.0);
    out.uv = v.uv;
    return out;
}

fn hash11(p: f32) -> f32 {
    return fract(sin(p * 127.1) * 43758.5453);
}

fn hash12(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let fr = fract(p);
    let u = fr * fr * (3.0 - 2.0 * fr);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// kind 0 — Jupitel Thunder detonation. shape: x=emission y=bolt_count z=crackle_hz.
// Flickering white-hot core, jagged radial bolts re-seeded crackle_hz times per
// second, and a thin expanding shock ring. Ported verbatim from the standalone
// jupitel_thunder.wgsl.
fn jupitel_fragment(uv: vec2<f32>) -> vec4<f32> {
    let centered = uv * 2.0 - 1.0;
    let r = length(centered);
    let theta = atan2(centered.y, centered.x);
    let f = material.factor;
    let emission = material.shape.x;
    let bolt_count = i32(material.shape.y);
    let crackle_hz = material.shape.z;

    let reseed = floor(globals.time * crackle_hz);

    let appear = smoothstep(0.0, 0.04, f);
    let fade = 1.0 - smoothstep(0.65, 1.0, f);

    let flick = 0.85 + 0.3 * hash11(reseed * 3.7);
    let core_env = appear * fade * mix(1.7, 0.55, smoothstep(0.0, 0.35, f));
    let core = exp(-r * r * 28.0) * flick * core_env;
    let halo = exp(-r * r * 5.0) * 0.35 * appear * fade;

    var sharp = 0.0;
    var glow = 0.0;
    for (var i = 0; i < bolt_count; i++) {
        let seed = f32(i) * 19.7 + reseed * 7.31;
        let ang0 = hash11(seed) * TAU;
        let len = 0.5 + hash11(seed + 3.1) * 0.45;
        let wiggle = (vnoise(vec2<f32>(r * 9.0, seed)) - 0.5) * 1.6 * smoothstep(0.0, 0.6, r);
        var d = theta - ang0 + wiggle;
        d = atan2(sin(d), cos(d));
        let dist = abs(d) * r;
        let width = 0.014 + 0.02 * r / len;
        let along = (1.0 - smoothstep(len * 0.55, len, r)) * smoothstep(0.02, 0.09, r);
        sharp += exp(-pow(dist / width, 2.0)) * along;
        glow += exp(-pow(dist / (width * 5.0), 2.0)) * along;
    }
    let bolt_env = appear * fade;
    sharp = min(sharp, 2.0) * bolt_env;
    glow = min(glow, 2.0) * 0.28 * bolt_env;

    let prog = smoothstep(0.05, 0.9, f);
    let ring_mod = 0.75 + 0.5 * vnoise(vec2<f32>(theta * 2.0 + 3.0, reseed * 0.13));
    let ring = exp(-pow((r - (0.12 + prog * 0.8)) / 0.035, 2.0))
        * (1.0 - prog) * smoothstep(0.0, 0.08, f) * ring_mod;

    let hot = core + sharp;
    let cool = halo + glow + ring * 1.3;
    let alpha = clamp(hot + cool, 0.0, 1.0);
    if (alpha < 0.01) {
        discard;
    }
    let color = (material.primary.rgb * hot + material.secondary.rgb * cool) * emission;
    return vec4<f32>(color, alpha);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    switch material.kind {
        case 0u: {
            return jupitel_fragment(in.uv);
        }
        default: {
            return vec4<f32>(1.0, 0.0, 1.0, 1.0);
        }
    }
}
