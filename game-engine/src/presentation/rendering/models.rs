use crate::domain::entities::systems::{
    AnimatedTransform, AnimationType, RsmAnimationController, RsmNodeAnimation,
};
use crate::domain::system_sets::ModelRenderingSystems;
use crate::domain::world::components::MapLoader;
use crate::infrastructure::assets::loaders::{RoGroundAsset, RoWorldAsset, RsmAsset};
use crate::infrastructure::ro_formats::{RsmFile, RswObject};
use crate::utils::{get_map_dimensions_from_ground, rsw_to_bevy_transform};
use bevy::asset::RenderAssetUsages;
use bevy::math::{Mat4, Vec4};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_auto_plugin::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Component)]
pub struct MapModel {
    pub filename: String,
    pub node_name: String,
}

#[derive(Component)]
pub struct ModelProcessed;

#[derive(Component)]
pub struct ModelsSpawned;

#[derive(Component)]
pub struct AnimationSpeed(pub f32);

/// Component to track RSM asset handle during async loading
#[derive(Component)]
pub struct RsmLoading {
    pub handle: Handle<RsmAsset>,
}

/// Component to track RSM model textures that are loading
#[derive(Component)]
pub struct ModelTexturesLoading {
    pub texture_handles: Vec<Handle<Image>>,
    pub texture_names: Vec<String>,
    pub rsm: Arc<RsmFile>,
    pub node_meshes: HashMap<usize, Vec<(i32, Mesh)>>,
    pub node_entities: Vec<Option<Entity>>,
    pub anim_type: Option<AnimationType>,
    pub anim_speed: f32,
}

/// Component to mark and identify RSM node entities
#[derive(Component, Debug)]
pub struct RsmNode {
    pub index: usize,
    pub name: String,
}

/// Type alias for the model mesh update query to improve readability
type ModelMeshUpdateQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MapModel,
        &'static RsmLoading,
        Option<&'static AnimationType>,
        Option<&'static AnimationSpeed>,
    ),
    (
        With<MapModel>,
        Without<ModelProcessed>,
        Without<ModelTexturesLoading>,
    ),
