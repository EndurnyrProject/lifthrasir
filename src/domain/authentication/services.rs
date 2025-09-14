use super::models::LoginCredentials;
use crate::infrastructure::networking::{errors::NetworkError, session::UserSession};
use async_trait::async_trait;

#[async_trait]
pub trait AuthenticationService: Send + Sync {
    async fn authenticate(
        &self,
        credentials: &LoginCredentials,
    ) -> Result<UserSession, NetworkError>;

    async fn logout(&self, session: &UserSession) -> Result<(), NetworkError>;

    fn is_session_valid(&self, session: &UserSession) -> bool;
}
