use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};

use crate::{
    assets::loaders::{GrfAsset, RoAltitudeAsset, RoGroundAsset},
    components::{GrfMapLoader, MapLoader, map::MapData},
    systems::camera_controls::CameraController,
    utils::constants::CELL_SIZE,
};

fn load_terrain_textures(
    ground: &crate::ro_formats::RoGround,
    grf_asset: &GrfAsset,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    images: &mut ResMut<Assets<Image>>,
) -> Vec<Handle<StandardMaterial>> {
    let mut texture_materials = Vec::new();

    for (i, texture_name) in ground.textures.iter().enumerate() {
        if !texture_name.is_empty() {
            let texture_paths = vec![
                texture_name.clone(),
                format!("data\\texture\\{}", texture_name),
            ];

            let mut texture_handle = None;
            for path in &texture_paths {
                if let Some(texture_data) = grf_asset.grf.get_file(path) {
                    if let Ok(image) = load_bmp_from_bytes(&texture_data) {
                        texture_handle = Some(images.add(image));
                        break;
                    }
                }
            }

            let material = if let Some(tex_handle) = texture_handle {
                materials.add(StandardMaterial {
                    base_color_texture: Some(tex_handle),
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.8,
                    metallic: 0.0,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            } else {
                // Fallback to colored material
                let fallback_color = match i {
                    0 => Color::srgb(0.8, 0.6, 0.4),
                    1 => Color::srgb(0.4, 0.8, 0.4),
                    2 => Color::srgb(0.6, 0.6, 0.8),
                    3 => Color::srgb(0.8, 0.8, 0.6),
                    _ => Color::srgb(0.7, 0.7, 0.7),
                };
                materials.add(StandardMaterial {
                    base_color: fallback_color,
                    perceptual_roughness: 0.8,
                    metallic: 0.0,
                    cull_mode: None,
                    alpha_mode: AlphaMode::Mask(0.5),
                    ..default()
                })
            };
            texture_materials.push(material);
        } else {
            // Empty texture - use dark grey
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 0.3, 0.3),
                perceptual_roughness: 0.9,
                metallic: 0.0,
                double_sided: true,
                cull_mode: None,
                alpha_mode: AlphaMode::Mask(0.5),
                ..default()
            });
            texture_materials.push(material);
        }
    }

    texture_materials
}

/// Calculate smooth normals by averaging neighboring cell normals
/// Port of roBrowser's getSmoothNormal function
fn calculate_smooth_normals(ground: &crate::ro_formats::RoGround) -> Vec<[Vec3; 4]> {
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
    ground: &crate::ro_formats::RoGround,
    x: usize,
    y: usize,
    width: usize,
    surface: &crate::ro_formats::GndSurface,
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
    ground: &crate::ro_formats::RoGround,
    x: usize,
    y: usize,
    width: usize,
    surface: &crate::ro_formats::GndSurface,
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    altitude_assets: Res<Assets<RoAltitudeAsset>>,
    grf_assets: Res<Assets<GrfAsset>>,
    query: Query<(Entity, &MapLoader, &GrfMapLoader), Without<MapData>>,
) {
    for (entity, map_loader, grf_loader) in query.iter() {
        let Some(ground) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let altitude = map_loader
            .altitude
            .as_ref()
            .and_then(|h| altitude_assets.get(h))
            .map(|a| &a.altitude);

        let Some(grf_asset) = grf_assets.get(&grf_loader.grf_handle) else {
            continue;
        };

        // Load textures
        let texture_materials =
            load_terrain_textures(&ground.ground, grf_asset, &mut materials, &mut images);

        // GAT is used for collision detection, not terrain rendering
        // GND surfaces contain the height data we need for terrain mesh generation

        // Create separate meshes for each texture using roBrowser approach (6 vertices per cell, no sharing)
        let meshes_by_texture = create_terrain_meshes_robrowser_style(&ground.ground, altitude);

        // Spawn a mesh entity for each texture
        for (texture_idx, mesh) in meshes_by_texture {
            if mesh.count_vertices() == 0 {
                continue; // Skip empty meshes
            }

            let mesh_handle = meshes.add(mesh);

            // Get the material for this texture index
            let material = if texture_idx < texture_materials.len() {
                texture_materials[texture_idx].clone()
            } else {
                // Fallback material - bright cyan to make it obvious
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
        }

        // Update the original entity with map data
        commands.entity(entity).insert((MapData {
            name: "Map".to_string(),
            width: ground.ground.width,
            height: ground.ground.height,
        },));
    }
}

fn create_terrain_meshes_robrowser_style(
    ground: &crate::ro_formats::RoGround,
    _altitude: Option<&crate::ro_formats::RoAltitude>,
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

pub fn setup_terrain_camera(mut commands: Commands, query: Query<&MapData, Added<MapData>>) {
    for map_data in query.iter() {
        let map_center_x = map_data.width as f32 * CELL_SIZE / 2.0;
        let map_center_z = map_data.height as f32 * CELL_SIZE / 2.0;

        let camera_pos = Vec3::new(map_center_x, -2000.0, -map_center_z * 2.5);
        let look_at = Vec3::new(map_center_x, 0.0, -map_center_z);

        commands.spawn((
            Camera3d::default(),
            Transform::from_translation(camera_pos).looking_at(look_at, Vec3::NEG_Y),
            CameraController::default(),
        ));
    }
}

fn load_bmp_from_bytes(data: &[u8]) -> Result<Image, Box<dyn std::error::Error>> {
    use crate::assets::converters::apply_magenta_transparency;
    use image::ImageFormat;

    // Use the image crate to decode BMP
    let img = image::load_from_memory_with_format(data, ImageFormat::Bmp)?;
    let rgba = img.to_rgba8();
    let dimensions = rgba.dimensions();

    // Get raw RGBA data and apply magenta transparency
    let mut rgba_data = rgba.into_raw();
    apply_magenta_transparency(&mut rgba_data);

    // Convert to Bevy Image
    let bevy_image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba_data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );

    Ok(bevy_image)
}
