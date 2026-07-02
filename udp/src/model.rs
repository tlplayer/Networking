use std::fmt;
use std::time::Duration;

use crate::utils::checksum::internet_checksum;

pub const ACK_LEN: usize = 8;
pub const DATA_HEADER_LEN: usize = 12;
pub const MAX_PAYLOAD_LEN: usize = 500;
pub const MAX_PACKET_LEN: usize = DATA_HEADER_LEN + MAX_PAYLOAD_LEN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub window: usize,
    pub timeout: Duration,
}

impl Config {
    pub fn stop_and_wait(timeout: Duration) -> Self {
        Self { window: 1, timeout }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Ack(AckPacket),
    Data(DataPacket),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckPacket {
    pub ackno: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPacket {
    pub ackno: u32,
    pub seqno: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    TooShort { actual: usize },
    BadLength { declared: usize, actual: usize },
    BadChecksum,
    BadPacketType { len: usize },
    PayloadTooLarge { actual: usize, max: usize },
    BufferTooSmall { actual: usize, required: usize },
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { actual } => write!(f, "packet is too short: {actual} bytes"),
            Self::BadLength { declared, actual } => {
                write!(f, "packet length field {declared} does not match datagram {actual}")
            }
            Self::BadChecksum => write!(f, "packet checksum is invalid"),
            Self::BadPacketType { len } => write!(f, "invalid packet length: {len}"),
            Self::PayloadTooLarge { actual, max } => {
                write!(f, "payload is too large: {actual} bytes, max {max}")
            }
            Self::BufferTooSmall { actual, required } => {
                write!(f, "buffer is too small: {actual} bytes, required {required}")
            }
        }
    }
}

impl std::error::Error for PacketError {}

impl Packet {
    pub fn ackno(&self) -> u32 {
        match self {
            Self::Ack(packet) => packet.ackno,
            Self::Data(packet) => packet.ackno,
        }
    }

    pub fn decode(datagram: &[u8]) -> Result<Self, PacketError> {
        if datagram.len() < ACK_LEN {
            return Err(PacketError::TooShort {
                actual: datagram.len(),
            });
        }

        let declared_len = u16::from_be_bytes([datagram[2], datagram[3]]) as usize;
        if declared_len != datagram.len() {
            return Err(PacketError::BadLength {
                declared: declared_len,
                actual: datagram.len(),
            });
        }

        if internet_checksum(datagram) != 0xffff {
            return Err(PacketError::BadChecksum);
        }

        let ackno = u32::from_be_bytes([datagram[4], datagram[5], datagram[6], datagram[7]]);

        if declared_len == ACK_LEN {
            return Ok(Self::Ack(AckPacket { ackno }));
        }

        if !(DATA_HEADER_LEN..=MAX_PACKET_LEN).contains(&declared_len) {
            return Err(PacketError::BadPacketType { len: declared_len });
        }

        let seqno = u32::from_be_bytes([datagram[8], datagram[9], datagram[10], datagram[11]]);
        let payload = datagram[DATA_HEADER_LEN..declared_len].to_vec();
        Ok(Self::Data(DataPacket {
            ackno,
            seqno,
            payload,
        }))
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<usize, PacketError> {
        let len = self.encoded_len()?;
        if out.len() < len {
            return Err(PacketError::BufferTooSmall {
                actual: out.len(),
                required: len,
            });
        }

        out[..len].fill(0);
        out[2..4].copy_from_slice(&(len as u16).to_be_bytes());
        out[4..8].copy_from_slice(&self.ackno().to_be_bytes());

        if let Self::Data(packet) = self {
            out[8..12].copy_from_slice(&packet.seqno.to_be_bytes());
            out[DATA_HEADER_LEN..len].copy_from_slice(&packet.payload);
        }

        let checksum = internet_checksum(&out[..len]);
        out[0..2].copy_from_slice(&checksum.to_be_bytes());
        Ok(len)
    }

    pub fn encoded_len(&self) -> Result<usize, PacketError> {
        match self {
            Self::Ack(_) => Ok(ACK_LEN),
            Self::Data(packet) => {
                if packet.payload.len() > MAX_PAYLOAD_LEN {
                    return Err(PacketError::PayloadTooLarge {
                        actual: packet.payload.len(),
                        max: MAX_PAYLOAD_LEN,
                    });
                }
                Ok(DATA_HEADER_LEN + packet.payload.len())
            }
        }
    }
}

impl DataPacket {
    pub fn is_eof(&self) -> bool {
        self.payload.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_ack() {
        let packet = Packet::Ack(AckPacket { ackno: 9 });
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = packet.encode(&mut buf).unwrap();
        assert_eq!(len, ACK_LEN);
        assert_eq!(Packet::decode(&buf[..len]).unwrap(), packet);
    }

    #[test]
    fn round_trips_data() {
        let packet = Packet::Data(DataPacket {
            ackno: 2,
            seqno: 1,
            payload: b"hello".to_vec(),
        });
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = packet.encode(&mut buf).unwrap();
        assert_eq!(len, DATA_HEADER_LEN + 5);
        assert_eq!(Packet::decode(&buf[..len]).unwrap(), packet);
    }

    #[test]
    fn rejects_bad_length() {
        let packet = Packet::Ack(AckPacket { ackno: 1 });
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = packet.encode(&mut buf).unwrap();
        assert!(matches!(
            Packet::decode(&buf[..len - 1]),
            Err(PacketError::BadLength { .. })
        ));
    }

    #[test]
    fn rejects_bad_checksum() {
        let packet = Packet::Ack(AckPacket { ackno: 1 });
        let mut buf = [0u8; MAX_PACKET_LEN];
        let len = packet.encode(&mut buf).unwrap();
        buf[7] ^= 1;
        assert_eq!(Packet::decode(&buf[..len]), Err(PacketError::BadChecksum));
    }
}
