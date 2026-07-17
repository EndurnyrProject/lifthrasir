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
// Optional classic GRF effect texture (SkillFxMaterial binding 1/2). Declared
// for the bind-group layout; fragments start sampling it in a later task. When
// the material carries no texture, Bevy binds the fallback image here.
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var fx_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var fx_sampler: sampler;

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

// Scalar intensity of the bound classic GRF effect texture. Black-background
// BMPs are additive-safe, so the brightest channel reads as glow. uv is clamped
// so out-of-range lookups sample the edge; callers still mask contribution to
// zero outside [0,1] to avoid edge smear. Level 0 (no derivatives) is required
// inside the non-uniform per-streak loops.
fn sample_fx(uv: vec2<f32>) -> f32 {
    let t = textureSampleLevel(fx_texture, fx_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    return max(max(t.r, t.g), t.b);
}

// kind 0 — Jupitel Thunder detonation. shape: x=emission y=bolt_count z=crackle_hz.
// Flickering white-hot core, jagged radial bolts re-seeded crackle_hz times per
// second, and a thin expanding shock ring. The classic thunder_pang starburst is
// sampled centered (uv direct) and added under the procedural bolts/ring, faded
// out early by `f` so the detonation flash reads first.
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

    let pang = sample_fx(uv) * appear * (1.0 - smoothstep(0.1, 0.7, f));

    let hot = core + sharp;
    let cool = halo + glow + ring * 1.3 + pang * 0.6;
    let alpha = clamp(hot + cool, 0.0, 1.0);
    if (alpha < 0.01) {
        discard;
    }
    let color = (material.primary.rgb * hot + material.secondary.rgb * cool) * emission;
    return vec4<f32>(color, alpha);
}

// kind 1 — Fire Bolt. shape: x=streak_count y=flicker_hz z=impact_bloom.
// A handful of fire_fall_b flame streaks fall from screen-up and strike the
// target (quad center), staggered across `factor` so they land in sequence; each
// landing blooms a brief fire flash (z is the flash falloff — larger = tighter).
// The texture (flame head at its bottom) is mapped per lane: u across the streak
// width around lane+wob, v tracks the falling head so the flame body lands at the
// head. primary tints the sampled core, secondary the orange-red falloff. Ember
// shimmer is scrolling vnoise near the impact, flicker keyed off globals.time.
fn fire_bolt_fragment(uv: vec2<f32>) -> vec4<f32> {
    let centered = uv * 2.0 - 1.0;
    let f = material.factor;
    let count = i32(material.shape.x);
    let flicker_hz = material.shape.y;
    let bloom = material.shape.z;

    let life = 1.0 - smoothstep(0.9, 1.0, f);
    let fcount = max(f32(count), 1.0);

    var hot = 0.0;
    var warm = 0.0;
    var flash = 0.0;
    for (var i = 0; i < count; i++) {
        let seed = f32(i);
        let lane = (hash11(seed * 4.7) - 0.5) * 1.3;
        let t0 = seed / fcount * 0.55;
        let local = f - t0;
        let p = clamp(local / 0.4, 0.0, 1.0);
        let vis = step(0.0, local) * (1.0 - smoothstep(0.4, 0.55, local));
        let head = 1.3 - 1.3 * p;

        let wob = (vnoise(vec2<f32>(centered.y * 5.0 + seed * 11.0, globals.time * flicker_hz)) - 0.5) * 0.12;
        let along = (centered.y - head) / 0.55;
        let u = 0.5 + (centered.x - lane - wob) / 0.5;
        let mask = step(0.0, along) * step(along, 1.0) * step(0.0, u) * step(u, 1.0);
        let ti = sample_fx(vec2<f32>(u, 1.0 - along)) * mask;
        let flick = 0.75 + 0.4 * hash11(floor(globals.time * flicker_hz) + seed);
        hot += ti * vis * flick;
        warm += ti * vis * 0.35;

        let tl = local - 0.4;
        let ri = length(centered - vec2<f32>(lane, 0.0));
        flash += exp(-ri * ri * bloom) * exp(-tl * tl * 55.0) * step(0.0, local);
    }

    let ember = vnoise(vec2<f32>(centered.x * 8.0, centered.y * 8.0 - globals.time * 2.5));
    let ember_glow = smoothstep(0.62, 1.0, ember) * exp(-dot(centered, centered) * 2.2) * 0.25;

    hot = min(hot, 2.0) * life;
    warm = (min(warm, 1.5) + ember_glow) * life;
    flash = min(flash, 1.5) * life;

    let core = hot + flash;
    let glow = warm + flash * 0.5;
    let alpha = clamp(core + glow, 0.0, 1.0);
    if (alpha < 0.01) {
        discard;
    }
    let color = material.primary.rgb * core + material.secondary.rgb * glow;
    return vec4<f32>(color, alpha);
}