>;

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelLoading)
)]
pub fn log_loaded_world_data(
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<&MapLoader, Changed<MapLoader>>,
) {
    for map_loader in query.iter() {
        if let Some(world) = map_loader.world.as_ref() {
            if let Some(world_asset) = world_assets.get(world) {
                let model_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Model(_)))
                    .count();
                let light_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Light(_)))
                    .count();
                let sound_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Sound(_)))
                    .count();
                let effect_count = world_asset
                    .world
                    .objects
                    .iter()
                    .filter(|o| matches!(o, RswObject::Effect(_)))
                    .count();

                info!("World data loaded:");
                info!("  Version: {}", world_asset.world.version);
                info!("  GND file: {}", world_asset.world.gnd_file);
                info!("  GAT file: {}", world_asset.world.gat_file);
                info!("  Water level: {}", world_asset.world.water.level);
                info!("  Total objects: {}", world_asset.world.objects.len());
                info!("    Models: {}", model_count);
                info!("    Lights: {}", light_count);
                info!("    Sounds: {}", sound_count);
                info!("    Effects: {}", effect_count);
            }
        }

        if let Some(ground_asset) = ground_assets.get(&map_loader.ground) {
            info!("Ground data loaded:");
            info!(
                "  Size: {}x{}",
                ground_asset.ground.width, ground_asset.ground.height
            );
            info!("  Textures: {}", ground_asset.ground.textures.len());
        }
    }
}

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelLoading)
)]
pub fn spawn_map_models(
    mut commands: Commands,
    world_assets: Res<Assets<RoWorldAsset>>,
    ground_assets: Res<Assets<RoGroundAsset>>,
    query: Query<(Entity, &MapLoader), Without<ModelsSpawned>>,
) {
    for (entity, map_loader) in query.iter() {
        let Some(world_handle) = &map_loader.world else {
            continue;
        };

        let Some(world_asset) = world_assets.get(world_handle) else {
            continue;
        };

        let Some(ground_asset) = ground_assets.get(&map_loader.ground) else {
            continue;
        };

        let (map_width, map_height) = get_map_dimensions_from_ground(&ground_asset.ground);

        let mut model_groups: HashMap<String, Vec<(Transform, String, AnimationType, f32)>> =
            HashMap::new();
        let mut empty_count = 0;

        for obj in &world_asset.world.objects {
            if let RswObject::Model(model) = obj {
                if model.filename.is_empty() {
                    empty_count += 1;
                    if empty_count <= 5 {
                        debug!(
                            "Empty filename model #{}: name='{}', node='{}', pos={:?}",
                            empty_count, model.name, model.node_name, model.position
                        );
                    }
                }

                let transform = rsw_to_bevy_transform(model, map_width, map_height);

                // Convert RSW animation type to our enum
                // Most RO models should loop by default for continuous animation
                let anim_type = match model.anim_type {
                    0 => AnimationType::None, // Explicitly no animation
                    1 => AnimationType::Loop, // Loop animation
                    2 => AnimationType::Loop, // Default to Loop instead of Once to prevent stopping
                    _ => {
                        // Default to Loop for any unknown animation types
                        AnimationType::Loop
                    }
                };

                model_groups
                    .entry(model.filename.clone())
                    .or_default()
                    .push((
                        transform,
                        model.node_name.clone(),
                        anim_type,
                        model.anim_speed,
                    ));
            }
        }

        if empty_count > 0 {
            warn!("{} models have empty filenames", empty_count);
        }

        for (filename, instances) in model_groups {
            for (transform, node_name, anim_type, anim_speed) in instances {
                let model_entity = commands
                    .spawn((
                        Transform::from_translation(transform.translation)
                            .with_rotation(transform.rotation)
                            .with_scale(transform.scale),
                        GlobalTransform::default(),
                        Visibility::default(),
                        ViewVisibility::default(),
                        InheritedVisibility::default(),
                        MapModel {
                            filename: filename.clone(),
                            node_name: node_name.clone(),
                        },
                    ))
                    .id();

                // Store animation data for later processing in update_model_meshes
                if anim_type != AnimationType::None {
                    commands
                        .entity(model_entity)
                        .insert((anim_type, AnimationSpeed(anim_speed)));
                }
            }
        }

        // Mark this MapLoader entity as having models spawned
        commands.entity(entity).insert(ModelsSpawned);
    }
}

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelLoading)
)]
pub fn load_rsm_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &MapModel), Without<RsmLoading>>,
) {
    for (entity, map_model) in query.iter() {
        if map_model.filename.is_empty() {
            continue;
        }

        let rsm_path = format!("ro://data\\model\\{}", map_model.filename);
        let rsm_handle: Handle<RsmAsset> = asset_server.load(&rsm_path);

        commands
            .entity(entity)
            .insert(RsmLoading { handle: rsm_handle });
    }
}

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelMeshUpdate)
)]
pub fn update_model_meshes(
    mut commands: Commands,
    model_query: ModelMeshUpdateQuery,
    asset_server: Res<AssetServer>,
    rsm_assets: Res<Assets<RsmAsset>>,
) {
    for (entity, map_model, rsm_loading, anim_type, anim_speed) in model_query.iter() {
        if map_model.filename.is_empty() {
            continue;
        }

        // Get RSM from loaded assets
        let Some(rsm_asset) = rsm_assets.get(&rsm_loading.handle) else {
            continue; // Still loading
        };
        let rsm = Arc::new(rsm_asset.model.clone());

        let node_meshes = convert_rsm_to_mesh(&rsm);

        // Create entity hierarchy: Model -> Node Entities -> Mesh Children
        let mut node_entities = vec![None; rsm.nodes.len()];

        // First pass: Create all node entities with their transforms
        for (node_idx, node) in rsm.nodes.iter().enumerate() {
            let node_transform = rsm_node_to_bevy_transform(&rsm, node, node_idx);

            let node_entity_commands = commands.spawn((
                node_transform,
                GlobalTransform::default(),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                RsmNode {
                    index: node_idx,
                    name: node.name.clone(),
                },
            ));

            // Store node entity ID for animation setup
            let node_entity_id = node_entity_commands.id();

            node_entities[node_idx] = Some(node_entity_id);

            // Create animation components if this node has keyframes and model has animation
            if let Some(&anim_type_value) = anim_type.filter(|_| node_has_animation(node)) {
                let speed = anim_speed.map(|s| s.0).unwrap_or(1.0);

                // Create base transform from current node transform
                let base_transform = AnimatedTransform::from_transform(&node_transform);

                // Create node animation component
                let node_animation = RsmNodeAnimation::new(
                    node.pos_keyframes.clone(),
                    node.rot_keyframes.clone(),
                    base_transform,
                    rsm.anim_len,
                );

                // Create animation controller
                let mut controller = RsmAnimationController::new();
                controller.play(anim_type_value);
                controller.set_speed(speed);
                // RSM anim_len is duration in milliseconds, not FPS
                // Use standard RO animation frame rate: 25 FPS
                let fps = 25.0; // Standard Ragnarok Online frame rate
                controller.set_frame_rate(fps);

                // Add animation components to node entity
                commands
                    .entity(node_entity_id)
                    .insert((node_animation, controller));
            }
        }

        // Second pass: Set up parent-child relationships between nodes
        for (node_idx, node) in rsm.nodes.iter().enumerate() {
            let node_entity = node_entities[node_idx].unwrap();

            // Find and set parent
            if let Some(parent_idx) = find_parent_node_index(&rsm, node) {
                if let Some(parent_entity) = node_entities[parent_idx] {
                    commands.entity(parent_entity).add_child(node_entity);
                } else {
                    // Parent not found, attach to model entity
                    commands.entity(entity).add_child(node_entity);
                    debug!(
                        "Parent '{}' not found for node '{}', attaching to model",
                        node.parent_name, node.name
                    );
                }
            } else {
                // Root node - attach directly to model entity
                commands.entity(entity).add_child(node_entity);
            }
        }

        // Third pass: Load textures asynchronously
        let mut texture_handles = Vec::new();
        let mut texture_names = Vec::new();

        for texture_name in rsm.textures.iter() {
            if !texture_name.is_empty() {
                let texture_path = format!("ro://data\\texture\\{}", texture_name);
                let handle: Handle<Image> = asset_server.load(&texture_path);
                texture_handles.push(handle);
                texture_names.push(texture_name.clone());
            } else {
                // Empty texture name - add default handle
                texture_handles.push(Handle::default());
                texture_names.push(String::new());
            }
        }

        // Add component to track texture loading
        // Materials and mesh spawning will happen in create_model_materials_when_textures_ready
        commands.entity(entity).insert(ModelTexturesLoading {
            texture_handles,
            texture_names,
            rsm,
            node_meshes,
            node_entities,
            anim_type: anim_type.copied(),
            anim_speed: anim_speed.map(|s| s.0).unwrap_or(1.0),
        });
    }
}

