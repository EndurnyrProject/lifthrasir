import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Hallucination {
    time: f32,
}
@group(0) @binding(2) var<uniform> hallucination: Hallucination;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let t = hallucination.time;
    let offset = vec2<f32>(
        sin(in.uv.y * 18.0 + t * 2.0) * 0.012,
        sin(in.uv.x * 14.0 + t * 1.5) * 0.008,
    );
    return textureSample(screen_texture, screen_sampler, clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0)));
}
