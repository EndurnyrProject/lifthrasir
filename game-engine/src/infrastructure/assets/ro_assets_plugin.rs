use super::sources::CompositeAssetSource;
use bevy::prelude::*;
use std::sync::{Arc, RwLock};

/// Shared CompositeAssetSource for access across the engine
#[derive(Resource, Clone)]
pub struct SharedCompositeAssetSource(pub Arc<RwLock<CompositeAssetSource>>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::assets::sources::AssetSource;

    #[test]
    fn resource_is_queryable_from_a_system() {
        let mut app = App::new();
        app.insert_resource(SharedCompositeAssetSource(Arc::new(RwLock::new(
            CompositeAssetSource::new(),
        ))));

        fn check_source_reachable(source: Res<SharedCompositeAssetSource>) {
            let composite = source.0.read().unwrap();
            assert!(!composite.exists("data/anything.spr"));
        }

        app.add_systems(Update, check_source_reachable);
        app.update();
    }
}
