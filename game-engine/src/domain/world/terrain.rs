use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};

use crate::{
    domain::world::{components::MapLoader, map::MapData, map_loader::MapRequestLoader},
    infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset},
    utils::constants::CELL_SIZE,
};

/// Component to track terrain textures that are loading
#[derive(Component)]
pub struct TerrainTexturesLoading {
    texture_handles: Vec<Handle<Image>>,
    texture_names: Vec<String>,
    ground_handle: Handle<RoGroundAsset>,
    altitude_handle: Option<Handle<RoAltitudeAsset>>,
}

/// Create materials from loaded texture handles
/// Only called after textures are confirmed loaded/failed
fn create_terrain_materials_from_loaded_textures(
    ground: &crate::infrastructure::ro_formats::RoGround,
    texture_handles: &[Handle<Image>],
    texture_names: &[String],
    asset_server: &AssetServer,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Vec<Handle<StandardMaterial>> {
    use bevy::asset::LoadState;
    let mut texture_materials = Vec::new();

    for (i, texture_name) in ground.textures.iter().enumerate() {
        let material = if i < texture_handles.len() && texture_handles[i].id() != AssetId::default()
        {
            // Check if texture actually loaded successfully
            match asset_server.load_state(&texture_handles[i]) {
                LoadState::Loaded => {
                    // Texture loaded successfully - use it!
                    info!("Using loaded texture #{}: {}", i, texture_name);
                    materials.add(StandardMaterial {
                        base_color_texture: Some(texture_handles[i].clone()),
                        base_color: Color::WHITE,
                        perceptual_roughness: 0.8,
                        metallic: 0.0,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Mask(0.5),
                        ..default()
                    })
                }
                _ => {
                    // Texture failed or not loaded - use colored fallback
                    warn!(
                        "Texture failed, using colored fallback for: {}",
                        texture_name
                    );
                    create_colored_fallback_material(i, materials)
                }
            }
        } else {
            // Empty texture name or no handle - use colored fallback
            create_colored_fallback_material(i, materials)
        };

        texture_materials.push(material);
    }

    texture_materials
}

/// Create a colored fallback material
fn create_colored_fallback_material(
    index: usize,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Handle<StandardMaterial> {
    let color = match index % 10 {
        0 => Color::srgb(0.8, 0.6, 0.4), // Brown
        1 => Color::srgb(0.4, 0.8, 0.4), // Green
        2 => Color::srgb(0.6, 0.6, 0.8), // Blue
        3 => Color::srgb(0.8, 0.8, 0.6), // Yellow
        4 => Color::srgb(0.8, 0.4, 0.4), // Red
        5 => Color::srgb(0.4, 0.8, 0.8), // Cyan
        6 => Color::srgb(0.8, 0.4, 0.8), // Magenta
        7 => Color::srgb(0.6, 0.8, 0.4), // Lime
        8 => Color::srgb(0.4, 0.6, 0.8), // Sky blue
        _ => Color::srgb(0.7, 0.7, 0.7), // Grey
    };

    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.8,
        metallic: 0.0,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    })
}

