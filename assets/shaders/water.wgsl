#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_functions::{get_world_from_local, mesh_position_local_to_clip},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

// Smaller values = more texture repetition
const TEXTURE_WORLD_SCALE: f32 = 100.0;

// Gerstner wave function - creates realistic ocean waves with sharp crests and wide troughs
fn gerstner_wave(
    position: vec2<f32>,
    direction: vec2<f32>,
    wavelength: f32,
    amplitude: f32,
    speed: f32,
    time: f32
) -> vec3<f32> {
    let k = 2.0 * 3.14159265359 / wavelength; // Wave number
    let c = sqrt(9.8 / k); // Phase speed (deep water approximation)
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * speed * time);
    let a = amplitude / k; // Steepness control

    return vec3<f32>(
        d.x * a * cos(f),
        a * sin(f),
        d.y * a * cos(f)
    );
}

// Calculate normal from Gerstner wave derivatives
fn gerstner_normal(
    position: vec2<f32>,
    direction: vec2<f32>,
    wavelength: f32,
    amplitude: f32,
    speed: f32,
    time: f32
) -> vec3<f32> {
    let k = 2.0 * 3.14159265359 / wavelength;
    let c = sqrt(9.8 / k);
    let d = normalize(direction);
    let f = k * (dot(d, position) - c * speed * time);
    let a = amplitude / k;

    // Partial derivatives
    let wa = k * a;
    let s = sin(f);
    let cos_f = cos(f);

    // Tangent vectors
    let tangent_x = vec3<f32>(
        1.0 - wa * d.x * d.x * s,
        wa * d.x * cos_f,
        -wa * d.x * d.y * s
    );

    let tangent_z = vec3<f32>(
        -wa * d.x * d.y * s,
        wa * d.y * cos_f,
        1.0 - wa * d.y * d.y * s
    );

    return normalize(cross(tangent_z, tangent_x));
}

struct WaterData {
    wave_params: vec4<f32>,
    animation_params: vec4<f32>,
    tile_coords: vec4<f32>, // xy = tile position, zw = texture scale
};

@group(2) @binding(100)
var<uniform> water: WaterData;
@group(2) @binding(101)
var water_texture: texture_2d<f32>;
@group(2) @binding(102)
var water_sampler: sampler;
@group(2) @binding(103)
var normal_map: texture_2d<f32>;
@group(2) @binding(104)
var normal_sampler: sampler;


