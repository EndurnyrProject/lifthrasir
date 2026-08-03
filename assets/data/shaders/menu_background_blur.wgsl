#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var background_image: texture_2d<f32>;
@group(1) @binding(1) var background_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(background_image));
    let half_step = texel * 8.0;
    let full_step = texel * 16.0;

    var color = textureSample(background_image, background_sampler, in.uv) * 0.20;

    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(half_step.x, 0.0)) * 0.10;
    color += textureSample(background_image, background_sampler, in.uv - vec2<f32>(half_step.x, 0.0)) * 0.10;
    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(0.0, half_step.y)) * 0.10;
    color += textureSample(background_image, background_sampler, in.uv - vec2<f32>(0.0, half_step.y)) * 0.10;

    color += textureSample(background_image, background_sampler, in.uv + half_step) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv - half_step) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(half_step.x, -half_step.y)) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(-half_step.x, half_step.y)) * 0.05;

    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(full_step.x, 0.0)) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv - vec2<f32>(full_step.x, 0.0)) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv + vec2<f32>(0.0, full_step.y)) * 0.05;
    color += textureSample(background_image, background_sampler, in.uv - vec2<f32>(0.0, full_step.y)) * 0.05;

    return color;
}
