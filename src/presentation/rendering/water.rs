use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};

use crate::{
    domain::{
        assets::components::{WaterAnimation, WaterExtension, WaterMaterial, WaterSurface},
        world::{components::MapLoader, map::MapData, map_loader::MapRequestLoader},
    },
    infrastructure::assets::{
        HierarchicalAssetManager,
        converters::decode_image_from_bytes,
        loaders::{RoGroundAsset, RoWorldAsset},
    },
    utils::constants::CELL_SIZE,
};

pub fn load_water_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    world_assets: Res<Assets<RoWorldAsset>>,
    asset_manager: Option<Res<HierarchicalAssetManager>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<
        (Entity, &MapLoader, &MapRequestLoader, &MapData),
        (Without<WaterSurface>, With<MapData>),
    >,
) {
    let Some(ref asset_manager) = asset_manager else {
        return; // No asset manager available yet
    };

    for (entity, map_loader, _map_request, _) in query.iter() {
        let Some(world_handle) = map_loader.world.as_ref() else {
            continue;
        };

        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };

        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let water = &world_asset.world.water;

        if water.level == 0.0 {
            continue;
        }

        info!(
            "Creating water surface at level: {}, wave_height: {}, wave_speed: {}, wave_pitch: {}, anim_speed: {}",
            water.level, water.wave_height, water.wave_speed, water.wave_pitch, water.anim_speed
        );

        // Implement per-tile water detection logic (based on GRF Editor)
        // Scale water level to match terrain height scaling (÷5.0 applied in GND parsing)
        let wave_height = (water.level - water.wave_height) / 5.0;
        let ground = &ground_asset.ground;
        let width = ground.width as usize;
        let height = ground.height as usize;
        let mut water_tiles = Vec::new();

        info!(
            "Water detection: water.level={}, water.wave_height={}, calculated wave_height={}",
            water.level, water.wave_height, wave_height
        );

        // Check each terrain cell for water presence
        for y in 0..height {
            for x in 0..width {
                let surface = &ground.surfaces[y * width + x];

                // Skip if no tile exists (TileUp == -1)
                if surface.tile_up == -1 {
                    continue;
                }

                // Check if any corner height is above wave_height (GRF Editor logic)
                let heights = &surface.height;
                let has_water = heights[0] > wave_height
                    || heights[1] > wave_height
                    || heights[2] > wave_height
                    || heights[3] > wave_height;

                if has_water {
                    water_tiles.push((x, y));
                }
            }
        }

        if water_tiles.is_empty() {
            info!("No water tiles detected for this map");
            continue;
        }

        info!("Detected {} water tiles", water_tiles.len());

        info!("Creating single water mesh for {} tiles", water_tiles.len());

        // Load water texture from hierarchical asset manager, or use fallback
        let water_texture = load_water_texture(water.water_type, 0, asset_manager, &mut images)
            .unwrap_or_else(|| {
                warn!("Water texture loading failed, using fallback white texture");
                // Create a fallback white texture if loading fails
                let fallback_image = Image::new_fill(
                    bevy::render::render_resource::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    &[255, 255, 255, 255], // White pixel
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::render::render_asset::RenderAssetUsages::default(),
                );
                images.add(fallback_image)
            });

        info!("Water texture handle created: {:?}", water_texture);

        // Generate procedural normal map for water
        let normal_map = generate_water_normal_map(&mut images);
        info!("Generated procedural water normal map");

        // Create single mesh containing all water tiles
        let water_mesh = create_water_tiles_mesh(&water_tiles, wave_height);
        let mesh_handle = meshes.add(water_mesh);

        // Calculate texture scale based on map size
        // This determines how many tiles the texture spans
        let texture_scale = 0.125; // Adjust this value to control texture tiling (0.125 = texture repeats every 8 tiles)

        // Create a single material for all water
        let water_extension = WaterExtension {
            water_data: crate::domain::assets::components::WaterData {
                wave_params: Vec4::new(water.wave_height, water.wave_speed, water.wave_pitch, 0.0),
                animation_params: Vec4::ZERO,
                tile_coords: Vec4::new(0.0, 0.0, texture_scale, 0.0),
            },
            water_texture,
            normal_map,
        };

        let water_material = WaterMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.3), // Much more transparent
                alpha_mode: AlphaMode::Blend,                 // Use Blend for proper transparency
                perceptual_roughness: 0.1,
                metallic: 0.0,
                reflectance: 0.9,
                ..default()
            },
            extension: water_extension,
        };

        let material_handle = materials.add(water_material);

        // Spawn single entity with all water
        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material_handle.clone()),
            Transform::IDENTITY,
        ));

        // Add water surface component to the main entity
        commands.entity(entity).insert((
            WaterSurface {
                water_level: water.level,
                wave_height: water.wave_height,
                wave_speed: water.wave_speed,
                wave_pitch: water.wave_pitch,
                animation_speed: water.anim_speed as f32,
                mesh_handle,
                material_handle,
            },
            WaterAnimation {
                time: 0.0,
                uv_offset: Vec2::ZERO,
            },
        ));
    }
}

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

        if let Some(material) = materials.get_mut(&water_surface.material_handle) {
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
    let mut indices = Vec::new();

    // Create 6 vertices per tile (2 triangles, no sharing)
    for &(x, y) in water_tiles.iter() {
        let vertex_offset = positions.len() as u32;

        // Calculate world position for this tile
        let x_pos = x as f32 * CELL_SIZE;
        let z_pos = y as f32 * CELL_SIZE;
        let x_pos_end = x_pos + CELL_SIZE;
        let z_pos_end = z_pos + CELL_SIZE;

        // Use simple 0-1 UVs for each tile quad
        // The shader will calculate actual UVs from world position

        // Triangle 1: SW -> SE -> NE
        positions.push([x_pos, water_y * 5.0, z_pos]); // SW
        positions.push([x_pos_end, water_y * 5.0, z_pos]); // SE  
        positions.push([x_pos_end, water_y * 5.0, z_pos_end]); // NE

        uvs.push([0.0, 0.0]); // SW
        uvs.push([1.0, 0.0]); // SE
        uvs.push([1.0, 1.0]); // NE

        // Triangle 2: SW -> NE -> NW
        positions.push([x_pos, water_y * 5.0, z_pos]); // SW
        positions.push([x_pos_end, water_y * 5.0, z_pos_end]); // NE
        positions.push([x_pos, water_y * 5.0, z_pos_end]); // NW

        uvs.push([0.0, 0.0]); // SW
        uvs.push([1.0, 1.0]); // NE
        uvs.push([0.0, 1.0]); // NW

        // Add normals (pointing down for RO water style)
        for _ in 0..6 {
            normals.push([0.0, -1.0, 0.0]);
        }

        // Add indices for these two triangles
        indices.push(vertex_offset);
        indices.push(vertex_offset + 1);
        indices.push(vertex_offset + 2);

        indices.push(vertex_offset + 3);
        indices.push(vertex_offset + 4);
        indices.push(vertex_offset + 5);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Load water texture from GRF based on water type and frame
fn load_water_texture(
    water_type: u32,
    frame: u32,
    asset_manager: &HierarchicalAssetManager,
    images: &mut ResMut<Assets<Image>>,
) -> Option<Handle<Image>> {
    let texture_path = format!("data\\texture\\워터\\water{water_type}{frame:02}.jpg");

    if let Ok(texture_data) = asset_manager.load(&texture_path) {
        match decode_image_from_bytes(&texture_data, &texture_path) {
            Ok(image) => {
                return Some(images.add(image));
            }
            Err(e) => {
                warn!("Failed to decode water texture {}: {}", texture_path, e);
            }
        }
    } else {
        warn!("Water texture not found in GRF: {}", texture_path);
    }

    None
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

            // Primary wave pattern
            let wave1 = (fx * 8.0 * std::f32::consts::PI).sin() * 0.5
                + (fy * 8.0 * std::f32::consts::PI).cos() * 0.5;

            // Secondary wave pattern (higher frequency, lower amplitude)
            let wave2 = (fx * 16.0 * std::f32::consts::PI + 1.57).sin() * 0.25
                + (fy * 16.0 * std::f32::consts::PI + 0.78).cos() * 0.25;

            // Tertiary ripples
            let wave3 = ((fx * 32.0 + fy * 24.0) * std::f32::consts::PI).sin() * 0.125;

            // Combine waves
            let height = wave1 + wave2 + wave3;

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
        bevy::render::render_asset::RenderAssetUsages::default(),
    );

    images.add(normal_image)
}