/// Calculate smooth normals by averaging neighboring cell normals
/// Port of roBrowser's getSmoothNormal function
fn calculate_smooth_normals(
    ground: &crate::infrastructure::ro_formats::RoGround,
) -> Vec<[Vec3; 4]> {
    let width = ground.width as usize;
    let height = ground.height as usize;
    let surfaces = &ground.surfaces;

    // Calculate normal for each cell first
    let mut cell_normals = vec![Vec3::ZERO; width * height];

    for y in 0..height {
        for x in 0..width {
            let surface = &surfaces[y * width + x];

            // Only calculate normal if tile_up exists
            if surface.tile_up >= 0 {
                // Calculate positions of the 4 corners using GND coordinate system
                let a = Vec3::new((x as f32) * 2.0, surface.height[0], (y as f32) * 2.0); // SW
                let b = Vec3::new((x as f32 + 1.0) * 2.0, surface.height[1], (y as f32) * 2.0); // SE
                let c = Vec3::new(
                    (x as f32 + 1.0) * 2.0,
                    surface.height[3],
                    (y as f32 + 1.0) * 2.0,
                ); // NE
                let d = Vec3::new((x as f32) * 2.0, surface.height[2], (y as f32 + 1.0) * 2.0); // NW

                // Calculate normal using cross product of quad diagonals (like roBrowser)
                let diag1 = c - a; // SW to NE diagonal
                let diag2 = d - b; // SE to NW diagonal
                let normal = diag1.cross(diag2).normalize_or_zero();

                cell_normals[y * width + x] = normal;
            }
        }
    }

    // Now smooth normals by averaging neighbors (like roBrowser getSmoothNormal)
    let mut smooth_normals = vec![[Vec3::ZERO; 4]; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            // Calculate smooth normal for each vertex of this quad
            // Each vertex normal = average of surrounding cell normals

            // Vertex 0 (SW): average of current + left + bottom-left + bottom cells
            let mut n0 = cell_normals[idx]; // current cell
            let mut count0 = 1;
            if x > 0 {
                n0 += cell_normals[y * width + (x - 1)]; // left cell
                count0 += 1;
            }
            if y + 1 < height && x > 0 {
                n0 += cell_normals[(y + 1) * width + (x - 1)]; // bottom-left cell
                count0 += 1;
            }
            if y + 1 < height {
                n0 += cell_normals[(y + 1) * width + x]; // bottom cell
                count0 += 1;
            }
            smooth_normals[idx][0] = (n0 / count0 as f32).normalize_or_zero(); // SW vertex

            // Vertex 1 (SE): average of current + right + bottom-right + bottom cells
            let mut n1 = cell_normals[idx]; // current cell
            let mut count1 = 1;
            if x + 1 < width {
                n1 += cell_normals[y * width + (x + 1)]; // right cell
                count1 += 1;
            }
            if y + 1 < height && x + 1 < width {
                n1 += cell_normals[(y + 1) * width + (x + 1)]; // bottom-right cell
                count1 += 1;
            }
            if y + 1 < height {
                n1 += cell_normals[(y + 1) * width + x]; // bottom cell
                count1 += 1;
            }
            smooth_normals[idx][1] = (n1 / count1 as f32).normalize_or_zero(); // SE vertex

            // Vertex 2 (NW): average of current + left + top-left + top cells
            let mut n2 = cell_normals[idx]; // current cell
            let mut count2 = 1;
            if x > 0 {
                n2 += cell_normals[y * width + (x - 1)]; // left cell
                count2 += 1;
            }
            if y > 0 && x > 0 {
                n2 += cell_normals[(y - 1) * width + (x - 1)]; // top-left cell
                count2 += 1;
            }
            if y > 0 {
                n2 += cell_normals[(y - 1) * width + x]; // top cell
                count2 += 1;
            }
            smooth_normals[idx][2] = (n2 / count2 as f32).normalize_or_zero(); // NW vertex

            // Vertex 3 (NE): average of current + right + top-right + top cells
            let mut n3 = cell_normals[idx]; // current cell
            let mut count3 = 1;
            if x + 1 < width {
                n3 += cell_normals[y * width + (x + 1)]; // right cell
                count3 += 1;
            }
            if y > 0 && x + 1 < width {
                n3 += cell_normals[(y - 1) * width + (x + 1)]; // top-right cell
                count3 += 1;
            }
            if y > 0 {
                n3 += cell_normals[(y - 1) * width + x]; // top cell
                count3 += 1;
            }
            smooth_normals[idx][3] = (n3 / count3 as f32).normalize_or_zero(); // NE vertex
        }
    }

    smooth_normals
}

