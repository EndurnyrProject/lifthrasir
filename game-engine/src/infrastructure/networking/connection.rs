#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Failed(String),
}

