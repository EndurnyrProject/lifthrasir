pub mod client_packets;
pub mod handlers;
pub mod protocol;
pub mod server_packets;
pub mod types;

// Re-export commonly used types
pub use client_packets::{CaLoginPacket, CA_LOGIN};
pub use handlers::{AcceptLoginHandler, LoginAccepted, LoginRefused, RefuseLoginHandler};
pub use protocol::{LoginClientPacket, LoginContext, LoginProtocol, LoginServerPacket};
pub use server_packets::{AcAcceptLoginPacket, AcRefuseLoginPacket, AC_ACCEPT_LOGIN, AC_REFUSE_LOGIN};
pub use types::{ServerInfo, ServerType};