/// Generate front wall using exact heights (no smoothing)
fn generate_front_wall(
    meshes_by_texture: &mut std::collections::HashMap<
        usize,
        (Vec<Vec3>, Vec<Vec3>, Vec<[f32; 4]>, Vec<[f32; 2]>, Vec<u32>),
    >,
    ground: &crate::infrastructure::ro_formats::RoGround,
    x: usize,
    y: usize,
    width: usize,
    surface: &crate::infrastructure::ro_formats::GndSurface,
) {
    let next_surface = &ground.surfaces[(y + 1) * width + x];

    // Get tile for this wall
    let tile_idx = surface.tile_front as usize;
    if tile_idx >= ground.tiles.len() {
        return;
    }

    let tile = &ground.tiles[tile_idx];
    let texture_index_idx = tile.texture as usize;
    let final_texture_idx = if texture_index_idx < ground.texture_indexes.len() {
        ground.texture_indexes[texture_index_idx]
    } else {
        0
    };

    // Get mesh data for this texture
    let mesh_data = meshes_by_texture
        .entry(final_texture_idx)
        .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()));

    let base_x = x as f32 * CELL_SIZE;
    let base_z = (y + 1) as f32 * CELL_SIZE; // Wall is at Y+1 boundary

    // Use EXACT heights from GND data (no smoothing for walls)
    let current_heights = [
        surface.height[0] * 5.0, // SW
        surface.height[1] * 5.0, // SE
        surface.height[2] * 5.0, // NW
        surface.height[3] * 5.0, // NE
    ];

    let next_heights = [
        next_surface.height[0] * 5.0, // SW
        next_surface.height[1] * 5.0, // SE
        next_surface.height[2] * 5.0, // NW
        next_surface.height[3] * 5.0, // NE
    ];

    let vertex_offset = mesh_data.0.len() as u32;

    // Create wall quad between the two cells using exact heights
    // Wall connects current cell's north edge (NW, NE) to next cell's south edge (SW, SE)
    mesh_data
        .0
        .push(Vec3::new(base_x, current_heights[2], base_z)); // Current NW
    mesh_data
        .0
        .push(Vec3::new(base_x + CELL_SIZE, current_heights[3], base_z)); // Current NE
    mesh_data.0.push(Vec3::new(base_x, next_heights[0], base_z)); // Next SW
    mesh_data
        .0
        .push(Vec3::new(base_x + CELL_SIZE, next_heights[1], base_z)); // Next SE

    // Hard normal for front wall (facing negative Z direction) - no smoothing
    let wall_normal = Vec3::new(0.0, 0.0, -1.0);
    for _ in 0..4 {
        mesh_data.1.push(wall_normal);
    }

    // Use tile color for artistic variation (GND uses ARGB format)
    let tile_color = [
        tile.color[1] as f32 / 255.0, // R
        tile.color[2] as f32 / 255.0, // G
        tile.color[3] as f32 / 255.0, // B
        tile.color[0] as f32 / 255.0, // A
    ];
    for _ in 0..4 {
        mesh_data.2.push(tile_color);
    }

    // UV coordinates from tile - matching corrected vertex order
    mesh_data.3.push([tile.u1, tile.v1]); // Current NW -> tile NW
    mesh_data.3.push([tile.u2, tile.v2]); // Current NE -> tile NE
    mesh_data.3.push([tile.u3, tile.v3]); // Next SW -> tile SW
    mesh_data.3.push([tile.u4, tile.v4]); // Next SE -> tile SE

    // Indices for wall quad
    mesh_data.4.push(vertex_offset); // Current NW
    mesh_data.4.push(vertex_offset + 1); // Current NE
    mesh_data.4.push(vertex_offset + 2); // Next SW
    mesh_data.4.push(vertex_offset + 2); // Next SW
    mesh_data.4.push(vertex_offset + 1); // Current NE
    mesh_data.4.push(vertex_offset + 3); // Next SE
}

