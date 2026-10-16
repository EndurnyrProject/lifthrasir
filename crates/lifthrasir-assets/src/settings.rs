use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum Upscaling {
    #[default]
    Off,
    X2,
    X3,
    X4,
}

impl Upscaling {
    /// The variants in stepper order.
    pub const ALL: [Upscaling; 4] = [Upscaling::Off, Upscaling::X2, Upscaling::X3, Upscaling::X4];

    /// Display label for the stepper value.
    pub fn label(self) -> &'static str {
        match self {
            Upscaling::Off => "Off",
            Upscaling::X2 => "2x",
            Upscaling::X3 => "3x",
            Upscaling::X4 => "4x",
        }
    }

    /// Next variant, clamped at the last.
    pub fn next(self) -> Upscaling {
        match self {
            Self::Off => Self::X2,
            Self::X2 => Self::X3,
            Self::X3 | Self::X4 => Self::X4,
        }
    }

    /// Previous variant, clamped at the first.
    pub fn prev(self) -> Upscaling {
        match self {
            Self::Off | Self::X2 => Self::Off,
            Self::X3 => Self::X2,
            Self::X4 => Self::X3,
        }
    }

    /// xBRZ scale factor; `Off` performs no upscaling.
    pub fn factor(self) -> Option<u8> {
        match self {
            Upscaling::Off => None,
            Upscaling::X2 => Some(2),
            Upscaling::X3 => Some(3),
            Upscaling::X4 => Some(4),
        }
    }
}

/// Processing configuration independent of saved client preferences.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpriteProcessingSettings {
    pub upscaling: Upscaling,
}
