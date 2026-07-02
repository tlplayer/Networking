use std::net::{SocketAddr, UdpSocket};

use crate::interface::DatagramTransport;
use crate::model::{Packet, MAX_PACKET_LEN};

pub struct StdUdpTransport {
    socket: UdpSocket,
}

impl StdUdpTransport {
    pub fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }

    pub fn from_socket(socket: UdpSocket) -> std::io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}

impl DatagramTransport for StdUdpTransport {
    fn send_to(&self, packet: &Packet, peer: SocketAddr) -> std::io::Result<usize> {
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = packet
            .encode(&mut buf)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        self.socket.send_to(&buf[..len], peer)
    }

    fn recv_from(&self) -> std::io::Result<Option<(Packet, SocketAddr)>> {
        let mut buf = [0u8; MAX_PACKET_LEN];
        match self.socket.recv_from(&mut buf) {
            Ok((len, peer)) => {
                let packet = Packet::decode(&buf[..len])
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
                Ok(Some((packet, peer)))
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(err),
        }
    }
}
