#[derive(Debug, Clone, PartialEq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Failed(String),
}
