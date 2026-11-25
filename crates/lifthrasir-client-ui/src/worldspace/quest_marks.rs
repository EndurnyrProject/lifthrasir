//! Quest marks over NPC heads: the retail quest icon floats over every NPC the
//! server marked, while that NPC is in view. The mark is rebuilt when the NPC
//! returns to view and replaced when its icon or color changes.

use bevy::prelude::*;
use lifthrasir_assets::paths::quest_mark_icon_path;
use lifthrasir_client::domain::entities::EntityRegistry;
use lifthrasir_client::domain::quest::QuestMarks;
use lifthrasir_ui::widgets::chrome::ignore_picking;

use crate::worldspace::{WorldAnchor, WorldOverlaySystems, raised_root};

/// The icon's on-screen size; the retail art is 28 x 28 px.
const MARK_WIDTH: f32 = 28.0;
/// Pixels from the NPC's origin up to the icon's bottom edge, tuned live.
const MARK_HEAD_GAP: f32 = 72.0;
/// Over nameplates (+100), under skill-cast labels (+160) and bubbles (+170).
const MARK_Z: i32 = super::WORLD_OVERLAY_Z + 150;

pub struct QuestMarkOverlayPlugin;

impl Plugin for QuestMarkOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_quest_mark_overlays.in_set(WorldOverlaySystems::Sync),
        );
    }
}

/// The over-head icon of one NPC's quest mark; its [`WorldAnchor`] names the NPC.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct QuestMarkOverlay {
    npc_id: u32,
    icon: u32,
    color: u32,
}

fn mark_icon(path: String) -> impl Scene {
    let content = bsn! {
        ImageNode { image: {path} }
        Node { width: px(MARK_WIDTH), height: px(MARK_WIDTH) }
        @ignore_picking()
    };
    raised_root(MARK_WIDTH, MARK_Z, content)
}

