use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use bevy_auto_plugin::prelude::*;
use lifthrasir_data::lif::LifWater;

use crate::{
    domain::{system_sets::WaterRenderingSystems, world::map_scoped::MapScoped},
    utils::constants::CELL_SIZE,
};

#[derive(Component)]
pub struct WaterSurface {
    pub water_level: f32,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub wave_pitch: f32,
    pub animation_speed: f32,
    pub mesh_handle: Handle<Mesh>,
    pub material_handle: Handle<WaterMaterial>,
}

#[derive(Component)]
pub struct WaterAnimation {
    pub time: f32,
    pub uv_offset: Vec2,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterExtension {
    #[uniform(100)]
    pub water_data: WaterData,
    #[texture(101)]
    #[sampler(102)]
    pub water_texture: Handle<Image>,
    #[texture(103)]
    #[sampler(104)]
    pub normal_map: Handle<Image>,
}

#[derive(Debug, Clone, ShaderType)]
pub struct WaterData {
    pub wave_params: Vec4,
    pub animation_params: Vec4,
    pub tile_coords: Vec4,
}

impl MaterialExtension for WaterExtension {
    fn fragment_shader() -> ShaderRef {
        "ro://shaders/water.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "ro://shaders/water.wgsl".into()
    }
}

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>;

impl Default for WaterExtension {
    fn default() -> Self {
        Self {
            water_data: WaterData {
                wave_params: Vec4::new(0.2, 2.0, 50.0, 0.0),
                animation_params: Vec4::ZERO,
                tile_coords: Vec4::ZERO,
            },
            water_texture: Handle::default(),
            normal_map: Handle::default(),
        }
    }
}

/// Temporary component to track every water zone texture loading state.
#[derive(Component)]
pub struct WaterLoadingState {
    pub(crate) zones: Vec<WaterZoneLoadingState>,
}

pub(crate) struct WaterZoneLoadingState {
    pub(crate) texture_handle: Handle<Image>,
    pub(crate) water_tiles: Vec<(usize, usize)>,
    pub(crate) wave_height: f32,
    pub(crate) water_level: f32,
    pub(crate) wave_height_param: f32,
    pub(crate) wave_speed: f32,
    pub(crate) wave_pitch: f32,
    pub(crate) animation_speed: f32,
}

/// Queues a map's selected water tiles by zone, starts every texture load and
/// leaves a [`WaterLoadingState`] for [`finalize_water_loading_system`].
pub(crate) fn begin_water_loading(
    commands: &mut Commands,
    asset_server: &AssetServer,
    entity: Entity,
    water: &LifWater,
    water_tiles: Vec<(usize, usize)>,
) {
    let zones: Vec<_> = water
        .zones
        .iter()
        .zip(group_water_tiles(water, water_tiles))
        .filter_map(|(zone, water_tiles)| {
            if zone.level == 0.0 || water_tiles.is_empty() {
                return None;
            }

            debug!(
                "Queueing {} water tiles at level {}, type {}",
                water_tiles.len(),
                zone.level,
                zone.water_type
            );
            Some(WaterZoneLoadingState {
                texture_handle: load_water_texture(zone.water_type, 0, asset_server),
                water_tiles,
                wave_height: zone.level - zone.wave_height,
                water_level: zone.level,
                wave_height_param: zone.wave_height,
                wave_speed: zone.wave_speed,
                wave_pitch: zone.wave_pitch,
                animation_speed: zone.anim_speed as f32,
            })
        })
        .collect();

    if zones.is_empty() {
        debug!("No water tiles detected for this map");
        return;
    }

    commands.entity(entity).insert(WaterLoadingState { zones });
}

fn group_water_tiles(
    water: &LifWater,
    water_tiles: Vec<(usize, usize)>,
) -> Vec<Vec<(usize, usize)>> {
    let mut tiles_by_zone = vec![Vec::new(); water.zones.len()];
    for (x, y) in water_tiles {
        if water.zone_at(x, y).level == 0.0 {
            continue;
        }
        tiles_by_zone[water.zone_index_at(x, y)].push((x, y));
    }
    tiles_by_zone
}

// Maximum water parameter values to prevent excessive movement in some maps
const MAX_WAVE_HEIGHT: f32 = 10.0;
const MAX_WAVE_SPEED: f32 = 10.0;
const MAX_WAVE_PITCH: f32 = 100.0;
const MIN_WAVE_PITCH: f32 = 0.1;

// Water mesh subdivision (8x8 = 128 triangles per tile)
const WATER_TILE_SUBDIVISIONS: usize = 8;

/// System to finalize water loading once textures are ready
#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = WaterRenderingSystems::WaterFinalization)
)]
pub fn finalize_water_loading_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &WaterLoadingState)>,
) {
    for (entity, loading_state) in query.iter() {
        if !loading_state
            .zones
            .iter()
            .all(|zone| asset_server.is_loaded_with_dependencies(&zone.texture_handle))
        {
            continue;
        }

        debug!("Water textures loaded, creating zone meshes and materials");
        for zone in &loading_state.zones {
            configure_water_sampler(&mut images, &zone.texture_handle);
        }
        let normal_map = generate_water_normal_map(&mut images);
        for zone in &loading_state.zones {
            spawn_water_zone(
                &mut commands,
                &mut meshes,
                &mut materials,
                zone,
                normal_map.clone(),
            );
        }
        commands.entity(entity).remove::<WaterLoadingState>();
        debug!("Water rendering setup complete");
    }
}