/// Generate right wall using exact heights (no smoothing)
fn generate_right_wall(
    meshes_by_texture: &mut std::collections::HashMap<
        usize,
        (Vec<Vec3>, Vec<Vec3>, Vec<[f32; 4]>, Vec<[f32; 2]>, Vec<u32>),
    >,
    ground: &crate::infrastructure::ro_formats::RoGround,
    x: usize,
    y: usize,
    width: usize,
    surface: &crate::infrastructure::ro_formats::GndSurface,
) {
    let next_surface = &ground.surfaces[y * width + (x + 1)];

    // Get tile for this wall
    let tile_idx = surface.tile_right as usize;
    if tile_idx >= ground.tiles.len() {
        return;
    }

    let tile = &ground.tiles[tile_idx];
    let texture_index_idx = tile.texture as usize;
    let final_texture_idx = if texture_index_idx < ground.texture_indexes.len() {
        ground.texture_indexes[texture_index_idx]
    } else {
        0
    };

    // Get mesh data for this texture
    let mesh_data = meshes_by_texture
        .entry(final_texture_idx)
        .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()));

    let base_x = (x + 1) as f32 * CELL_SIZE; // Wall is at X+1 boundary
    let base_z = y as f32 * CELL_SIZE;

    // Use EXACT heights from GND data (no smoothing for walls)
    let current_heights = [
        surface.height[0] * 5.0, // SW
        surface.height[1] * 5.0, // SE
        surface.height[2] * 5.0, // NW
        surface.height[3] * 5.0, // NE
    ];

    let next_heights = [
        next_surface.height[0] * 5.0, // SW
        next_surface.height[1] * 5.0, // SE
        next_surface.height[2] * 5.0, // NW
        next_surface.height[3] * 5.0, // NE
    ];

    let vertex_offset = mesh_data.0.len() as u32;

    // Create wall quad between the two cells using exact heights
    // Wall connects current cell's east edge (SE, NE) to next cell's west edge (SW, NW)
    mesh_data
        .0
        .push(Vec3::new(base_x, current_heights[1], base_z)); // Current SE (at y+0)
    mesh_data
        .0
        .push(Vec3::new(base_x, current_heights[3], base_z + CELL_SIZE)); // Current NE (at y+1)
    mesh_data.0.push(Vec3::new(base_x, next_heights[0], base_z)); // Next SW (at y+0)
    mesh_data
        .0
        .push(Vec3::new(base_x, next_heights[2], base_z + CELL_SIZE)); // Next NW (at y+1)

    // Hard normal for right wall (facing negative X direction) - no smoothing
    let wall_normal = Vec3::new(-1.0, 0.0, 0.0);
    for _ in 0..4 {
        mesh_data.1.push(wall_normal);
    }

    // Use tile color for artistic variation (GND uses ARGB format)
    let tile_color = [
        tile.color[1] as f32 / 255.0, // R
        tile.color[2] as f32 / 255.0, // G
        tile.color[3] as f32 / 255.0, // B
        tile.color[0] as f32 / 255.0, // A
    ];
    for _ in 0..4 {
        mesh_data.2.push(tile_color);
    }

    // UV coordinates from tile - matching roBrowser right wall pattern
    mesh_data.3.push([tile.u2, tile.v2]); // Bottom-left
    mesh_data.3.push([tile.u1, tile.v1]); // Bottom-right
    mesh_data.3.push([tile.u4, tile.v4]); // Top-left
    mesh_data.3.push([tile.u3, tile.v3]); // Top-right

    // Indices for wall quad
    mesh_data.4.push(vertex_offset); // Current SE
    mesh_data.4.push(vertex_offset + 1); // Current NE
    mesh_data.4.push(vertex_offset + 2); // Next SW
    mesh_data.4.push(vertex_offset + 2); // Next SW
    mesh_data.4.push(vertex_offset + 1); // Current NE
    mesh_data.4.push(vertex_offset + 3); // Next NW
}