// kind 2 — Cold Bolt. shape: x=shard_count y=glint_hz z=impact_bloom.
// The ice twin of fire: a handful of ice_fall_bb shard streaks drop straight
// down (no wobble) and strike the target, staggered across `factor` so they land
// in sequence; each landing blooms a brief frosty flash (z is the flash falloff —
// larger = tighter). The texture (shard head at its bottom) is mapped per lane
// like fire_bolt. primary tints the white-blue HDR core, secondary the deep-blue
// falloff. Glints are hash-gated brightness pops (glint_hz reseeds per period)
// on the sampled streak. A slow-drifting cold mist glow sits at the impact base
// in place of fire's ember shimmer.
fn cold_bolt_fragment(uv: vec2<f32>) -> vec4<f32> {
    let centered = uv * 2.0 - 1.0;
    let f = material.factor;
    let count = i32(material.shape.x);
    let glint_hz = material.shape.y;
    let bloom = material.shape.z;

    let life = 1.0 - smoothstep(0.9, 1.0, f);
    let fcount = max(f32(count), 1.0);

    var hot = 0.0;
    var cold = 0.0;
    var flash = 0.0;
    for (var i = 0; i < count; i++) {
        let seed = f32(i);
        let lane = (hash11(seed * 4.7) - 0.5) * 1.3;
        let t0 = seed / fcount * 0.55;
        let local = f - t0;
        let p = clamp(local / 0.4, 0.0, 1.0);
        let vis = step(0.0, local) * (1.0 - smoothstep(0.4, 0.55, local));
        let head = 1.3 - 1.3 * p;

        let along = (centered.y - head) / 0.55;
        let u = 0.5 + (centered.x - lane) / 0.5;
        let mask = step(0.0, along) * step(along, 1.0) * step(0.0, u) * step(u, 1.0);
        let ti = sample_fx(vec2<f32>(u, 1.0 - along)) * mask;
        let glint_gate = step(0.94, hash11(floor(globals.time * glint_hz) + seed * 5.3));
        let glint = 1.0 + glint_gate * 1.6;
        hot += ti * vis * glint;
        cold += ti * vis * 0.3;

        let tl = local - 0.4;
        let ri = length(centered - vec2<f32>(lane, 0.0));
        flash += exp(-ri * ri * bloom) * exp(-tl * tl * 55.0) * step(0.0, local);
    }

    let mist = vnoise(vec2<f32>(centered.x * 3.0 + globals.time * 0.4, centered.y * 3.0));
    let mist_glow = smoothstep(0.4, 0.9, mist) * exp(-dot(centered, centered) * 2.5) * 0.22;

    hot = min(hot, 2.0) * life;
    cold = (min(cold, 1.5) + mist_glow) * life;
    flash = min(flash, 1.5) * life;

    let core = hot + flash;
    let glow = cold + flash * 0.5;
    let alpha = clamp(core + glow, 0.0, 1.0);
    if (alpha < 0.01) {
        discard;
    }
    let color = material.primary.rgb * core + material.secondary.rgb * glow;
    return vec4<f32>(color, alpha);
}

