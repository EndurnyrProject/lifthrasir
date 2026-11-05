// Unlit material for STR skill-effect billboards: sample the per-frame texture
// and multiply by the interpolated mesh vertex colour (frame colour x tint).
// `uv` / `color` are gated on their vertex-attribute shader-defs so a layer mesh
// missing `ATTRIBUTE_UV_0` / `ATTRIBUTE_COLOR` still compiles and renders
// (degrade, never fail: effects are non-critical).
//
// STR layers are camera-plane quads, so whatever a layer draws below its anchor
// on screen sits under the ground and fails the depth test (a feet-anchored
// ground circle loses its front half). The fragment stage gives those pixels
// the depth of the ground plane instead, so flat ground no longer hides them
// while props and units in front still do. Pixels above the plane keep their
// own depth. Screen shape and UVs are untouched: only depth changes.
import bevy_pbr::render::{
    forward_io::{Vertex, decompress_vertex},
    mesh_functions::{get_world_from_local, mesh_position_local_to_world},
    view_transformations::{
        frag_coord_to_ndc, position_ndc_to_world, position_world_to_clip, position_world_to_ndc,
    },
};

// World units the ground plane is lifted above the layer origin, so the lower
// part of an effect stays clear of flat ground instead of z-fighting it.
const GROUND_LIFT: f32 = 1.0;

struct EffectVertexOutput {
    @builtin(position) position: vec4<f32>,
    // Height of the ground plane this layer's lower part is projected onto.
    @location(0) @interpolate(flat) ground_y: f32,
    @if(VERTEX_UVS_A)
    @location(1) uv: vec2<f32>,
    @if(VERTEX_COLORS)
    @location(2) color: vec4<f32>,
}

struct EffectFragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var base_color_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_color_sampler: sampler;

@vertex
fn vertex(vertex_in: Vertex) -> EffectVertexOutput {
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = get_world_from_local(vertex_in.instance_index);
    let world = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    var out: EffectVertexOutput;
    out.position = position_world_to_clip(world.xyz);
    out.ground_y = world_from_local[3].y + GROUND_LIFT;
    @if(VERTEX_UVS_A)
    out.uv = vertex.uv;
    @if(VERTEX_COLORS)
    out.color = vertex.color;
    return out;
}

// NDC depth of this fragment, or of the point where its view ray crosses the
// plane `y = ground_y` when the fragment lies below that plane.
fn ground_projected_depth(frag_coord: vec4<f32>, ground_y: f32) -> f32 {
    let ndc = frag_coord_to_ndc(frag_coord);
    let world = position_ndc_to_world(ndc);
    if world.y >= ground_y {
        return ndc.z;
    }
    // Reverse-Z: NDC depth 1 is the near plane, for both projections.
    let toward_camera = position_ndc_to_world(vec3(ndc.xy, 1.0)) - world;
    if toward_camera.y <= 0.0 {
        return ndc.z;
    }
    let t = min((ground_y - world.y) / toward_camera.y, 1.0);
    return position_world_to_ndc(world + toward_camera * t).z;
}

@fragment
fn fragment(in: EffectVertexOutput) -> EffectFragmentOutput {
    @if(VERTEX_UVS_A)
    var color = textureSample(base_color_texture, base_color_sampler, in.uv);
    @else
    var color = vec4<f32>(1.0);
    // RO color-key: some effect BMPs use magenta (1, 0, 1) as the transparent
    // background. Key it out. Pure black is keyed too, as the reference client's
    // STR shader discards it: additive layers lose nothing, and opaque layers on
    // black-backed BMPs (shield_charge.str's `shield.bmp`) drop the square.
    let is_magenta = color.r > 0.9 && color.g < 0.1 && color.b > 0.9;
    let is_black = max(color.r, max(color.g, color.b)) < 0.004;
    if (is_magenta || is_black) {
        color.a = 0.0;
    }
    @if(VERTEX_COLORS)
    color = color * in.color;

    // Bevy's Add/Premultiplied/Multiply alpha modes share a premultiplied blend
    // state and rely on the fragment shader to premultiply (and, for additive,
    // zero the alpha), mirroring bevy_pbr's `premultiply_alpha`. A custom
    // material shader must do the same. Otherwise the premultiplied blend state
    // degenerates to "replace", painting each layer's opaque black background
    // over the scene (the dark shards). Effects only ever request AlphaMode::Add
    // on the premultiplied pass, so emit additive output there.
    var out: EffectFragmentOutput;
    out.color = color;
    @if(BLEND_PREMULTIPLIED_ALPHA)
    out.color = vec4<f32>(color.rgb * color.a, 0.0);
    @if(BLEND_MULTIPLY)
    out.color = vec4<f32>(color.rgb * color.a, color.a);
    out.depth = ground_projected_depth(in.position, in.ground_y);
    return out;
}
