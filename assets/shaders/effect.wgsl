// Unlit material for STR skill-effect billboards: sample the per-frame texture
// and multiply by the interpolated mesh vertex colour (frame colour x tint).
// The default mesh vertex shader feeds `uv` / `color` into `VertexOutput`.
// Both are gated on their vertex-attribute shader-defs so a layer mesh missing
// `ATTRIBUTE_UV_0` / `ATTRIBUTE_COLOR` still compiles and renders (degrade,
// never fail — effects are non-critical).
#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var base_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_color_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS_A
    var color = textureSample(base_color_texture, base_color_sampler, in.uv);
#else
    var color = vec4<f32>(1.0);
#endif
#ifdef VERTEX_COLORS
    color = color * in.color;
#endif
    return color;
}
