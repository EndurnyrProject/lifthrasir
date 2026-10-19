use bevy::prelude::*;

/// Animation type for converted GLB map props.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationType {
    #[default]
    None,
    Loop,
    Once,
}
