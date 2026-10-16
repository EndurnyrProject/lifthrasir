use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Gender enum - shared across the application
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gender {
    Female = 0,
    Male = 1,
}

impl Serialize for Gender {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for Gender {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Ok(Gender::from(value))
    }
}

impl From<u8> for Gender {
    fn from(value: u8) -> Self {
        match value {
            0 => Gender::Female,
            _ => Gender::Male,
        }
    }
}

impl Gender {
    /// Korean sprite-filename segment: `남` (male) / `여` (female).
    pub const fn as_korean(self) -> &'static str {
        match self {
            Gender::Male => "남",
            Gender::Female => "여",
        }
    }
}
