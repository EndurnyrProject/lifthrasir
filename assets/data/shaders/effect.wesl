// Unlit material for STR skill-effect billboards: sample the per-frame texture
// and multiply by the interpolated mesh vertex colour (frame colour x tint).
// The default mesh vertex shader feeds `uv` / `color` into `VertexOutput`.
// Both are gated on their vertex-attribute shader-defs so a layer mesh missing
// `ATTRIBUTE_UV_0` / `ATTRIBUTE_COLOR` still compiles and renders (degrade,
// never fail — effects are non-critical).
import bevy_pbr::render::forward_io::VertexOutput;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var base_color_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var base_color_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
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
    var result = color;
    @if(BLEND_PREMULTIPLIED_ALPHA)
    result = vec4<f32>(color.rgb * color.a, 0.0);
    @if(BLEND_MULTIPLY)
    result = vec4<f32>(color.rgb * color.a, color.a);
    return result;
}
