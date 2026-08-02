pub mod backdrop;
pub mod rail;
pub mod stage;
pub mod tokens;

#[cfg(test)]
mod tests {
    use bevy::{asset::AssetPlugin, prelude::*};

    use super::{backdrop, rail, stage};
    use crate::theme;

    #[test]
    fn scene_builders_spawn_under_root() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Font>();
        app.init_asset::<Image>();
        let assets = app.world().resource::<AssetServer>().clone();
        let root = app.world_mut().spawn(Node::default()).id();

        app.world_mut().entity_mut(root).with_children(|children| {
            children.spawn(backdrop::key_light(theme::EMERALD));
            children.spawn(backdrop::gold_rim());
            children.spawn(backdrop::grade());
            children.spawn(backdrop::vignette());
            children.spawn(backdrop::grain(&assets));
            children.spawn(stage::spot_glow(theme::EMERALD));
            children.spawn(stage::spot_ring(&assets, theme::EMERALD));
            children.spawn(stage::spot_ring_thin(&assets, theme::EMERALD));
            children.spawn(stage::spot_beam(&assets, theme::EMERALD));
            children.spawn(stage::ground_shadow());
            children.spawn(stage::horizon_line());
            children.spawn(rail::rail_container());
            children.spawn(rail::rail_header(&assets, "Codex", "Asgard"));
            children.spawn(rail::gold_rule());
            children.spawn(rail::section_label(&assets, "Attributes"));
        });

        assert_eq!(app.world().get::<Children>(root).unwrap().len(), 15);
    }
}