pub fn generate_terrain_mesh(
    mut commands: Commands,
    ground_assets: Res<Assets<RoGroundAsset>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    asset_server: Res<AssetServer>,
    query: Query<
        (Entity, &MapLoader, &MapRequestLoader),
        (Without<MapData>, Without<TerrainTexturesLoading>),
    >,
) {
    for (entity, map_loader, map_request) in query.iter() {
        debug!(
            "generate_terrain_mesh: Processing MapLoader for map '{}'",
            map_request.map_name
        );

        // Check if ground asset and its dependencies are actually loaded via AssetServer
        if !asset_server.is_loaded_with_dependencies(&map_loader.ground) {
            debug!(
                "generate_terrain_mesh: Waiting for ground asset and dependencies to load for '{}'",
                map_request.map_name
            );
            continue;
        }

        // Check optional altitude asset
        if let Some(ref alt_handle) = map_loader.altitude {
            if !asset_server.is_loaded_with_dependencies(alt_handle) {
                debug!(
                    "generate_terrain_mesh: Waiting for altitude asset to load for '{}'",
                    map_request.map_name
                );
                continue;
            }
        }

        // Check optional world asset
        if let Some(ref world_handle) = map_loader.world {
            if !asset_server.is_loaded_with_dependencies(world_handle) {
                debug!(
                    "generate_terrain_mesh: Waiting for world asset to load for '{}'",
                    map_request.map_name
                );
                continue;
            }
        }

        // NOW it's safe to access - assets are guaranteed loaded
        let Some(ground) = ground_assets.get(&map_loader.ground) else {
            error!(
                "generate_terrain_mesh: Asset marked as loaded but not in storage for '{}'",
                map_request.map_name
            );
            continue;
        };

        debug!(
            "generate_terrain_mesh: Ground asset loaded for map '{}', starting texture loading",
            map_request.map_name
        );

        // Start loading textures asynchronously
        let mut texture_handles = Vec::new();
        let mut texture_names = Vec::new();

        for texture_name in ground.ground.textures.iter() {
            if !texture_name.is_empty() {
                let texture_path = format!("ro://data\\texture\\{}", texture_name);
                let handle: Handle<Image> = asset_server.load(&texture_path);
                texture_handles.push(handle);
                texture_names.push(texture_name.clone());
                info!("Started loading terrain texture: {}", texture_path);
            } else {
                // For empty texture names, push a default handle (will use colored fallback)
                texture_handles.push(Handle::default());
                texture_names.push(String::new());
            }
        }

        // Add component to track texture loading
        commands.entity(entity).insert(TerrainTexturesLoading {
            texture_handles,
            texture_names,
            ground_handle: map_loader.ground.clone(),
            altitude_handle: map_loader.altitude.clone(),
        });

        info!(
            "generate_terrain_mesh: Started loading {} textures for map '{}'",
            ground.ground.textures.len(),
            map_request.map_name
        );
    }
}

/// System that waits for textures to load, then generates terrain meshes
pub fn generate_terrain_when_textures_ready(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &TerrainTexturesLoading, &MapRequestLoader)>,
) {
    use bevy::asset::LoadState;

    for (entity, textures_loading, map_request) in query.iter() {
        // Check if all textures are loaded or failed
        let mut all_ready = true;
        let mut loaded_count = 0;
        let mut failed_count = 0;

        for (i, handle) in textures_loading.texture_handles.iter().enumerate() {
            if handle.id() == AssetId::default() {
                // Empty texture, skip
                continue;
            }

            match asset_server.load_state(handle) {
                LoadState::Loaded => {
                    loaded_count += 1;
                }
                LoadState::Failed(_) => {
                    failed_count += 1;
                    warn!(
                        "Failed to load texture: {}",
                        textures_loading.texture_names[i]
                    );
                }
                LoadState::Loading | LoadState::NotLoaded => {
                    all_ready = false;
                    break;
                }
            }
        }

        if !all_ready {
            debug!(
                "Waiting for textures to load for map '{}'",
                map_request.map_name
            );
            continue;
        }

        info!(
            "All textures ready for map '{}': {} loaded, {} failed",
            map_request.map_name, loaded_count, failed_count
        );

        // Get ground asset
        let Some(ground) = ground_assets.get(&textures_loading.ground_handle) else {
            error!("Ground asset not found in storage");
            continue;
        };

        let altitude = textures_loading
            .altitude_handle
            .as_ref()
            .and_then(|h| altitude_assets.get(h))
            .map(|a| &a.altitude);

        // Create materials now that textures are loaded
        let texture_materials = create_terrain_materials_from_loaded_textures(
            &ground.ground,
            &textures_loading.texture_handles,
            &textures_loading.texture_names,
            &asset_server,
            &mut materials,
        );

        // GAT is used for collision detection, not terrain rendering
        // GND surfaces contain the height data we need for terrain mesh generation

        // Create separate meshes for each texture using roBrowser approach (6 vertices per cell, no sharing)
        let meshes_by_texture = create_terrain_meshes_robrowser_style(&ground.ground, altitude);

        // Spawn a mesh entity for each texture
        let mut mesh_count = 0;
        for (texture_idx, mesh) in meshes_by_texture {
            let vertex_count = mesh.count_vertices();
            if vertex_count == 0 {
                debug!(
                    "generate_terrain_mesh: Skipping empty mesh for texture_idx {}",
                    texture_idx
                );
                continue; // Skip empty meshes
            }

            debug!(
                "generate_terrain_mesh: Spawning mesh #{} at (0,0,0), vertices: {}, texture_idx: {}",
                mesh_count + 1,
                vertex_count,
                texture_idx
            );

            let mesh_handle = meshes.add(mesh);

            // Get the material for this texture index
            let material = if texture_idx < texture_materials.len() {
                texture_materials[texture_idx].clone()
            } else {
                // Fallback material - bright cyan to make it obvious
                warn!(
                    "generate_terrain_mesh: Using fallback material for texture_idx {}",
                    texture_idx
                );
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 1.0, 1.0),
                    perceptual_roughness: 0.5,
                    metallic: 0.0,
                    double_sided: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            };

            // Spawn terrain chunk at origin like RoBrowser
            commands.spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(material),
                Transform::from_xyz(
                    0.0, // Start at origin X
                    0.0, // Y unchanged
                    0.0, // Start at origin Z
                ),
            ));

            mesh_count += 1;
        }

        info!(
            "generate_terrain_mesh: Spawned {} terrain mesh entities total for map '{}'",
            mesh_count, map_request.map_name
        );

        // Update the original entity with map data and remove loading component
        commands
            .entity(entity)
            .insert(MapData {
                name: "Map".to_string(),
                width: ground.ground.width,
                height: ground.ground.height,
            })
            .remove::<TerrainTexturesLoading>();

        info!(
            "generate_terrain_when_textures_ready: Successfully generated terrain mesh and inserted MapData for map '{}'",
            map_request.map_name
        );
    }
}

