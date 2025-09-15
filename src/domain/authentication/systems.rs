use bevy::prelude::*;
use bevy_tokio_tasks::TokioTasksRuntime;
use secrecy::ExposeSecret;

use super::{components::*, events::*, models::*};
use crate::{
    core::state::GameState,
    infrastructure::networking::{ConnectionState, login_client},
    presentation::ui::events::LoginAttemptEvent,
};

pub fn handle_login_attempts(
    mut commands: Commands,
    mut login_attempts: EventReader<LoginAttemptEvent>,
    mut login_started_events: EventWriter<LoginAttemptStartedEvent>,
    auth_context: Res<AuthenticationContext>,
    existing_tasks: Query<Entity, With<LoginTask>>,
    runtime: ResMut<TokioTasksRuntime>,
) {
    // Prevent multiple simultaneous login attempts
    if !existing_tasks.is_empty() {
        return;
    }

    if let Some(attempt) = login_attempts.read().last() {
        info!("Starting login attempt for user: {}", attempt.username);

        let server_address = auth_context.server_config.login_server_address.clone();
        let client_version = auth_context.server_config.client_version;
        let username = attempt.username.clone();
        let password = attempt.password.expose_secret().to_string();
        let username_for_task = username.clone();
        let password_for_task = password.clone();

        let task = runtime.spawn_background_task(move |_ctx| async move {
            login_client::attempt_login(
                &server_address,
                &username_for_task,
                &password_for_task,
                client_version,
            )
            .await
        });

        // Spawn entity to hold the login task
        commands.spawn((
            LoginTask {
                username: username.clone(),
                task,
            },
            AuthenticationAttempt {
                username: username.clone(),
                started_at: std::time::Instant::now(),
            },
            ConnectionStateComponent {
                state: ConnectionState::Connecting,
            },
        ));

        // Emit event for UI feedback
        login_started_events.write(LoginAttemptStartedEvent { username });
    }
}

pub fn poll_login_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut LoginTask, &mut ConnectionStateComponent)>,
    mut success_events: EventWriter<LoginSuccessEvent>,
    mut failure_events: EventWriter<LoginFailureEvent>,
) {
    for (entity, mut login_task, mut connection_state) in &mut tasks {
        connection_state.state = ConnectionState::Authenticating;

        // Try to get the result using try_join (non-blocking check)
        if login_task.task.is_finished() {
            // Use block_on to get the result since we know it's finished
            let result = futures_lite::future::block_on(&mut login_task.task).unwrap();
            match result {
                Ok(login_response) => {
                    info!("Login successful for user: {}", login_task.username);

                    let session = crate::infrastructure::networking::session::UserSession::new(
                        login_task.username.clone(),
                        login_response,
                    );

                    connection_state.state = ConnectionState::Connected;

                    success_events.write(LoginSuccessEvent { session });
                }
                Err(error) => {
                    error!("Login failed for user {}: {}", login_task.username, error);

                    connection_state.state = ConnectionState::Failed(error.to_string());

                    failure_events.write(LoginFailureEvent {
                        error,
                        username: login_task.username.clone(),
                    });
                }
            }

            // Remove the completed task
            commands.entity(entity).despawn();
        }
    }
}

pub fn handle_login_success(
    mut success_events: EventReader<LoginSuccessEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    for event in success_events.read() {
        info!("Authentication successful - transitioning to ServerSelection state");

        // Store session data as a resource
        commands.insert_resource(event.session.clone());

        // Transition to the server selection state
        next_state.set(GameState::ServerSelection);
    }
}

pub fn handle_login_failure(
    mut failure_events: EventReader<LoginFailureEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in failure_events.read() {
        warn!(
            "Authentication failed for {}: {}",
            event.username, event.error
        );

        // Stay in login state to allow retry
        // The UI will handle displaying the error message
        next_state.set(GameState::Login);
    }
}

pub fn cleanup_failed_connections(
    mut commands: Commands,
    failed_connections: Query<(Entity, &ConnectionStateComponent, &AuthenticationAttempt)>,
    _time: Res<Time>,
) {
    // Clean up connection entities after a short timeout (2 seconds)
    for (entity, connection, attempt) in &failed_connections {
        if matches!(connection.state, ConnectionState::Failed(_)) {
            let elapsed = std::time::Instant::now() - attempt.started_at;
            if elapsed.as_secs() > 2 {
                commands.entity(entity).despawn();
            }
        }
    }
}
