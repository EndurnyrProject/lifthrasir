//! EF_WARPZONE2 (321) is a client effect, not an STR or a warp NPC sprite.
//! Ring and sparkle timing follows the ragnarok-offline EffectTable reference.

use std::{array, f32::consts::TAU};

use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, mesh::PrimitiveTopology, picking::Pickable,
    prelude::*,
};
use lifthrasir_runtime::{
    effects::procedural::VfxSystems,
    rendering::{Billboard, SharedSpriteQuad},
};

use crate::domain::{entities::markers::WarpZone, system_sets::EntityLifecycleSystems};

const WARP_ZONE_SCALE: f32 = 3.5;
const RING_TEXTURE: &str = "ro://data/texture/effect/ring_blue.tga";
const SPARKLE_TEXTURE: &str = "ro://data/texture/effect/pok1.tga";
// NOTE: Fixed offsets keep the repeating sparkles simple; vary them per cycle if the pattern is noticeable.
const SPARKLES: [(f32, f32, f32); 5] = [
    (-2.4, 0.8, 3.0),
    (1.2, -1.8, 3.7),
    (0.1, 2.3, 2.5),
    (2.5, 1.3, 3.4),
    (-0.8, -2.0, 2.0),
];

pub(super) struct WarpZonePlugin;

impl Plugin for WarpZonePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_assets).add_systems(
            Update,
            (attach, animate)
                .chain()
                .in_set(VfxSystems)
                .after(EntityLifecycleSystems::Spawning)
                .run_if(any_with_component::<WarpZone>),
        );
    }
}

#[derive(Resource)]
struct WarpZoneAssets {
    rings: [Handle<Mesh>; 2],
    ring_materials: [Handle<StandardMaterial>; 4],
    sparkle_materials: [Handle<StandardMaterial>; SPARKLES.len()],
}

#[derive(Component)]
enum WarpLayer {
    Ring(usize),
    Sparkle(usize),
}

fn load_assets(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ring = server.load(RING_TEXTURE);
    let sparkle = server.load(SPARKLE_TEXTURE);
    let mut additive = |texture: &Handle<Image>| {
        materials.add(StandardMaterial {
            base_color_texture: Some(texture.clone()),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            ..default()
        })
    };
    commands.insert_resource(WarpZoneAssets {
        rings: [(3.3, 2.0), (3.2, 1.9)].map(|(top, bottom)| meshes.add(ring_mesh(top, bottom))),
        ring_materials: array::from_fn(|_| additive(&ring)),
        sparkle_materials: array::from_fn(|_| additive(&sparkle)),
    });
}

/// An open cylinder wall so the original ring texture wraps once without caps.
fn ring_mesh(top: f32, bottom: f32) -> Mesh {
    const SEGMENTS: u32 = 32;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut normals = Vec::new();
    for side in 0..=SEGMENTS {
        let u = side as f32 / SEGMENTS as f32;
        let (sin, cos) = (TAU * u).sin_cos();
        positions.extend([
            [sin * top, -1.1, cos * top],
            [sin * bottom, 0.0, cos * bottom],
        ]);
        uvs.extend([[u, 0.0], [u, 1.0]]);
        normals.extend([[sin, 0.0, cos]; 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleStrip,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

fn attach(
    mut commands: Commands,
    portals: Query<Entity, Added<WarpZone>>,
    assets: Res<WarpZoneAssets>,
    quad: Res<SharedSpriteQuad>,
) {
    for portal in &portals {
        for (phase, material) in assets.ring_materials.iter().enumerate() {
            for mesh in &assets.rings {
                commands.spawn((
                    WarpLayer::Ring(phase),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(0.0, -0.03, 0.0),
                    ChildOf(portal),
                    NotShadowCaster,
                    Pickable::IGNORE,
                ));
            }
        }
        for (phase, material) in assets.sparkle_materials.iter().enumerate() {
            commands.spawn((
                WarpLayer::Sparkle(phase),
                Mesh3d(quad.mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_scale(Vec3::splat(50.0 / 35.0 * WARP_ZONE_SCALE)),
                Billboard,
                ChildOf(portal),
                NotShadowCaster,
                Pickable::IGNORE,
            ));
        }
    }
}

fn animate(
    time: Res<Time>,
    assets: Res<WarpZoneAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut layers: Query<(&WarpLayer, &mut Transform)>,
) {
    let time = time.elapsed_secs();
    let ring_progress =
        array::from_fn::<_, 4, _>(|phase| (time - phase as f32).rem_euclid(4.0) / 4.0);
    let sparkle_progress = array::from_fn::<_, { SPARKLES.len() }, _>(|phase| {
        (time - phase as f32 * 0.3).rem_euclid(1.0)
    });

    for (handle, progress) in assets.ring_materials.iter().zip(ring_progress) {
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color = Color::linear_rgba(0.5, 0.5, 1.0, 0.4 * fade(progress));
        }
    }
    for (handle, progress) in assets.sparkle_materials.iter().zip(sparkle_progress) {
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color = Color::linear_rgba(0.9, 1.0, 0.9, fade(progress));
        }
    }

    for (layer, mut transform) in &mut layers {
        match *layer {
            WarpLayer::Ring(phase) => {
                let progress = ring_progress[phase];
                let radius = 1.0 - progress;
                let height = 2.0 * progress.min(radius);
                transform.scale = Vec3::new(radius, height.max(0.001), radius) * WARP_ZONE_SCALE;
            }
            WarpLayer::Sparkle(phase) => {
                let (x, z, rise) = SPARKLES[phase];
                transform.translation =
                    Vec3::new(x, -rise * sparkle_progress[phase], z) * WARP_ZONE_SCALE;
            }
        }
    }
}

fn fade(progress: f32) -> f32 {
    (4.0 * progress.min(1.0 - progress)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::io::AssetSourceBuilder, time::TimeUpdateStrategy};
    use std::time::Duration;

    #[test]
    fn portal_rings_shrink_and_sparkles_rise_until_the_portal_despawns() {
        let mut app = App::new();
        app.register_asset_source("ro", AssetSourceBuilder::platform_default("assets", None))
            .add_plugins((MinimalPlugins, AssetPlugin::default(), WarpZonePlugin))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .insert_resource(SharedSpriteQuad {
                mesh: Handle::default(),
            })
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )));

        let portal = app.world_mut().spawn((WarpZone, Transform::default())).id();
        app.update();

        let children = app.world().get::<Children>(portal).unwrap();
        assert_eq!(children.len(), 13);
        let find_layer = |predicate: fn(&WarpLayer) -> bool| {
            children
                .iter()
                .find(|&child| app.world().get::<WarpLayer>(child).is_some_and(predicate))
                .unwrap()
        };
        let ring = find_layer(|layer| matches!(layer, WarpLayer::Ring(0)));
        let sparkle = find_layer(|layer| matches!(layer, WarpLayer::Sparkle(0)));
        let ring_before = app.world().get::<Transform>(ring).unwrap().scale.x;
        let sparkle_before = app.world().get::<Transform>(sparkle).unwrap().translation.y;
        app.update();
        assert!(app.world().get::<Transform>(ring).unwrap().scale.x < ring_before);
        assert!(app.world().get::<Transform>(sparkle).unwrap().translation.y < sparkle_before);

        app.world_mut().entity_mut(portal).despawn();
        assert!(app.world().get_entity(ring).is_err());
        assert!(app.world().get_entity(sparkle).is_err());
    }
}