fn configure_water_sampler(images: &mut Assets<Image>, texture_handle: &Handle<Image>) {
    let Some(mut image) = images.get_mut(texture_handle) else {
        warn!("Water texture handle loaded but image data not in Assets<Image>");
        return;
    };

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..Default::default()
    });
}

fn spawn_water_zone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    zone: &WaterZoneLoadingState,
    normal_map: Handle<Image>,
) {
    let mesh_handle = meshes.add(create_water_tiles_mesh(&zone.water_tiles, zone.wave_height));
    let wave_pitch = zone.wave_pitch.clamp(MIN_WAVE_PITCH, MAX_WAVE_PITCH);
    let k = 2.0 * std::f32::consts::PI / wave_pitch;
    let scaled_wave_height = (zone.wave_height_param.min(MAX_WAVE_HEIGHT) * k).min(MAX_WAVE_HEIGHT);
    let material_handle = materials.add(WaterMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.05),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            metallic: 0.0,
            reflectance: 0.9,
            ..default()
        },
        extension: WaterExtension {
            water_data: WaterData {
                wave_params: Vec4::new(
                    scaled_wave_height,
                    zone.wave_speed.min(MAX_WAVE_SPEED),
                    wave_pitch,
                    0.0,
                ),
                animation_params: Vec4::ZERO,
                tile_coords: Vec4::new(0.0, 0.0, 0.125, 0.0),
            },
            water_texture: zone.texture_handle.clone(),
            normal_map,
        },
    });

    commands.spawn((
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(material_handle.clone()),
        Transform::IDENTITY,
        MapScoped,
        WaterSurface {
            water_level: zone.water_level,
            wave_height: zone.wave_height_param,
            wave_speed: zone.wave_speed,
            wave_pitch: zone.wave_pitch,
            animation_speed: zone.animation_speed,
            mesh_handle,
            material_handle,
        },
        WaterAnimation {
            time: 0.0,
            uv_offset: Vec2::ZERO,
        },
    ));
}

#[auto_add_system(
    plugin = crate::presentation::rendering::map_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = WaterRenderingSystems::WaterAnimation)
)]
pub fn animate_water_system(
    time: Res<Time>,
    mut water_query: Query<(&WaterSurface, &mut WaterAnimation)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    for (water_surface, mut water_animation) in water_query.iter_mut() {
        water_animation.time += time.delta_secs();

        let uv_scroll_speed = water_surface.animation_speed * 0.01;
        water_animation.uv_offset +=
            Vec2::new(uv_scroll_speed, uv_scroll_speed * 0.7) * time.delta_secs();

        if let Some(mut material) = materials.get_mut(&water_surface.material_handle) {
            material.extension.water_data.wave_params.w = water_animation.time;
            material.extension.water_data.animation_params = Vec4::new(
                water_animation.uv_offset.x,
                water_animation.uv_offset.y,
                0.0,
                0.0,
            );
        }
    }
}

