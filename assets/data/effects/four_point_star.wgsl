// Four-point glint ported from BinbunVFX's four_point_star.gdshader. An additive
// star whose points sharpen and fade as the `factor` uniform rises 0->1 over the
// burst lifetime (Godot drove this via the per-particle COLOR.a decay). Unlit,
// additive blend, camera-facing built in the vertex stage (design D5). Dropped
// from the source: per-instance COLOR and the depth-texture proximity fade.
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::view

struct StarParams {
    primary_color: vec4<f32>,
    secondary_color: vec4<f32>,
    shape: vec4<f32>,   // x=emission y=star_shape z=star_smoothness
    factor: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: StarParams;

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

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = abs(in.uv * 2.0 - 1.0);
    let emission = material.shape.x;
    let star_shape = material.shape.y;
    let star_smoothness = material.shape.z;

    // factor rises 0->1 over the burst; the glint is brightest early and fades.
    let decay = 1.0 - material.factor;
    let exponent = star_shape * decay;
    var value = clamp(pow(max(uv.x, 1e-4), exponent) + pow(max(uv.y, 1e-4), exponent), 0.0, 2.0);
    value = smoothstep(0.95, 0.95 - star_smoothness, value);

    let color = mix(material.secondary_color.rgb, material.primary_color.rgb, value) * emission;
    return vec4<f32>(color, value);
}
