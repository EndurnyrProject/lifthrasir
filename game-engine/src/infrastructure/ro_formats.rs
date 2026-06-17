//! Re-export of the standalone Bevy-free `ro-formats` crate, preserving the
//! `crate::infrastructure::ro_formats::*` paths (including submodule paths such
//! as `ro_formats::rsm::Node`) used across the engine.
pub use ro_formats::*;
