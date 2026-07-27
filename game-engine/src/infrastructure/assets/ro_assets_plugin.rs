use super::sources::CompositeAssetSource;
use bevy::prelude::*;
use std::sync::{Arc, RwLock};

/// Shared CompositeAssetSource for access across the engine
#[derive(Resource, Clone)]
pub struct SharedCompositeAssetSource(pub Arc<RwLock<CompositeAssetSource>>);
