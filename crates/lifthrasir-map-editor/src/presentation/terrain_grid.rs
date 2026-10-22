//! Visual-terrain grid rendering and live appearance settings.
use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemParam,
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use std::collections::HashSet;

use crate::{application::EditorSystems, infrastructure::content::LifMapData};

const GRID_LIFT: f32 = 0.08;

/// Live appearance settings for the visual-terrain grid.
#[derive(Resource)]
pub(crate) struct TerrainGridStyle {
    pub line_width: f32,
    pub color: [f32; 3],
}

impl Default for TerrainGridStyle {
    fn default() -> Self {
        Self {
            line_width: 0.08,
            color: [0.2, 0.9, 1.0],
        }
    }
}

#[derive(Component)]
struct TerrainGrid;

/// Read-only scene access needed to distinguish terrain from prop meshes.
#[derive(SystemParam)]
struct TerrainScene<'w, 's> {
    roots: Query<'w, 's, Entity, With<LifMapData>>,
    children: Query<'w, 's, &'static Children>,
    names: Query<'w, 's, &'static Name>,
    meshes: Query<'w, 's, (&'static Mesh3d, &'static GlobalTransform)>,
}

pub(super) struct TerrainGridPlugin;

impl Plugin for TerrainGridPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainGridStyle>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    sync_terrain_grid,
                    toggle_terrain_grid.run_if(super::ui::viewport_input_allowed),
                )
                    .in_set(EditorSystems::Present),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((TerrainGrid, Visibility::Visible));
}

