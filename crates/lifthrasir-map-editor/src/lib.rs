//! GLB-native map-editor domain types.
//!
//! This crate deliberately opens converted Lifthrasir map GLBs directly. It
//! does not read RSW, GND, GAT, or GRF files: those are import-only formats.
//! The first GUI application will use [`MapDocument`] as its document model.

mod document;

pub use document::{MapDocument, MapSceneSummary};
pub use lifthrasir_data::lif::{LifCollisionCell, LifCollisionFlags};