fn create_terrain_meshes_robrowser_style(
    ground: &crate::infrastructure::ro_formats::RoGround,
    _altitude: Option<&crate::infrastructure::ro_formats::RoAltitude>,
) -> Vec<(usize, Mesh)> {
    use std::collections::HashMap;

    let width = ground.width as usize;
    let height = ground.height as usize;

    // Calculate smooth normals for all cells (same as roBrowser)
    let smooth_normals = calculate_smooth_normals(ground);

    // Group triangles by texture, storing vertex data and indices
    let mut meshes_by_texture: HashMap<
        usize,
        (Vec<Vec3>, Vec<Vec3>, Vec<[f32; 4]>, Vec<[f32; 2]>, Vec<u32>),
    > = HashMap::new();

    // Generate terrain quads (roBrowser approach - 6 vertices per cell, no sharing)
    for y in 0..height {
        for x in 0..width {
            let surface = &ground.surfaces[y * width + x];

            // Check tile up (same condition as roBrowser)
            if surface.tile_up < 0 {
                continue;
            }

            // Get the tile for this surface
            let tile_idx = surface.tile_up as usize;
            let tile = if tile_idx < ground.tiles.len() {
                &ground.tiles[tile_idx]
            } else {
                continue; // Skip if tile index is invalid
            };

            // Get texture index (same logic as roBrowser)
            let texture_index_idx = tile.texture as usize;
            let final_texture_idx = if texture_index_idx < ground.texture_indexes.len() {
                ground.texture_indexes[texture_index_idx]
            } else {
                0
            };

            // Get or create mesh data for this texture
            let mesh_data = meshes_by_texture
                .entry(final_texture_idx)
                .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()));

            // Get heights for this cell (exact heights like roBrowser)
            let h = &surface.height;
            let normals = &smooth_normals[y * width + x];

            let vertex_offset = mesh_data.0.len() as u32;

            // Generate 6 vertices for 2 triangles (roBrowser approach)
            // Positions: (x+0)*2, h[0], (y+0)*2 etc. (matching roBrowser exactly)

            // Triangle 1: (x+0,y+0) -> (x+1,y+0) -> (x+1,y+1)
            mesh_data.0.push(Vec3::new(
                (x as f32 + 0.0) * CELL_SIZE,
                h[0] * 5.0,
                (y as f32 + 0.0) * CELL_SIZE,
            ));
            mesh_data.0.push(Vec3::new(
                (x as f32 + 1.0) * CELL_SIZE,
                h[1] * 5.0,
                (y as f32 + 0.0) * CELL_SIZE,
            ));
            mesh_data.0.push(Vec3::new(
                (x as f32 + 1.0) * CELL_SIZE,
                h[3] * 5.0,
                (y as f32 + 1.0) * CELL_SIZE,
            ));

            // Triangle 2: (x+1,y+1) -> (x+0,y+1) -> (x+0,y+0)
            mesh_data.0.push(Vec3::new(
                (x as f32 + 1.0) * CELL_SIZE,
                h[3] * 5.0,
                (y as f32 + 1.0) * CELL_SIZE,
            ));
            mesh_data.0.push(Vec3::new(
                (x as f32 + 0.0) * CELL_SIZE,
                h[2] * 5.0,
                (y as f32 + 1.0) * CELL_SIZE,
            ));
            mesh_data.0.push(Vec3::new(
                (x as f32 + 0.0) * CELL_SIZE,
                h[0] * 5.0,
                (y as f32 + 0.0) * CELL_SIZE,
            ));

            // Normals (roBrowser mapping: n[0]=UL, n[1]=UR, n[2]=BR, n[3]=BL)
            mesh_data.1.push(normals[0]); // UL (x+0,y+0)
            mesh_data.1.push(normals[1]); // UR (x+1,y+0)
            mesh_data.1.push(normals[2]); // BR (x+1,y+1)
            mesh_data.1.push(normals[2]); // BR (x+1,y+1) - repeated
            mesh_data.1.push(normals[3]); // BL (x+0,y+1)
            mesh_data.1.push(normals[0]); // UL (x+0,y+0) - repeated

            // Use tile color for artistic variation (GND uses ARGB format)
            let tile_color = [
                tile.color[1] as f32 / 255.0, // R
                tile.color[2] as f32 / 255.0, // G
                tile.color[3] as f32 / 255.0, // B
                tile.color[0] as f32 / 255.0, // A
            ];
            for _ in 0..6 {
                mesh_data.2.push(tile_color);
            }

            // UV coordinates from tile (roBrowser mapping)
            mesh_data.3.push([tile.u1, tile.v1]); // UL
            mesh_data.3.push([tile.u2, tile.v2]); // UR
            mesh_data.3.push([tile.u4, tile.v4]); // BR
            mesh_data.3.push([tile.u4, tile.v4]); // BR - repeated
            mesh_data.3.push([tile.u3, tile.v3]); // BL
            mesh_data.3.push([tile.u1, tile.v1]); // UL - repeated

            // Indices (sequential, no sharing)
            for i in 0..6 {
                mesh_data.4.push(vertex_offset + i);
            }
        }
    }

    // Generate walls after terrain quads (selective based on GND tile data)
    for y in 0..height {
        for x in 0..width {
            let surface = &ground.surfaces[y * width + x];

            // Generate front wall ONLY when tile_front is explicitly defined
            if surface.tile_front >= 0 && (y + 1) < height {
                generate_front_wall(&mut meshes_by_texture, ground, x, y, width, surface);
            }

            // Generate right wall ONLY when tile_right is explicitly defined
            if surface.tile_right >= 0 && (x + 1) < width {
                generate_right_wall(&mut meshes_by_texture, ground, x, y, width, surface);
            }
        }
    }

    let mut result = Vec::new();

    for (texture_idx, (positions, normals, colors, uvs, indices)) in meshes_by_texture {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        if !positions.is_empty() {
            // Convert Vec3 positions to [f32; 3] arrays
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                positions
                    .iter()
                    .map(|v| [v.x, v.y, v.z])
                    .collect::<Vec<_>>(),
            );

            // Convert Vec3 normals to [f32; 3] arrays
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                normals.iter().map(|v| [v.x, v.y, v.z]).collect::<Vec<_>>(),
            );

            // Colors are already [f32; 4] arrays
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

            // UVs are already [f32; 2] arrays
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

            mesh.insert_indices(Indices::U32(indices));

            if texture_idx < ground.textures.len() {
                &ground.textures[texture_idx]
            } else {
                "unknown"
            };

            result.push((texture_idx, mesh));
        }
    }

    result
}
