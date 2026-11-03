// Reference: roBrowserLegacy PropertyGround.fs and LPEffect.fs.
import bevy_pbr::render::forward_io::VertexOutput;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var field_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var field_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> opacity: f32;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(field_texture, field_sampler, in.uv);
    // The reference's 0.5 sRGB threshold, after our sRGB texture decoding.
    if (color.a == 0.0 || any(color.rgb < vec3<f32>(0.21404114))) {
        discard;
    }
    return vec4<f32>(color.rgb, opacity);
}