/// Create a single mesh containing all water tiles
fn create_water_tiles_mesh(water_tiles: &[(usize, usize)], water_y: f32) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut tangents = Vec::new();
    let mut indices = Vec::new();

    let subdivisions = WATER_TILE_SUBDIVISIONS;
    let verts_per_side = subdivisions + 1;
    let step_size = CELL_SIZE / subdivisions as f32;

    // Create subdivided mesh for each tile
    for &(tile_x, tile_y) in water_tiles.iter() {
        let base_vertex = positions.len() as u32;

        let tile_world_x = tile_x as f32 * CELL_SIZE;
        let tile_world_z = tile_y as f32 * CELL_SIZE;

        // Create vertex grid for this tile (5x5 for 4x4 subdivision)
        for row in 0..verts_per_side {
            for col in 0..verts_per_side {
                let x = tile_world_x + col as f32 * step_size;
                let z = tile_world_z + row as f32 * step_size;

                positions.push([x, water_y, z]);
                normals.push([0.0, -1.0, 0.0]);

                // UV coordinates (0-1 within tile, shader calculates world UVs)
                let u = col as f32 / subdivisions as f32;
                let v = row as f32 / subdivisions as f32;
                uvs.push([u, v]);

                // Tangent vector (X-axis aligned, w=1.0 for handedness)
                tangents.push([1.0, 0.0, 0.0, 1.0]);
            }
        }

        // Create indices for quads
        for row in 0..subdivisions {
            for col in 0..subdivisions {
                let i0 = base_vertex + (row * verts_per_side + col) as u32;
                let i1 = i0 + 1;
                let i2 = i0 + verts_per_side as u32;
                let i3 = i2 + 1;

                // Triangle 1: bottom-left, bottom-right, top-right
                indices.push(i0);
                indices.push(i1);
                indices.push(i3);

                // Triangle 2: bottom-left, top-right, top-left
                indices.push(i0);
                indices.push(i3);
                indices.push(i2);
            }
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Load water texture from GRF based on water type and frame
fn load_water_texture(water_type: u32, frame: u32, asset_server: &AssetServer) -> Handle<Image> {
    let texture_path = format!("ro://data\\texture\\워터\\water{water_type}{frame:02}.jpg");

    // Use AssetServer to load the texture directly
    asset_server.load(texture_path)
}

/// Generate a procedural normal map for water surface detail
fn generate_water_normal_map(images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    const SIZE: u32 = 512;
    let mut data = vec![0u8; (SIZE * SIZE * 4) as usize];

    // Generate multiple octaves of noise for realistic water surface
    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = ((y * SIZE + x) * 4) as usize;

            // Create multiple waves at different frequencies
            let fx = x as f32 / SIZE as f32;
            let fy = y as f32 / SIZE as f32;

            // Calculate normal from height gradient
            // Sample neighboring pixels for gradient calculation
            let dx = if x > 0 && x < SIZE - 1 {
                let fx_prev = (x - 1) as f32 / SIZE as f32;
                let fx_next = (x + 1) as f32 / SIZE as f32;

                let h_prev = (fx_prev * 8.0 * std::f32::consts::PI).sin() * 0.5
                    + (fx_prev * 16.0 * std::f32::consts::PI + 1.57).sin() * 0.25
                    + ((fx_prev * 32.0 + fy * 24.0) * std::f32::consts::PI).sin() * 0.125;

                let h_next = (fx_next * 8.0 * std::f32::consts::PI).sin() * 0.5
                    + (fx_next * 16.0 * std::f32::consts::PI + 1.57).sin() * 0.25
                    + ((fx_next * 32.0 + fy * 24.0) * std::f32::consts::PI).sin() * 0.125;

                (h_next - h_prev) * 0.5
            } else {
                0.0
            };

            let dy = if y > 0 && y < SIZE - 1 {
                let fy_prev = (y - 1) as f32 / SIZE as f32;
                let fy_next = (y + 1) as f32 / SIZE as f32;

                let h_prev = (fy_prev * 8.0 * std::f32::consts::PI).cos() * 0.5
                    + (fy_prev * 16.0 * std::f32::consts::PI + 0.78).cos() * 0.25
                    + ((fx * 32.0 + fy_prev * 24.0) * std::f32::consts::PI).sin() * 0.125;

                let h_next = (fy_next * 8.0 * std::f32::consts::PI).cos() * 0.5
                    + (fy_next * 16.0 * std::f32::consts::PI + 0.78).cos() * 0.25
                    + ((fx * 32.0 + fy_next * 24.0) * std::f32::consts::PI).sin() * 0.125;

                (h_next - h_prev) * 0.5
            } else {
                0.0
            };

            // Convert gradient to normal (tangent space)
            let normal = Vec3::new(-dx * 2.0, -dy * 2.0, 1.0).normalize();

            // Encode normal to RGB (0-1 range)
            data[idx] = ((normal.x * 0.5 + 0.5) * 255.0) as u8; // R
            data[idx + 1] = ((normal.y * 0.5 + 0.5) * 255.0) as u8; // G
            data[idx + 2] = ((normal.z * 0.5 + 0.5) * 255.0) as u8; // B
            data[idx + 3] = 255; // A
        }
    }

    let normal_image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );

    images.add(normal_image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifthrasir_data::lif::LifWaterZone;

    #[test]
    fn groups_selected_tiles_by_their_evenly_tiled_zone() {
        let water = LifWater {
            split_width: 2,
            split_height: 1,
            zones: vec![water_zone(5.0), water_zone(8.0)],
            width: 4,
            height: 2,
            buffer_view: 0,
        };

        assert_eq!(
            group_water_tiles(&water, vec![(0, 0), (3, 0), (1, 1), (2, 1)]),
            vec![vec![(0, 0), (1, 1)], vec![(3, 0), (2, 1)]]
        );
    }

    fn water_zone(level: f32) -> LifWaterZone {
        LifWaterZone {
            level,
            water_type: 1,
            wave_height: 0.5,
            wave_speed: 1.0,
            wave_pitch: 20.0,
            anim_speed: 3,
        }
    }
}
