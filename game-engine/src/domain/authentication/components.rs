use bevy::prelude::*;

/// Component for tracking authentication attempts
/// Used for cleanup and timeout logic
#[derive(Component, Debug)]
pub struct AuthenticationAttempt {
    pub username: String,
    pub started_at: std::time::Instant,
}
