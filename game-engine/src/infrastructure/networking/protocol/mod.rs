pub mod dispatcher;
pub mod traits;

// Protocol implementations
pub mod character;
pub mod login;
pub mod zone;

// Re-export commonly used types
pub use dispatcher::PacketDispatcher;
pub use traits::{
    ClientPacket, EventBuffer, EventWriter, PacketHandler, PacketSize, Protocol, ServerPacket,
};
