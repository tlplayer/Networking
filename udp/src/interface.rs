use crate::model::Packet;
use std::net::SocketAddr;

/// Actions requested by the protocol core.
///
/// Runtime code is responsible for performing these actions through real
/// sockets and streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    SendPacket(Packet),
    WriteOutput(Vec<u8>),
    OutputEof,
    Close,
}

/// Application-side byte stream.
///
/// Lab 1 can back this with stdin/stdout. Later TCP relay code can back this
/// with `std::net::TcpStream` or an async equivalent.
pub trait AppStream {
    /// Returns `Ok(None)` when no bytes are currently available.
    /// Returns `Ok(Some(vec![]))` to signal EOF.
    fn read_chunk(&mut self, max_len: usize) -> std::io::Result<Option<Vec<u8>>>;
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn write_eof(&mut self) -> std::io::Result<()>;
}

/// UDP-like packet transport.
///
/// This keeps protocol code independent from `UdpSocket`.
pub trait DatagramTransport {
    fn send_to(&self, packet: &Packet, peer: SocketAddr) -> std::io::Result<usize>;
    fn recv_from(&self) -> std::io::Result<Option<(Packet, SocketAddr)>>;
}
