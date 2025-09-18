use bevy::prelude::*;

// LoginUiState resource for managing UI state
#[derive(Resource)]
pub struct LoginUiState {
    pub is_connecting: bool,
    pub error_message: Option<String>,
    pub last_username: String,
    pub login_cooldown: Timer,
    pub initialized: bool,
}

impl Default for LoginUiState {
    fn default() -> Self {
        let mut cooldown = Timer::from_seconds(0.0, TimerMode::Once);
        cooldown.tick(std::time::Duration::from_secs(1)); // Force timer to finish immediately

        Self {
            is_connecting: false,
            error_message: None,
            last_username: String::new(),
            login_cooldown: cooldown,
            initialized: false,
        }
    }
}
