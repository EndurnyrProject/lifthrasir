//! GLB-native map-editor domain types.
//!
//! This crate deliberately opens converted Lifthrasir map GLBs directly. It
//! does not read RSW, GND, GAT, or GRF files: those are import-only formats.
//! The GUI uses [`MapDocument`] as its document model. This library contains
//! no Bevy systems, ECS types, asset-source setup or client/network policy.

mod document;

pub use document::{MapDocument, MapSceneSummary};
pub use lifthrasir_data::lif::{LifCollisionCell, LifCollisionFlags};
