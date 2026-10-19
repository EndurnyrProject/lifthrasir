use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Why the server rejected a Peco Peco mount/unmount attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PecoMountRejection {
    /// The character has not learned `KN_RIDING`.
    SkillNotLearned,
    /// A Peco is already mounted.
    AlreadyMounted,
    /// An unmount was requested while not mounted.
    NotMounted,
    /// The character is dead.
    Dead,
}

/// The server's outcome of a [`MountPeco`](crate::commands::MountPeco) request.
/// A successful mount/unmount is `Ok`; the body sprite swap already reflects it,
/// so the UI only surfaces the rejection reason.
#[derive(Message, Debug, Clone)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub struct PecoMountResult {
    pub outcome: Result<(), PecoMountRejection>,
}