// kind 3 — Lightning Bolt. shape: x=restrike_count y=crackle_hz z=fork_count.
// The classic lightning.bmp (full-height vertical bolt) is mapped into the upper
// half of the quad — u across the bolt width (nudged per restrike so the strike
// path shifts), v from the target at quad center up to the top — reseeding
// restrike_count times across `factor` for a fast-attack/decay double-or-triple
// strike. The texture's own jaggedness carries the wander; each restrike still
// throws fork_count procedural branches off the main path. primary tints the
// white-violet core, secondary the blue-violet falloff; an impact flash pulses
// on every restrike, afterglow dims through the tail.
fn lightning_bolt_fragment(uv: vec2<f32>) -> vec4<f32> {
    let centered = uv * 2.0 - 1.0;
    let f = material.factor;
    let restrike_count = i32(material.shape.x);
    let crackle_hz = material.shape.y;
    let fork_count = i32(material.shape.z);
    let fcount = max(f32(restrike_count), 1.0);

    let y = clamp(centered.y, 0.0, 1.0);
    let above = step(0.0, centered.y);

    var sharp = 0.0;
    var glow = 0.0;
    var flash = 0.0;
    for (var i = 0; i < restrike_count; i++) {
        let seed = f32(i) * 13.7;
        let t0 = f32(i) / fcount * 0.55;
        let local = f - t0;
        let attack = smoothstep(0.0, 0.025, local);
        let decay = exp(-max(local, 0.0) * 9.0);
        let env = attack * decay;
        let reseed = floor(globals.time * crackle_hz) + seed;
        let crackle = 0.8 + 0.5 * hash11(reseed * 5.3);

        let width = 0.018 + 0.02 * y;
        let u_off = (hash11(reseed * 2.3) - 0.5) * 0.3;
        let u = 0.5 + (centered.x + u_off) / 0.8;
        let mask = step(0.0, u) * step(u, 1.0) * above;
        let ti = sample_fx(vec2<f32>(u, 1.0 - y)) * mask;
        sharp += ti * env * crackle;
        glow += ti * env * 0.3;

        for (var k = 0; k < fork_count; k++) {
            let fseed = seed + f32(k) * 7.3;
            let hbranch = 0.3 + 0.5 * hash11(fseed);
            let flen = 0.2 + 0.2 * hash11(fseed + 1.3);
            let fdir = sign(hash11(fseed + 2.1) - 0.5);
            let fx = (vnoise(vec2<f32>(hbranch * 4.0, reseed)) - 0.5) * 0.55 * hbranch
                + fdir * clamp((hbranch - y) / flen, 0.0, 1.0) * 0.35;
            let fvis = step(y, hbranch) * step(hbranch - flen, y);
            let fdist = abs(centered.x - fx);
            sharp += exp(-pow(fdist / (width * 0.5), 2.0)) * env * crackle * fvis * 0.6 * above;
            glow += exp(-pow(fdist / (width * 2.5), 2.0)) * env * 0.2 * fvis * above;
        }

        flash += exp(-dot(centered, centered) * 26.0) * attack * decay;
    }

    let tail = 1.0 - smoothstep(0.5, 1.0, f);
    let afterglow = exp(-abs(centered.x) * 3.0) * exp(-y * 1.5) * tail * 0.15;

    sharp = min(sharp, 2.0);
    glow = min(glow + afterglow, 1.5);
    flash = min(flash, 1.5);

    let core = sharp + flash;
    let cool = glow + flash * 0.5;
    let alpha = clamp(core + cool, 0.0, 1.0);
    if (alpha < 0.01) {
        discard;
    }
    let color = material.primary.rgb * core + material.secondary.rgb * cool;
    return vec4<f32>(color, alpha);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    switch material.kind {
        case 0u: {
            return jupitel_fragment(in.uv);
        }
        case 1u: {
            return fire_bolt_fragment(in.uv);
        }
        case 2u: {
            return cold_bolt_fragment(in.uv);
        }
        case 3u: {
            return lightning_bolt_fragment(in.uv);
        }
        default: {
            return vec4<f32>(1.0, 0.0, 1.0, 1.0);
        }
    }
}