#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelMaterialUpdate)
)]
pub fn create_model_materials_when_textures_ready(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &ModelTexturesLoading)>,
) {
    use bevy::asset::LoadState;

    for (entity, textures_loading) in query.iter() {
        // Check if all textures are loaded or failed
        let mut all_ready = true;
        let mut loaded_count = 0;
        let mut failed_count = 0;

        for (i, handle) in textures_loading.texture_handles.iter().enumerate() {
            if handle.id() == AssetId::default() {
                continue; // Empty texture name
            }

            match asset_server.load_state(handle) {
                LoadState::Loaded => {
                    loaded_count += 1;
                }
                LoadState::Failed(err) => {
                    debug!(
                        "Failed to load RSM texture '{}': {:?}",
                        textures_loading
                            .texture_names
                            .get(i)
                            .unwrap_or(&"unknown".to_string()),
                        err
                    );
                    failed_count += 1;
                }
                LoadState::Loading | LoadState::NotLoaded => {
                    all_ready = false;
                    break; // Wait for all textures
                }
            }
        }

        if !all_ready {
            continue; // Not ready yet, check next frame
        }

        debug!(
            "All RSM textures ready for model - loaded: {}, failed: {}, total: {}",
            loaded_count,
            failed_count,
            textures_loading.texture_handles.len()
        );

        // Create materials from loaded textures
        let texture_materials = create_model_materials_from_loaded_textures(
            &textures_loading.rsm,
            &textures_loading.texture_handles,
            &textures_loading.texture_names,
            &asset_server,
            &mut materials,
        );

        // Spawn mesh children with materials
        for (node_idx, node_mesh_list) in &textures_loading.node_meshes {
            let node_entity = textures_loading.node_entities[*node_idx].unwrap();

            for (texture_id, mesh) in node_mesh_list {
                let mesh_handle = meshes.add(mesh.clone());

                // Get material for this texture ID
                let material_handle =
                    texture_materials
                        .get(texture_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            debug!(
                                "No material found for texture ID {}, using fallback",
                                texture_id
                            );
                            create_colored_fallback_material_for_model(
                                *texture_id as usize,
                                &mut materials,
                            )
                        });

                // Create mesh entity with local space transform (IDENTITY)
                let mesh_entity = commands
                    .spawn((
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(material_handle),
                        Transform::IDENTITY, // Local space - let Bevy handle hierarchy transforms
                        GlobalTransform::default(),
                    ))
                    .id();

                // Add mesh as child of its node
                commands.entity(node_entity).add_child(mesh_entity);
            }
        }

        // Mark as processed and remove loading component
        commands
            .entity(entity)
            .insert(ModelProcessed)
            .remove::<ModelTexturesLoading>();
    }
}