@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    // Get world position
    let world_from_local = get_world_from_local(vertex.instance_index);
    let world_position = world_from_local * vec4<f32>(vertex.position, 1.0);

    // Wave parameters
    let wave_height = water.wave_params.x;
    let wave_speed = water.wave_params.y;
    let wave_pitch = water.wave_params.z;
    let time = water.wave_params.w;

    // Apply multiple Gerstner waves for complex ocean surface
    // Each wave has: direction, wavelength, amplitude, speed multiplier
    var displacement = vec3<f32>(0.0);
    var normal_sum = vec3<f32>(0.0, 1.0, 0.0);

    // Wave 1: Primary swell (large, more intense)
    let wave1 = gerstner_wave(
        world_position.xz,
        vec2<f32>(1.0, 0.3),
        wave_pitch * 1.5,
        wave_height * 0.6,
        wave_speed * 0.8,
        time
    );
    displacement += wave1;
    normal_sum += gerstner_normal(
        world_position.xz,
        vec2<f32>(1.0, 0.3),
        wave_pitch * 1.5,
        wave_height * 0.6,
        wave_speed * 0.8,
        time
    ) - vec3<f32>(0.0, 1.0, 0.0);

    // Wave 2: Secondary swell (medium, more dynamic)
    let wave2 = gerstner_wave(
        world_position.xz,
        vec2<f32>(-0.7, 1.0),
        wave_pitch * 1.0,
        wave_height * 0.5,
        wave_speed * 1.2,
        time
    );
    displacement += wave2;
    normal_sum += gerstner_normal(
        world_position.xz,
        vec2<f32>(-0.7, 1.0),
        wave_pitch * 1.0,
        wave_height * 0.5,
        wave_speed * 1.2,
        time
    ) - vec3<f32>(0.0, 1.0, 0.0);

    // Wave 3: Detail waves (choppy, fast)
    let wave3 = gerstner_wave(
        world_position.xz,
        vec2<f32>(0.5, -0.8),
        wave_pitch * 0.4,
        wave_height * 0.35,
        wave_speed * 1.8,
        time
    );
    displacement += wave3;
    normal_sum += gerstner_normal(
        world_position.xz,
        vec2<f32>(0.5, -0.8),
        wave_pitch * 0.4,
        wave_height * 0.35,
        wave_speed * 1.8,
        time
    ) - vec3<f32>(0.0, 1.0, 0.0);

    // Wave 4: Turbulent ripples (rapid, chaotic)
    let wave4 = gerstner_wave(
        world_position.xz,
        vec2<f32>(-0.3, -0.6),
        wave_pitch * 0.25,
        wave_height * 0.25,
        wave_speed * 2.5,
        time
    );
    displacement += wave4;
    normal_sum += gerstner_normal(
        world_position.xz,
        vec2<f32>(-0.3, -0.6),
        wave_pitch * 0.25,
        wave_height * 0.25,
        wave_speed * 2.5,
        time
    ) - vec3<f32>(0.0, 1.0, 0.0);

    // Apply Gerstner wave displacement
    let displaced_position = vertex.position + displacement;

    out.position = mesh_position_local_to_clip(
        world_from_local,
        vec4<f32>(displaced_position, 1.0),
    );

    out.world_position = world_from_local * vec4<f32>(displaced_position, 1.0);

    // Use the Gerstner wave normal (normalized sum of all wave normals)
    let wave_normal = normalize(normal_sum);
    let final_wave_normal = vec3<f32>(wave_normal.x, -wave_normal.y, wave_normal.z);
    out.world_normal = normalize((world_from_local * vec4<f32>(final_wave_normal, 0.0)).xyz);

    // Calculate tangent and bitangent for normal mapping
    // For a horizontal water plane, tangent aligns with X-axis, bitangent with Z-axis
    let tangent = vec3<f32>(1.0, 0.0, 0.0);
    let bitangent = vec3<f32>(0.0, 0.0, 1.0);

    #ifdef VERTEX_TANGENTS
    // Store tangent in the existing tangent output
    out.world_tangent = vec4<f32>(
        normalize((world_from_local * vec4<f32>(tangent, 0.0)).xyz),
        1.0
    );
    #endif

    // Calculate UVs from world position for seamless tiling
    // This creates a continuous UV space across all water tiles
    let base_uv = world_position.xz / TEXTURE_WORLD_SCALE;

    // Apply animation offset
    out.uv = base_uv + water.animation_params.xy;

    #ifdef VERTEX_TANGENTS
    out.world_tangent = world_from_local * vec4<f32>(vertex.tangent.xyz, vertex.tangent.w);
    #endif

    return out;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Sample at two different scales and speeds for variety
    let time = water.wave_params.w;
    let normal_scale1 = 0.08;  // Larger scale for bigger ripples
    let normal_scale2 = 0.04;  // Medium scale

    // First normal sample - larger scale, faster movement
    let uv1 = in.uv * normal_scale1 + vec2<f32>(time * 0.025, time * 0.018);
    var normal_sample1 = textureSample(normal_map, normal_sampler, uv1).xyz;

    // Second normal sample - smaller scale, rapid movement, different direction
    let uv2 = in.uv * normal_scale2 + vec2<f32>(-time * 0.032, time * 0.027);
    var normal_sample2 = textureSample(normal_map, normal_sampler, uv2).xyz;

    // Decode normals from [0,1] to [-1,1] range
    normal_sample1 = normal_sample1 * 2.0 - 1.0;
    normal_sample2 = normal_sample2 * 2.0 - 1.0;

    // Blend the two normal samples for more complex surface detail
    var combined_normal = normalize(normal_sample1 + normal_sample2 * 0.5);

    // Construct TBN matrix for transforming normal from tangent to world space
    let N = normalize(in.world_normal.xyz);

    #ifdef VERTEX_TANGENTS
        let T = normalize(in.world_tangent.xyz);
        let B = normalize(cross(N, T) * in.world_tangent.w);
    #else
        // Fallback if tangents aren't available
        let T = normalize(vec3<f32>(1.0, 0.0, 0.0));
        let B = normalize(cross(N, T));
    #endif

    // Transform the normal from tangent space to world space
    let final_normal = normalize(
        T * combined_normal.x +
        B * combined_normal.y +
        N * combined_normal.z
    );

    // Sample water texture using the UV coordinates
    var texture_color = textureSample(water_texture, water_sampler, in.uv);

    // Mix the texture with a water tint (very low alpha for high transparency)
    let water_tint = vec4<f32>(0.7, 0.85, 1.0, 0.3);
    var base_color = texture_color * water_tint;

    // Enhanced fresnel effect using the mapped normal
    let view_dir = normalize(in.world_position.xyz);
    let fresnel = pow(1.0 - max(dot(-view_dir, final_normal), 0.0), 2.0);

    // Adjust alpha based on fresnel for transparency variation
    // Very low alpha values for high transparency
    base_color.a = mix(0.15, 0.35, fresnel);

    // Add specular highlights based on the detailed normal
    let light_dir = normalize(vec3<f32>(0.5, -1.0, 0.3)); // Simple directional light
    let half_dir = normalize(light_dir - view_dir);
    let specular = pow(max(dot(final_normal, half_dir), 0.0), 32.0) * 0.5;

    // Add subtle specular highlights to the color
    base_color = vec4<f32>(base_color.rgb + vec3<f32>(specular), base_color.a);

    var out: FragmentOutput;
    out.color = base_color;
    return out;
}
