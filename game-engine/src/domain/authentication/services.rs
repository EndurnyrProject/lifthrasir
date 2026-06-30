use super::models::LoginCredentials;
use async_trait::async_trait;
use net_contract::{dto::NetworkError, state::UserSession};

#[async_trait]
pub trait AuthenticationService: Send + Sync {
    async fn authenticate(
        &self,
        credentials: &LoginCredentials,
    ) -> Result<UserSession, NetworkError>;

    async fn logout(&self, session: &UserSession) -> Result<(), NetworkError>;

    fn is_session_valid(&self, session: &UserSession) -> bool;
}