fn create_model_materials_from_loaded_textures(
    rsm: &RsmFile,
    texture_handles: &[Handle<Image>],
    _texture_names: &[String],
    asset_server: &AssetServer,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> HashMap<i32, Handle<StandardMaterial>> {
    use bevy::asset::LoadState;

    let mut material_map = HashMap::new();

    for (i, _texture_name) in rsm.textures.iter().enumerate() {
        let material = if i < texture_handles.len() && texture_handles[i].id() != AssetId::default()
        {
            match asset_server.load_state(&texture_handles[i]) {
                LoadState::Loaded => {
                    materials.add(StandardMaterial {
                        base_color_texture: Some(texture_handles[i].clone()),
                        base_color: Color::WHITE,
                        // Use Mask with very low threshold (0.01) to cut off transparent pixels
                        // This avoids Blend mode's depth sorting issues while preventing magenta bleeding
                        alpha_mode: if rsm.alpha < 1.0 {
                            AlphaMode::Blend
                        } else {
                            AlphaMode::Mask(0.01)
                        },
                        perceptual_roughness: 0.8,
                        metallic: 0.0,
                        reflectance: 0.1,
                        cull_mode: None,
                        ..default()
                    })
                }
                _ => create_colored_fallback_material_for_model(i, materials),
            }
        } else {
            create_colored_fallback_material_for_model(i, materials)
        };

        material_map.insert(i as i32, material);
    }

    material_map
}

fn create_colored_fallback_material_for_model(
    index: usize,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Handle<StandardMaterial> {
    let color = match index % 10 {
        0 => Color::srgb(0.8, 0.6, 0.4), // Brown
        1 => Color::srgb(0.4, 0.8, 0.4), // Green
        2 => Color::srgb(0.4, 0.4, 0.8), // Blue
        3 => Color::srgb(0.8, 0.8, 0.4), // Yellow
        4 => Color::srgb(0.8, 0.4, 0.8), // Magenta
        5 => Color::srgb(0.4, 0.8, 0.8), // Cyan
        6 => Color::srgb(0.8, 0.4, 0.4), // Red
        7 => Color::srgb(0.6, 0.6, 0.8), // Light Blue
        8 => Color::srgb(0.6, 0.8, 0.6), // Light Green
        9 => Color::srgb(0.8, 0.6, 0.6), // Light Red
        _ => Color::srgb(0.7, 0.7, 0.7), // Grey
    };

    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.8,
        metallic: 0.0,
        reflectance: 0.1,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    })
}

