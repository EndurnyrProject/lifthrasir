use bevy::prelude::*;

/// Marker component for the BGM audio channel
/// This is used to identify the BGM channel in bevy_kira_audio
#[derive(Debug, Default, Clone, Copy, Reflect)]
#[reflect(Debug, Default)]
pub struct BgmChannel;