/// Keeps one overlay per quest mark whose NPC is in view and positioned. An
/// overlay whose mark is gone or changed look is despawned; [`follow_anchors`]
/// drops the ones whose NPC left, and they are rebuilt once it returns.
///
/// [`follow_anchors`]: super::follow_anchors
fn sync_quest_mark_overlays(
    marks: Res<QuestMarks>,
    registry: Res<EntityRegistry>,
    positioned: Query<(), With<GlobalTransform>>,
    overlays: Query<(Entity, &QuestMarkOverlay)>,
    mut commands: Commands,
) {
    let current = |overlay: &QuestMarkOverlay| {
        marks
            .get(overlay.npc_id)
            .is_some_and(|mark| (mark.icon, mark.color) == (overlay.icon, overlay.color))
    };
    for (entity, overlay) in &overlays {
        if !current(overlay) {
            commands.entity(entity).despawn();
        }
    }

    for (npc_id, mark) in marks.iter() {
        if overlays
            .iter()
            .any(|(_, overlay)| overlay.npc_id == npc_id && current(overlay))
        {
            continue;
        }
        let Some(target) = registry
            .get_entity(npc_id)
            .filter(|entity| positioned.contains(*entity))
        else {
            continue;
        };
        let Some(path) = quest_mark_icon_path(mark.icon, mark.color) else {
            continue;
        };
        commands.spawn_scene(mark_icon(path)).insert((
            WorldAnchor {
                target,
                width: MARK_WIDTH,
                drop: -MARK_HEAD_GAP,
            },
            QuestMarkOverlay {
                npc_id,
                icon: mark.icon,
                color: mark.color,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use super::*;
    use bevy::scene::ScenePlugin;
    use lifthrasir_assets::source::{CompositeAssetSource, register_ro_asset_source};
    use net_contract::events::QuestMarkChanged;

    const NPC: u32 = 7;

    fn app() -> App {
        let mut app = App::new();
        register_ro_asset_source(&mut app, Arc::new(RwLock::new(CompositeAssetSource::new())));
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>().init_asset::<Font>();
        app.init_resource::<QuestMarks>()
            .init_resource::<EntityRegistry>()
            .add_systems(Update, sync_quest_mark_overlays);
        app
    }

    fn register(app: &mut App, bundle: impl Bundle) -> Entity {
        let npc = app.world_mut().spawn(bundle).id();
        app.world_mut()
            .resource_mut::<EntityRegistry>()
            .register_entity(NPC, npc);
        npc
    }

    fn show(app: &mut App, icon: u32, color: u32) {
        app.world_mut()
            .resource_mut::<QuestMarks>()
            .apply(&QuestMarkChanged::Show {
                npc_id: NPC,
                x: 10,
                y: 20,
                icon,
                color,
            });
    }

    fn overlays(app: &mut App) -> Vec<(Entity, QuestMarkOverlay, WorldAnchor)> {
        let world = app.world_mut();
        world
            .query::<(Entity, &QuestMarkOverlay, &WorldAnchor)>()
            .iter(world)
            .map(|(entity, overlay, anchor)| (entity, *overlay, *anchor))
            .collect()
    }

    fn image_paths(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        world
            .query::<&ImageNode>()
            .iter(world)
            .filter_map(|image| {
                image
                    .image
                    .path()
                    .map(|path| path.path().display().to_string())
            })
            .collect()
    }

    #[test]
    fn no_overlay_until_the_npc_is_registered() {
        let mut app = app();
        show(&mut app, 1, 1);
        app.update();
        assert!(overlays(&mut app).is_empty());

        register(&mut app, GlobalTransform::default());
        app.update();
        assert_eq!(overlays(&mut app).len(), 1);
    }

    #[test]
    fn registered_npc_gets_one_overlay() {
        let mut app = app();
        let npc = register(&mut app, GlobalTransform::default());
        show(&mut app, 1, 0);
        app.update();
        app.update();

        let shown = overlays(&mut app);
        assert_eq!(shown.len(), 1);
        let (_, overlay, anchor) = shown[0];
        assert_eq!(
            overlay,
            QuestMarkOverlay {
                npc_id: NPC,
                icon: 1,
                color: 0
            }
        );
        assert_eq!(anchor.target, npc);
        assert_eq!(anchor.drop, -MARK_HEAD_GAP);
        let paths = image_paths(&mut app);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("minimap/quest_1_1.bmp"));
    }

    #[test]
    fn unpositioned_npc_gets_no_overlay() {
        let mut app = app();
        register(&mut app, ());
        show(&mut app, 1, 1);
        for _ in 0..3 {
            app.update();
        }
        assert!(overlays(&mut app).is_empty());
    }

    #[test]
    fn overlay_is_rebuilt_after_the_npc_returns() {
        let mut app = app();
        register(&mut app, GlobalTransform::default());
        show(&mut app, 1, 1);
        app.update();
        let (gone, ..) = overlays(&mut app)[0];
        app.world_mut().entity_mut(gone).despawn();

        app.update();

        let shown = overlays(&mut app);
        assert_eq!(shown.len(), 1);
        assert_ne!(shown[0].0, gone);
    }

    #[test]
    fn changed_icon_replaces_the_overlay() {
        let mut app = app();
        register(&mut app, GlobalTransform::default());
        show(&mut app, 1, 1);
        app.update();
        let (first, ..) = overlays(&mut app)[0];

        show(&mut app, 2, 3);
        app.update();

        let shown = overlays(&mut app);
        assert_eq!(shown.len(), 1);
        assert_ne!(shown[0].0, first);
        assert_eq!((shown[0].1.icon, shown[0].1.color), (2, 3));
        let paths = image_paths(&mut app);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("minimap/quest_2_3.bmp"));
    }

    #[test]
    fn cleared_mark_removes_the_overlay() {
        let mut app = app();
        register(&mut app, GlobalTransform::default());
        show(&mut app, 1, 1);
        app.update();
        assert_eq!(overlays(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<QuestMarks>()
            .apply(&QuestMarkChanged::Clear { npc_id: NPC });
        app.update();

        assert!(overlays(&mut app).is_empty());
        assert!(
            image_paths(&mut app).is_empty(),
            "the icon goes with its overlay"
        );
    }

    #[test]
    fn unknown_icon_gets_no_overlay() {
        let mut app = app();
        register(&mut app, GlobalTransform::default());
        show(&mut app, 11, 1);
        app.update();
        assert!(overlays(&mut app).is_empty());
    }
}