/// Build the grid from the converted terrain mesh rather than GAT collision
/// data. GAT is independent from visual terrain and can place an overlay
/// inside props or above another ground level.
fn sync_terrain_grid(
    mut commands: Commands,
    scene: TerrainScene,
    grids: Query<(Entity, Option<&Mesh3d>), With<TerrainGrid>>,
    style: Res<TerrainGridStyle>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let style_changed = style.is_changed();
    if grids.iter().all(|(_, mesh)| mesh.is_some()) && !style_changed {
        return;
    }
    let Some(terrain) = scene
        .roots
        .iter()
        .flat_map(|root| terrain_mesh_entities(root, &scene.children))
        .find(|&entity| {
            scene
                .names
                .get(entity)
                .is_ok_and(|name| name.as_str() == "terrain")
        })
    else {
        return;
    };
    let terrain_meshes = terrain_mesh_entities(terrain, &scene.children)
        .into_iter()
        .filter_map(|entity| scene.meshes.get(entity).ok())
        .filter_map(|(handle, transform)| meshes.get(&handle.0).map(|mesh| (mesh, transform)))
        .collect::<Vec<_>>();
    let Some(grid_mesh) = terrain_grid_mesh(terrain_meshes, style.line_width) else {
        return;
    };
    let mesh = meshes.add(grid_mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(style.color[0], style.color[1], style.color[2], 0.9),
        emissive: LinearRgba::rgb(
            style.color[0] * 0.25,
            style.color[1] * 0.25,
            style.color[2] * 0.25,
        ),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    for (entity, existing_mesh) in &grids {
        if existing_mesh.is_none() || style_changed {
            commands
                .entity(entity)
                .insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
        }
    }
}

fn terrain_mesh_entities(root: Entity, children: &Query<&Children>) -> Vec<Entity> {
    let mut entities = vec![root];
    let mut index = 0;
    while let Some(entity) = entities.get(index).copied() {
        if let Ok(entity_children) = children.get(entity) {
            entities.extend(entity_children.iter());
        }
        index += 1;
    }
    entities
}

/// Build ribbons instead of hardware lines so the grid has a consistent width
/// across renderers. Only horizontal and vertical edges of upward-facing
/// terrain triangles are retained: diagonals and vertical cliff faces vanish.
fn terrain_grid_mesh(sources: Vec<(&Mesh, &GlobalTransform)>, line_width: f32) -> Option<Mesh> {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut edges = HashSet::new();
    for (mesh, transform) in sources {
        add_terrain_edges(
            mesh,
            transform,
            &mut edges,
            &mut positions,
            &mut normals,
            &mut indices,
            line_width,
        );
    }
    (!positions.is_empty()).then(|| {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
    })
}

fn add_terrain_edges(
    mesh: &Mesh,
    transform: &GlobalTransform,
    seen: &mut HashSet<([i32; 3], [i32; 3])>,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    line_width: f32,
) {
    let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let source_indices: Vec<usize> = match mesh.indices() {
        Some(Indices::U16(indices)) => indices.iter().map(|&index| index as usize).collect(),
        Some(Indices::U32(indices)) => indices.iter().map(|&index| index as usize).collect(),
        None => (0..vertices.len()).collect(),
    };
    for &[a, b, c] in source_indices.as_chunks::<3>().0 {
        let (Some(a), Some(b), Some(c)) = (vertices.get(a), vertices.get(b), vertices.get(c))
        else {
            continue;
        };
        let (a, b, c) = (
            transform.transform_point(Vec3::from(*a)),
            transform.transform_point(Vec3::from(*b)),
            transform.transform_point(Vec3::from(*c)),
        );
        if (b - a).cross(c - a).y <= 0.001 {
            continue;
        }
        for (start, end) in [(a, b), (b, c), (c, a)] {
            if (start.x - end.x).abs() > 0.001 && (start.z - end.z).abs() > 0.001 {
                continue;
            }
            add_grid_edge(seen, positions, normals, indices, line_width, start, end);
        }
    }
}

fn add_grid_edge(
    seen: &mut HashSet<([i32; 3], [i32; 3])>,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    line_width: f32,
    start: Vec3,
    end: Vec3,
) {
    let key = ordered_edge_key(start, end);
    if !seen.insert(key) {
        return;
    }
    let direction = (end - start).xz().normalize();
    let offset = Vec3::new(-direction.y, 0.0, direction.x) * line_width;
    let base = positions.len() as u32;
    let start = start + Vec3::Y * GRID_LIFT;
    let end = end + Vec3::Y * GRID_LIFT;
    positions.extend([
        (start - offset).to_array(),
        (start + offset).to_array(),
        (end - offset).to_array(),
        (end + offset).to_array(),
    ]);
    normals.extend([[0.0, 1.0, 0.0]; 4]);
    indices.extend([base, base + 2, base + 3, base, base + 3, base + 1]);
}

fn ordered_edge_key(start: Vec3, end: Vec3) -> ([i32; 3], [i32; 3]) {
    let point_key = |point: Vec3| (point * 1_000.0).round().as_ivec3().to_array();
    let (start, end) = (point_key(start), point_key(end));
    (start.min(end), start.max(end))
}

fn toggle_terrain_grid(
    keys: Res<ButtonInput<KeyCode>>,
    mut grids: Query<&mut Visibility, With<TerrainGrid>>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    for mut visibility in &mut grids {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            Visibility::Inherited | Visibility::Visible => Visibility::Hidden,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_grid_deduplicates_reversed_edges() {
        let a = Vec3::new(5.0, 2.0, -10.0);
        let b = Vec3::new(10.0, 3.0, -10.0);
        assert_eq!(ordered_edge_key(a, b), ordered_edge_key(b, a));
    }

    #[test]
    fn terrain_grid_uses_only_top_face_cell_edges() {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [0.0, 0.0, 0.0],
                [5.0, 0.0, 0.0],
                [5.0, 2.0, -5.0],
                [0.0, 2.0, -5.0],
            ],
        )
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        let transform = GlobalTransform::default();

        let grid = terrain_grid_mesh(vec![(&mesh, &transform)], 0.08).expect("terrain grid");
        let Some(VertexAttributeValues::Float32x3(positions)) =
            grid.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("grid positions");
        };
        assert_eq!(positions.len(), 16, "four cell edges, not the diagonal");
    }

    #[test]
    fn terrain_grid_toggle_is_an_in_memory_viewport_action() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, toggle_terrain_grid);
        let grid = app
            .world_mut()
            .spawn((TerrainGrid, Visibility::Visible))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(grid),
            Some(&Visibility::Hidden)
        );
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(grid),
            Some(&Visibility::Visible)
        );
    }
}
