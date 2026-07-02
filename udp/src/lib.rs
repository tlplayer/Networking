//! Reliable transport over UDP.
//!
//! The crate is organized as a small layered architecture:
//! interfaces define the runtime boundary, models define packets and
//! configuration, protocol owns reliability state, and runtime adapters own
//! sockets or application streams.

pub mod interface;
pub mod model;
pub mod protocol;
pub mod runtime;
pub mod utils;

pub use interface::{Action, AppStream, DatagramTransport};
pub use model::{AckPacket, Config, DataPacket, Packet, PacketError};
pub use protocol::ReliableConnection;
