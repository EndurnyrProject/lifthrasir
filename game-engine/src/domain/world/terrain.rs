use crate::{
    domain::world::{components::MapLoader, map::MapData, map_loader::MapRequestLoader},
    infrastructure::assets::loaders::{RoAltitudeAsset, RoGroundAsset},
    utils::constants::CELL_SIZE,
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::HashMap;

/// Type alias for mesh data grouped by texture index.
/// Maps texture index to its associated mesh data
type MeshDataByTexture = HashMap<usize, MeshData>;

/// Type alias for querying map entities ready for terrain generation
type TerrainGenerationQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static MapLoader, &'static MapRequestLoader),
    (Without<MapData>, Without<TerrainTexturesLoading>),
>;

/// Material property constants for terrain rendering
const TERRAIN_ROUGHNESS: f32 = 0.8;
const TERRAIN_METALLIC: f32 = 0.0;
const TERRAIN_ALPHA_THRESHOLD: f32 = 0.5;

/// Mesh data grouped by texture index for terrain generation
#[derive(Debug, Default)]
struct MeshData {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colors: Vec<[f32; 4]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Convert ARGB color format to normalized RGBA values
/// GND files use ARGB format (alpha in index 0), we need RGBA for rendering
#[inline]
fn argb_to_rgba_normalized(argb: &[u8; 4]) -> [f32; 4] {
    [
        argb[1] as f32 / 255.0, // R
        argb[2] as f32 / 255.0, // G
        argb[3] as f32 / 255.0, // B
        argb[0] as f32 / 255.0, // A
    ]
}

/// Resolve the actual texture index from a tile's texture reference
/// Returns 0 as fallback if the index is out of bounds
#[inline]
fn resolve_texture_index(
    tile: &crate::infrastructure::ro_formats::GndTile,
    ground: &crate::infrastructure::ro_formats::RoGround,
) -> usize {
    let idx = tile.texture as usize;
    if idx < ground.texture_indexes.len() {
        ground.texture_indexes[idx]
    } else {
        0
    }
}

/// Calculate 3D position for a cell vertex
/// Applies CELL_SIZE scaling and supports fractional offsets for sub-cell positioning
#[inline]
fn cell_vertex_position(x: usize, y: usize, height: f32, offset_x: f32, offset_y: f32) -> Vec3 {
    Vec3::new(
        (x as f32 + offset_x) * CELL_SIZE,
        height,
        (y as f32 + offset_y) * CELL_SIZE,
    )
}

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
    asset_server: &AssetServer,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Vec<Handle<StandardMaterial>> {
    use bevy::asset::LoadState;
    let mut texture_materials = Vec::new();

    for (i, texture_name) in ground.textures.iter().enumerate() {
        let material = if i < texture_handles.len() && texture_handles[i].id() != AssetId::default()
        {
            match asset_server.load_state(&texture_handles[i]) {
                LoadState::Loaded => {
                    info!("Using loaded texture #{}: {}", i, texture_name);
                    materials.add(StandardMaterial {
                        base_color_texture: Some(texture_handles[i].clone()),
                        base_color: Color::WHITE,
                        perceptual_roughness: TERRAIN_ROUGHNESS,
                        metallic: TERRAIN_METALLIC,
                        cull_mode: None,
                        alpha_mode: AlphaMode::Mask(TERRAIN_ALPHA_THRESHOLD),
                        ..default()
                    })
                }
                _ => {
                    warn!(
                        "Texture failed, using colored fallback for: {}",
                        texture_name
                    );
                    create_colored_fallback_material(i, materials)
                }
            }
        } else {
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
        perceptual_roughness: TERRAIN_ROUGHNESS,
        metallic: TERRAIN_METALLIC,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(TERRAIN_ALPHA_THRESHOLD),
        ..default()
    })
}

/// Wall direction for parametric wall generation
enum WallDirection {
    /// Front wall (facing negative Z)
    Front,
    /// Right wall (facing negative X)
    Right,
}

/// Generate a wall quad (front or right) using exact heights
/// Unifies the logic from generate_front_wall and generate_right_wall
fn generate_wall(
    meshes_by_texture: &mut MeshDataByTexture,
    ground: &crate::infrastructure::ro_formats::RoGround,
    x: usize,
    y: usize,
    width: usize,
    surface: &crate::infrastructure::ro_formats::GndSurface,
    direction: WallDirection,
) {
    // Get the appropriate tile index and next surface based on direction
    let (tile_field, next_surface_offset) = match direction {
        WallDirection::Front => (surface.tile_front, (y + 1) * width + x),
        WallDirection::Right => (surface.tile_right, y * width + (x + 1)),
    };

    let tile_idx = tile_field as usize;
    if tile_idx >= ground.tiles.len() {
        return;
    }

    let tile = &ground.tiles[tile_idx];
    let final_texture_idx = resolve_texture_index(tile, ground);

    // Get mesh data for this texture
    let mesh_data = meshes_by_texture.entry(final_texture_idx).or_default();

    let next_surface = &ground.surfaces[next_surface_offset];

    // Heights from GND data (no smoothing for walls)
    let current_heights = [
        surface.height[0], // SW
        surface.height[1], // SE
        surface.height[2], // NW
        surface.height[3], // NE
    ];

    let next_heights = [
        next_surface.height[0], // SW
        next_surface.height[1], // SE
        next_surface.height[2], // NW
        next_surface.height[3], // NE
    ];

    let vertex_offset = mesh_data.positions.len() as u32;

    // Generate vertices based on wall direction
    match direction {
        WallDirection::Front => {
            let base_x = x as f32 * CELL_SIZE;
            let base_z = (y + 1) as f32 * CELL_SIZE; // Wall is at Y+1 boundary

            // Wall connects current cell's north edge (NW, NE) to next cell's south edge (SW, SE)
            mesh_data
                .positions
                .push(Vec3::new(base_x, current_heights[2], base_z)); // Current NW
            mesh_data
                .positions
                .push(Vec3::new(base_x + CELL_SIZE, current_heights[3], base_z)); // Current NE
            mesh_data
                .positions
                .push(Vec3::new(base_x, next_heights[0], base_z)); // Next SW
            mesh_data
                .positions
                .push(Vec3::new(base_x + CELL_SIZE, next_heights[1], base_z)); // Next SE

            // UV coordinates for front wall
            mesh_data.uvs.push([tile.u1, tile.v1]); // Current NW -> tile NW
            mesh_data.uvs.push([tile.u2, tile.v2]); // Current NE -> tile NE
            mesh_data.uvs.push([tile.u3, tile.v3]); // Next SW -> tile SW
            mesh_data.uvs.push([tile.u4, tile.v4]); // Next SE -> tile SE
        }
        WallDirection::Right => {
            let base_x = (x + 1) as f32 * CELL_SIZE; // Wall is at X+1 boundary
            let base_z = y as f32 * CELL_SIZE;

            // Wall connects current cell's east edge (SE, NE) to next cell's west edge (SW, NW)
            mesh_data
                .positions
                .push(Vec3::new(base_x, current_heights[1], base_z)); // Current SE (at y+0)
            mesh_data
                .positions
                .push(Vec3::new(base_x, current_heights[3], base_z + CELL_SIZE)); // Current NE (at y+1)
            mesh_data
                .positions
                .push(Vec3::new(base_x, next_heights[0], base_z)); // Next SW (at y+0)
            mesh_data
                .positions
                .push(Vec3::new(base_x, next_heights[2], base_z + CELL_SIZE)); // Next NW (at y+1)

            // UV coordinates for right wall
            mesh_data.uvs.push([tile.u2, tile.v2]); // Bottom-left
            mesh_data.uvs.push([tile.u1, tile.v1]); // Bottom-right
            mesh_data.uvs.push([tile.u4, tile.v4]); // Top-left
            mesh_data.uvs.push([tile.u3, tile.v3]); // Top-right
        }
    }

    // Hard normal based on wall direction - no smoothing
    let wall_normal = match direction {
        WallDirection::Front => Vec3::new(0.0, 0.0, -1.0),
        WallDirection::Right => Vec3::new(-1.0, 0.0, 0.0),
    };
    for _ in 0..4 {
        mesh_data.normals.push(wall_normal);
    }

    // Use tile color for artistic variation (GND uses ARGB format)
    let tile_color = argb_to_rgba_normalized(&tile.color);
    for _ in 0..4 {
        mesh_data.colors.push(tile_color);
    }

    // Indices for wall quad (same for both directions)
    mesh_data.indices.push(vertex_offset); // First vertex
    mesh_data.indices.push(vertex_offset + 1); // Second vertex
    mesh_data.indices.push(vertex_offset + 2); // Third vertex
    mesh_data.indices.push(vertex_offset + 2); // Third vertex (repeated)
    mesh_data.indices.push(vertex_offset + 1); // Second vertex (repeated)
    mesh_data.indices.push(vertex_offset + 3); // Fourth vertex
}

/// Calculate the base normal for each cell using cross product of diagonals
fn calculate_cell_normals(
    surfaces: &[crate::infrastructure::ro_formats::GndSurface],
    width: usize,
    height: usize,
) -> Vec<Vec3> {
    let mut cell_normals = vec![Vec3::ZERO; width * height];

    for y in 0..height {
        for x in 0..width {
            let surface = &surfaces[y * width + x];

            // Only calculate normal if tile_up exists
            if surface.tile_up >= 0 {
                // Calculate positions of the 4 corners matching actual mesh coordinates
                let a = Vec3::new(
                    (x as f32) * CELL_SIZE,
                    surface.height[0],
                    (y as f32) * CELL_SIZE,
                ); // SW
                let b = Vec3::new(
                    (x as f32 + 1.0) * CELL_SIZE,
                    surface.height[1],
                    (y as f32) * CELL_SIZE,
                ); // SE
                let c = Vec3::new(
                    (x as f32 + 1.0) * CELL_SIZE,
                    surface.height[3],
                    (y as f32 + 1.0) * CELL_SIZE,
                ); // NE
                let d = Vec3::new(
                    (x as f32) * CELL_SIZE,
                    surface.height[2],
                    (y as f32 + 1.0) * CELL_SIZE,
                ); // NW

                // Calculate normal using cross product of quad diagonals (like roBrowser)
                let diag1 = c - a; // SW to NE diagonal
                let diag2 = d - b; // SE to NW diagonal
                let normal = diag1.cross(diag2).normalize_or_zero();

                cell_normals[y * width + x] = normal;
            }
        }
    }

    cell_normals
}

/// Calculate smooth normals by averaging neighboring cell normals
fn calculate_smooth_normals(
    ground: &crate::infrastructure::ro_formats::RoGround,
) -> Vec<[Vec3; 4]> {
    let width = ground.width as usize;
    let height = ground.height as usize;

    // First calculate base normal for each cell
    let cell_normals = calculate_cell_normals(&ground.surfaces, width, height);

    // Now smooth normals by averaging neighbors
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

pub fn generate_terrain_mesh(
    mut commands: Commands,
    ground_assets: Res<Assets<RoGroundAsset>>,
    asset_server: Res<AssetServer>,
    query: TerrainGenerationQuery,
) {
    for (entity, map_loader, map_request) in query.iter() {
        debug!(
            "generate_terrain_mesh: Processing MapLoader for map '{}'",
            map_request.map_name
        );

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
            &asset_server,
            &mut materials,
        );

        let meshes_by_texture = create_terrain_meshes(&ground.ground, altitude);

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

            let material = if texture_idx < texture_materials.len() {
                texture_materials[texture_idx].clone()
            } else {
                warn!(
                    "generate_terrain_mesh: Using fallback material for texture_idx {}",
                    texture_idx
                );
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.0, 1.0, 1.0),
                    perceptual_roughness: TERRAIN_ROUGHNESS,
                    metallic: TERRAIN_METALLIC,
                    double_sided: true,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(TERRAIN_ALPHA_THRESHOLD),
                    ..default()
                })
            };

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

fn create_terrain_meshes(
    ground: &crate::infrastructure::ro_formats::RoGround,
    _altitude: Option<&crate::infrastructure::ro_formats::RoAltitude>,
) -> Vec<(usize, Mesh)> {
    let width = ground.width as usize;
    let height = ground.height as usize;
    let smooth_normals = calculate_smooth_normals(ground);

    // Group triangles by texture, storing vertex data and indices
    let mut meshes_by_texture: MeshDataByTexture = HashMap::new();

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
            let final_texture_idx = resolve_texture_index(tile, ground);

            // Get or create mesh data for this texture
            let mesh_data = meshes_by_texture.entry(final_texture_idx).or_default();

            // Get heights for this cell (exact heights like roBrowser)
            let h = &surface.height;
            let normals = &smooth_normals[y * width + x];

            let vertex_offset = mesh_data.positions.len() as u32;

            // Generate 6 vertices for 2 triangles (roBrowser approach)
            // Positions: (x+0)*2, h[0], (y+0)*2 etc. (matching roBrowser exactly)

            // Triangle 1: (x+0,y+0) -> (x+1,y+0) -> (x+1,y+1)
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[0], 0.0, 0.0));
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[1], 1.0, 0.0));
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[3], 1.0, 1.0));

            // Triangle 2: (x+1,y+1) -> (x+0,y+1) -> (x+0,y+0)
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[3], 1.0, 1.0));
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[2], 0.0, 1.0));
            mesh_data
                .positions
                .push(cell_vertex_position(x, y, h[0], 0.0, 0.0));

            // Normals (roBrowser mapping: n[0]=UL, n[1]=UR, n[2]=BR, n[3]=BL)
            mesh_data.normals.push(normals[0]); // UL (x+0,y+0)
            mesh_data.normals.push(normals[1]); // UR (x+1,y+0)
            mesh_data.normals.push(normals[2]); // BR (x+1,y+1)
            mesh_data.normals.push(normals[2]); // BR (x+1,y+1) - repeated
            mesh_data.normals.push(normals[3]); // BL (x+0,y+1)
            mesh_data.normals.push(normals[0]); // UL (x+0,y+0) - repeated

            // Use tile color for artistic variation (GND uses ARGB format)
            let tile_color = argb_to_rgba_normalized(&tile.color);
            for _ in 0..6 {
                mesh_data.colors.push(tile_color);
            }

            // UV coordinates from tile (roBrowser mapping)
            mesh_data.uvs.push([tile.u1, tile.v1]); // UL
            mesh_data.uvs.push([tile.u2, tile.v2]); // UR
            mesh_data.uvs.push([tile.u4, tile.v4]); // BR
            mesh_data.uvs.push([tile.u4, tile.v4]); // BR - repeated
            mesh_data.uvs.push([tile.u3, tile.v3]); // BL
            mesh_data.uvs.push([tile.u1, tile.v1]); // UL - repeated

            // Indices (sequential, no sharing)
            for i in 0..6 {
                mesh_data.indices.push(vertex_offset + i);
            }
        }
    }

    // Generate walls after terrain quads (selective based on GND tile data)
    for y in 0..height {
        for x in 0..width {
            let surface = &ground.surfaces[y * width + x];

            // Generate front wall ONLY when tile_front is explicitly defined
            if surface.tile_front >= 0 && (y + 1) < height {
                generate_wall(
                    &mut meshes_by_texture,
                    ground,
                    x,
                    y,
                    width,
                    surface,
                    WallDirection::Front,
                );
            }

            // Generate right wall ONLY when tile_right is explicitly defined
            if surface.tile_right >= 0 && (x + 1) < width {
                generate_wall(
                    &mut meshes_by_texture,
                    ground,
                    x,
                    y,
                    width,
                    surface,
                    WallDirection::Right,
                );
            }
        }
    }

    let mut result = Vec::new();

    for (texture_idx, mesh_data) in meshes_by_texture {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        if !mesh_data.positions.is_empty() {
            // Convert Vec3 positions to [f32; 3] arrays
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                mesh_data
                    .positions
                    .iter()
                    .map(|v| [v.x, v.y, v.z])
                    .collect::<Vec<_>>(),
            );

            // Convert Vec3 normals to [f32; 3] arrays
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                mesh_data
                    .normals
                    .iter()
                    .map(|v| [v.x, v.y, v.z])
                    .collect::<Vec<_>>(),
            );

            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, mesh_data.colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs);
            mesh.insert_indices(Indices::U32(mesh_data.indices));

            result.push((texture_idx, mesh));
        }
    }

    result
}
