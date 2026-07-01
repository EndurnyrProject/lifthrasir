// Radial hit flash ported from BinbunVFX's impact_core.gdshader. A centred UV
// grows into a streaked ring driven by the `factor` uniform (0->1 over the burst
// lifetime, the ECS equivalent of the Godot AnimationPlayer ramping grow_factor).
// Unlit, alpha-blended, camera-facing built in the vertex stage (design D5).
// Dropped from the source: TIME-based streak drift, per-instance COLOR, the
// variance/clearance knobs and the depth-texture proximity fade.
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::view

struct ImpactParams {
    primary_color: vec4<f32>,
    secondary_color: vec4<f32>,
    shape: vec4<f32>,   // x=emission y=streak_amount z=edge_hardness w=edge_position
    factor: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: ImpactParams;

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
    let centered = in.uv * 2.0 - 1.0;
    var radius = length(centered);
    let angle = atan2(centered.x, centered.y);

    let emission = material.shape.x;
    let streak = material.shape.y;
    let edge_hardness = material.shape.z;
    let edge_position = material.shape.w;

    let grow = material.factor * 4.0 - 1.0;

    let offset = abs(sin(angle * (streak * 0.5))) * radius;
    radius = radius + offset * 0.5;
    var value = 1.0 - abs(radius * 2.0 - grow);
    value = value * pow(max(1.0 - radius, 0.0), 0.1);

    let l_edge = mix(0.0, edge_position, edge_hardness);
    let r_edge = mix(1.0, edge_position + 0.01, edge_hardness);
    value = smoothstep(l_edge, r_edge, value);

    if (value < 0.01) {
        discard;
    }
    let color = mix(material.secondary_color.rgb, material.primary_color.rgb, value) * emission;
    return vec4<f32>(color, value);
}
