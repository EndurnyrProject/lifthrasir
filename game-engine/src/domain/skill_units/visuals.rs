//! Persistent visuals for ground-skill units.
//!
//! Visuals are attached as effect children of the group root (for a
//! `GroundAnchor::Group` skill like Storm Gust) or of each visible cell (for a
//! `GroundAnchor::Cell` skill like Ice Wall). They are children rather than
//! entity-anchored effects because `EffectAnchor::Entity` only follows the
//! anchor's transform and does not tear down when the anchor dies; parenting
//! makes the recursive group despawn remove every visual, which is the invariant.

use bevy::mesh::ConeAnchor;
use bevy::prelude::*;
use lifthrasir_data::EffectDescriptor;

use crate::domain::effects::components::EffectAnchor;
use crate::domain::effects::spawn_effect;
use crate::domain::effects::triggers::{descriptor_tint, load_effect};

/// The classic client's translucent frosted-ice texture (`ice.tga`) — the same
/// one it wraps around its hardcoded Ice Wall pillars. Its own alpha channel
/// (~0.8 average) provides the translucency under `AlphaMode::Blend`.
const CRYSTAL_TEXTURE: &str = "ro://data/texture/effect/ice.tga";
/// Main spike of the per-cell crystal cluster: base radius and height in world
/// units (a grid cell is 5.0 across, a character ~5.0 tall).
const CRYSTAL_RADIUS: f32 = 12.0;
const CRYSTAL_HEIGHT: f32 = 40.0;
/// How far the cluster is sunk into the ground (world down is `+Y`) so the cone
/// bases sit buried instead of hovering at the cell origin.
const CRYSTAL_SINK: f32 = 10.0;
/// Facet count per spike, low so the cones read as cut crystal, not smooth horns.
const CRYSTAL_FACETS: u32 = 6;
/// Side shards: scale relative to the main spike, horizontal offset from the
/// cell center, and outward lean in radians.
const CRYSTAL_SHARD_SCALE: f32 = 0.55;
const CRYSTAL_SHARD_OFFSET: f32 = 9.0;
const CRYSTAL_SHARD_TILT: f32 = 0.28;

/// Spawn the descriptor's persistent visual as a child of `parent` (root or
/// cell), at the parent's origin. Preference order:
///   1. an STR effect when the descriptor has one;
///   2. otherwise, for a `vfx`-only descriptor (e.g. Ice Wall, which the classic
///      client hardcodes and no STR exists for), a persistent ice-crystal
///      cluster tinted by the descriptor — `PlayProceduralVfx` is fire-and-forget
///      and cannot back a persistent wall cell, so the wall lives here as a child
///      that despawns with the group/cell.
///
/// A descriptor with neither spawns nothing; the group is still
/// gameplay-relevant, so that is not an error.
pub(super) fn spawn_effect_child(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    descriptor: &EffectDescriptor,
    parent: Entity,
) {
    if let Some(handle) = load_effect(asset_server, descriptor) {
        let effect = spawn_effect(
            commands,
            handle,
            EffectAnchor::Position(Vec3::ZERO),
            descriptor.repeating,
            descriptor_tint(descriptor),
            None,
        );
        commands.entity(effect).insert(ChildOf(parent));
        return;
    }

    if descriptor.vfx.is_some() {
        spawn_vfx_crystal(
            commands,
            asset_server,
            meshes,
            materials,
            descriptor,
            parent,
        );
    }
}

/// Spawn the persistent visual for a `vfx`-only ground descriptor: a faceted
/// ice-crystal cluster — one tall spike plus three shorter shards merged into a
/// single mesh — wrapped in the classic client's `ice.tga` and tinted by the
/// descriptor. The parent entity index seeds a per-cell yaw so adjacent wall
/// cells don't read as identical stamps; child hierarchy handles teardown when
/// the group/cell despawns.
fn spawn_vfx_crystal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    descriptor: &EffectDescriptor,
    parent: Entity,
) {
    let spike = |radius: f32, height: f32| {
        Mesh::from(
            Cone::new(radius, height)
                .mesh()
                .resolution(CRYSTAL_FACETS)
                .anchor(ConeAnchor::Base),
        )
    };

    let yaw = parent.index_u32() as f32 * 2.399;
    let mut mesh = spike(CRYSTAL_RADIUS, CRYSTAL_HEIGHT).rotated_by(Quat::from_rotation_y(yaw));
    for (i, angle) in [0.9f32, 3.1, 5.2].into_iter().enumerate() {
        let dir = Vec3::new((yaw + angle).cos(), 0.0, (yaw + angle).sin());
        let height = CRYSTAL_HEIGHT * CRYSTAL_SHARD_SCALE * (1.0 + 0.15 * i as f32);
        let shard = spike(CRYSTAL_RADIUS * CRYSTAL_SHARD_SCALE, height).transformed_by(
            Transform::from_translation(dir * CRYSTAL_SHARD_OFFSET).with_rotation(
                Quat::from_axis_angle(Vec3::Y.cross(dir), CRYSTAL_SHARD_TILT),
            ),
        );
        mesh.merge(&shard)
            .expect("cone meshes share one attribute layout");
    }
    // World up is -Y: the cones are built along +Y with their bases at the cell
    // origin, so flip the finished cluster to stand it upright on the ground.
    // The sink is baked into the vertices, not the entity transform, so the
    // spawned child stays at the cell origin.
    let mesh = mesh
        .rotated_by(Quat::from_rotation_x(std::f32::consts::PI))
        .translated_by(Vec3::Y * CRYSTAL_SINK);

    let material = materials.add(StandardMaterial {
        base_color: descriptor_tint(descriptor),
        base_color_texture: Some(asset_server.load(CRYSTAL_TEXTURE)),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::default(),
        // The visual must never swallow or block picking rays: the cell's flat
        // click collider (spawn::spawn_cell_collider) is the only click target.
        Pickable::IGNORE,
        ChildOf(parent),
    ));
}