fn mat3_to_mat4(mat3: &[f32; 9]) -> Mat4 {
    // Convert 3x3 matrix to 4x4 format (column-major order)
    Mat4::from_cols(
        Vec4::new(mat3[0], mat3[1], mat3[2], 0.0),
        Vec4::new(mat3[3], mat3[4], mat3[5], 0.0),
        Vec4::new(mat3[6], mat3[7], mat3[8], 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

/// Returns a map of node index -> vec of (texture_id, mesh) pairs
fn convert_rsm_to_mesh(rsm: &RsmFile) -> HashMap<usize, Vec<(i32, Mesh)>> {
    let mut node_meshes = HashMap::new();

    for (idx, node) in rsm.nodes.iter().enumerate() {
        let meshes = extract_node_meshes(rsm, node);
        if !meshes.is_empty() {
            node_meshes.insert(idx, meshes);
        }
    }

    // If no meshes were generated at all, create a fallback for the main node
    if node_meshes.is_empty() {
        debug!("No meshes generated from RSM nodes, using fallback cube");
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let main_node_idx = rsm
            .nodes
            .iter()
            .position(|n| n.name == rsm.main_node_name)
            .unwrap_or(0);
        node_meshes.insert(main_node_idx, vec![(-1, mesh)]);
    }

    node_meshes
}

fn extract_node_meshes(
    rsm: &RsmFile,
    node: &crate::infrastructure::ro_formats::rsm::Node,
) -> Vec<(i32, Mesh)> {
    if node.vertices.is_empty() || node.faces.is_empty() {
        return Vec::new();
    }

    let is_only = rsm.nodes.len() == 1;

    let mut transform = Mat4::IDENTITY;

    // 1. Apply offset (only if not is_only) - following RoBrowser pattern
    if !is_only {
        let offset_trans = Mat4::from_translation(Vec3::from_array(node.offset));
        transform *= offset_trans;
    }

    // 2. Apply mat3 transformation - local node transform matrix
    let mat3_as_mat4 = mat3_to_mat4(&node.mat3);
    transform *= mat3_as_mat4;

    // Apply transformations to vertices (keeping them in local node space)
    let mut transformed_vertices: Vec<[f32; 3]> = Vec::with_capacity(node.vertices.len());
    for vertex in &node.vertices {
        // Transform vertex by local node matrix only
        let v = Vec4::new(vertex[0], vertex[1], vertex[2], 1.0);
        let transformed = transform * v;

        // Store in local space (no Y-flip or global transforms here)
        let local_v = Vec3::new(transformed.x, transformed.y, transformed.z);
        transformed_vertices.push(local_v.to_array());
    }

    generate_meshes_from_vertices_and_faces(node, &transformed_vertices)
}

fn generate_meshes_from_vertices_and_faces(
    node: &crate::infrastructure::ro_formats::rsm::Node,
    transformed_vertices: &[[f32; 3]],
) -> Vec<(i32, Mesh)> {
    // Group faces by texture
    let mut faces_by_texture: HashMap<i32, Vec<usize>> = HashMap::new();
    for (idx, face) in node.faces.iter().enumerate() {
        let actual_texture_idx = if (face.tex_id as usize) < node.texture_ids.len() {
            node.texture_ids[face.tex_id as usize]
        } else {
            -1 // Invalid texture
        };
        faces_by_texture
            .entry(actual_texture_idx)
            .or_default()
            .push(idx);
    }

    // Calculate face normals
    let mut face_normals = Vec::with_capacity(node.faces.len());
    for face in &node.faces {
        let v1_idx = face.vertex_ids[0] as usize;
        let v2_idx = face.vertex_ids[1] as usize;
        let v3_idx = face.vertex_ids[2] as usize;

        if v1_idx >= node.vertices.len()
            || v2_idx >= node.vertices.len()
            || v3_idx >= node.vertices.len()
        {
            face_normals.push([0.0, 1.0, 0.0]); // Default up normal for invalid faces
            continue;
        }

        let v1 = Vec3::from(transformed_vertices[v1_idx]);
        let v2 = Vec3::from(transformed_vertices[v2_idx]);
        let v3 = Vec3::from(transformed_vertices[v3_idx]);

        let edge1 = v2 - v1;
        let edge2 = v3 - v1;
        let normal = edge1.cross(edge2).normalize_or_zero();
        face_normals.push([normal.x, normal.y, normal.z]);
    }

    let mut result = Vec::new();

    // Generate mesh for each texture
    for (actual_texture_idx, face_indices) in faces_by_texture {
        let mut final_positions: Vec<[f32; 3]> = Vec::new();
        let mut final_normals: Vec<[f32; 3]> = Vec::new();
        let mut final_uvs: Vec<[f32; 2]> = Vec::new();
        let mut final_indices: Vec<u32> = Vec::new();

        // Map to track unique vertex combinations per texture
        let mut vertex_map: HashMap<(u16, u16), u32> = HashMap::new();

        for &face_idx in &face_indices {
            let face = &node.faces[face_idx];
            let face_normal = face_normals[face_idx];

            for i in 0..3 {
                let pos_idx = face.vertex_ids[i];
                let uv_idx = face.texture_vertex_ids[i];

                if pos_idx as usize >= node.vertices.len() {
                    continue;
                }

                let vertex_key = (pos_idx, uv_idx);

                if let Some(&existing_idx) = vertex_map.get(&vertex_key) {
                    final_indices.push(existing_idx);
                } else {
                    let position = transformed_vertices[pos_idx as usize];

                    let uv = if (uv_idx as usize) < node.texture_vertices.len() {
                        let tex_vert = &node.texture_vertices[uv_idx as usize];
                        [tex_vert.u, tex_vert.v]
                    } else {
                        [0.0, 0.0]
                    };

                    let new_idx = final_positions.len() as u32;
                    final_positions.push(position);
                    final_uvs.push(uv);
                    final_normals.push(face_normal);

                    final_indices.push(new_idx);
                    vertex_map.insert(vertex_key, new_idx);
                }
            }

            // Correct winding order for the last triangle added
            let idx_count = final_indices.len();
            if idx_count >= 3 && idx_count.is_multiple_of(3) {
                final_indices.swap(idx_count - 2, idx_count - 1);
            }
        }

        if final_positions.is_empty() {
            continue;
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, final_positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, final_normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, final_uvs);
        mesh.insert_indices(Indices::U32(final_indices));

        result.push((actual_texture_idx, mesh));
    }

    if result.is_empty() {
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        vec![(-1, mesh)]
    } else {
        result
    }
}

/// Convert RSM node properties to Bevy Transform (local space)
/// This handles the node's local position, rotation, and scale
fn rsm_node_to_bevy_transform(
    rsm: &RsmFile,
    node: &crate::infrastructure::ro_formats::rsm::Node,
    node_idx: usize,
) -> Transform {
    let mut transform = Transform::from_translation(Vec3::from_array(node.pos));

    // Apply rotation if present
    if node.rot_angle != 0.0 {
        let axis = Vec3::from_array(node.rot_axis);
        if axis.length() > 0.0 {
            let normalized_axis = axis.normalize();
            transform.rotation = Quat::from_axis_angle(normalized_axis, node.rot_angle);
        }
    }

    // Apply scale
    transform.scale = Vec3::from_array(node.scale);

    // Special handling for the main node: apply bounding box adjustment
    // This maintains the coordinate system setup with NEG_Y camera
    let is_main_node = node.name == rsm.main_node_name || node_idx == 0;
    if is_main_node {
        if let Some(ref bbox) = rsm.bounding_box {
            // Apply bounding box translation to the main node's transform
            // This replaces the global bbox transform that was applied to all vertices
            let bbox_offset = Vec3::new(-bbox.center[0], -bbox.max[1], -bbox.center[2]);
            transform.translation += bbox_offset;
        }
    }

    transform
}

/// Find the parent node index for a given node in the RSM hierarchy
fn find_parent_node_index(
    rsm: &RsmFile,
    node: &crate::infrastructure::ro_formats::rsm::Node,
) -> Option<usize> {
    if node.parent_name.is_empty() || node.parent_name == node.name {
        return None; // Root node or self-referencing
    }

    rsm.nodes.iter().position(|n| n.name == node.parent_name)
}

/// Helper to check if a node has animation keyframes
fn node_has_animation(node: &crate::infrastructure::ro_formats::rsm::Node) -> bool {
    !node.pos_keyframes.is_empty() || !node.rot_keyframes.is_empty()
}

/// Update RSM animation components each frame
#[auto_add_system(
    plugin = crate::app::map_domain_plugin::MapDomainPlugin,
    schedule = Update,
    config(in_set = ModelRenderingSystems::ModelAnimation)
)]
pub fn update_rsm_animations(
    mut node_query: Query<(
        &mut Transform,
        &mut RsmAnimationController,
        &RsmNodeAnimation,
    )>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();

    for (mut transform, mut controller, animation) in node_query.iter_mut() {
        if !controller.is_playing || controller.anim_type == AnimationType::None {
            continue;
        }

        // Update animation frame using RSM animation length in milliseconds
        controller.update_frame(delta_time, animation.duration_frames);

        // Apply keyframe interpolation to transform
        let current_frame = controller.current_frame;

        // Get interpolated position and rotation from animation data
        let new_position = animation.get_position_at_frame(current_frame);
        let new_rotation = animation.get_rotation_at_frame(current_frame);

        // Only update transform if values changed to avoid triggering expensive
        // GlobalTransform propagation through the entire hierarchy on every frame
        if transform.translation != new_position {
            transform.translation = new_position;
        }
        if transform.rotation != new_rotation {
            transform.rotation = new_rotation;
        }
    }
}
