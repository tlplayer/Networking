use std::time::Instant;

use crate::interface::Action;
use crate::model::{AckPacket, Config, DataPacket, Packet, MAX_PAYLOAD_LEN};

#[derive(Debug)]
pub struct ReliableConnection {
    config: Config,
    send: SenderState,
    recv: ReceiverState,
    input_eof: bool,
    output_eof: bool,
}

#[derive(Debug)]
struct SenderState {
    next_seqno: u32,
    inflight: Option<InflightPacket>,
    eof_sent: bool,
}

#[derive(Debug, Clone)]
struct InflightPacket {
    packet: DataPacket,
    sent_at: Instant,
    retransmits: usize,
}

#[derive(Debug)]
struct ReceiverState {
    expected_seqno: u32,
}

impl ReliableConnection {
    pub fn new(config: Config) -> Self {
        assert!(config.window >= 1, "window must be at least 1");
        Self {
            config,
            send: SenderState::new(),
            recv: ReceiverState::new(),
            input_eof: false,
            output_eof: false,
        }
    }

    pub fn can_accept_input(&self) -> bool {
        self.send.can_send()
    }

    pub fn is_finished(&self) -> bool {
        self.input_eof && self.output_eof && self.send.inflight.is_none()
    }

    pub fn on_input(&mut self, bytes: &[u8], now: Instant) -> Vec<Action> {
        if !self.send.can_send() || bytes.is_empty() {
            return Vec::new();
        }

        let payload_len = bytes.len().min(MAX_PAYLOAD_LEN);
        let data = self.build_data_packet(bytes[..payload_len].to_vec());
        self.send_inflight(data, now)
    }

    pub fn on_input_eof(&mut self, now: Instant) -> Vec<Action> {
        self.input_eof = true;
        if self.send.eof_sent {
            return Vec::new();
        }

        if !self.send.can_send() {
            return Vec::new();
        }

        self.send_eof(now)
    }

    pub fn on_packet(&mut self, packet: Packet, now: Instant) -> Vec<Action> {
        self.apply_ack(packet.ackno());

        let mut actions = match packet {
            Packet::Ack(_) => Vec::new(),
            Packet::Data(data) => self.on_data(data),
        };

        actions.extend(self.send_pending_eof(now));
        actions
    }

    pub fn on_timer(&mut self, now: Instant) -> Vec<Action> {
        let Some(inflight) = self.send.inflight.as_mut() else {
            return Vec::new();
        };

        if now.saturating_duration_since(inflight.sent_at) < self.config.timeout {
            return Vec::new();
        }

        inflight.sent_at = now;
        inflight.retransmits += 1;
        vec![Action::SendPacket(Packet::Data(inflight.packet.clone()))]
    }

    fn on_data(&mut self, data: DataPacket) -> Vec<Action> {
        let mut actions = Vec::new();

        if data.seqno == self.recv.expected_seqno {
            if data.is_eof() {
                self.output_eof = true;
                actions.push(Action::OutputEof);
            } else {
                actions.push(Action::WriteOutput(data.payload));
            }
            self.recv.expected_seqno += 1;
        }

        actions.push(Action::SendPacket(Packet::Ack(AckPacket {
            ackno: self.recv.expected_seqno,
        })));

        if self.is_finished() {
            actions.push(Action::Close);
        }

        actions
    }

    fn apply_ack(&mut self, ackno: u32) {
        let Some(inflight) = self.send.inflight.as_ref() else {
            return;
        };

        if ackno > inflight.packet.seqno {
            self.send.inflight = None;
        }
    }

    fn build_data_packet(&mut self, payload: Vec<u8>) -> DataPacket {
        let seqno = self.send.next_seqno;
        self.send.next_seqno += 1;
        DataPacket {
            ackno: self.recv.expected_seqno,
            seqno,
            payload,
        }
    }

    fn send_inflight(&mut self, packet: DataPacket, now: Instant) -> Vec<Action> {
        self.send.inflight = Some(InflightPacket {
            packet: packet.clone(),
            sent_at: now,
            retransmits: 0,
        });
        vec![Action::SendPacket(Packet::Data(packet))]
    }

    fn send_pending_eof(&mut self, now: Instant) -> Vec<Action> {
        if self.input_eof && !self.send.eof_sent && self.send.can_send() {
            return self.send_eof(now);
        }

        Vec::new()
    }

    fn send_eof(&mut self, now: Instant) -> Vec<Action> {
        self.send.eof_sent = true;
        let data = self.build_data_packet(Vec::new());
        self.send_inflight(data, now)
    }
}

impl SenderState {
    fn new() -> Self {
        Self {
            next_seqno: 1,
            inflight: None,
            eof_sent: false,
        }
    }

    fn can_send(&self) -> bool {
        self.inflight.is_none()
    }
}

impl ReceiverState {
    fn new() -> Self {
        Self { expected_seqno: 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn conn() -> ReliableConnection {
        ReliableConnection::new(Config::stop_and_wait(Duration::from_millis(100)))
    }

    #[test]
    fn sends_first_input_chunk() {
        let now = Instant::now();
        let mut conn = conn();
        let actions = conn.on_input(b"abc", now);

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            &actions[0],
            Action::SendPacket(Packet::Data(data))
                if data.seqno == 1 && data.ackno == 1 && data.payload.as_slice() == b"abc"
        ));
        assert!(!conn.can_accept_input());
    }

    #[test]
    fn ack_clears_inflight_packet() {
        let now = Instant::now();
        let mut conn = conn();
        conn.on_input(b"abc", now);
        conn.on_packet(Packet::Ack(AckPacket { ackno: 2 }), now);
        assert!(conn.can_accept_input());
    }

    #[test]
    fn retransmits_after_timeout() {
        let now = Instant::now();
        let mut conn = conn();
        conn.on_input(b"abc", now);

        assert!(conn.on_timer(now + Duration::from_millis(99)).is_empty());
        let actions = conn.on_timer(now + Duration::from_millis(100));
        assert!(matches!(&actions[0], Action::SendPacket(Packet::Data(_))));
    }

    #[test]
    fn delivers_in_order_data_and_acks_next_seqno() {
        let now = Instant::now();
        let mut conn = conn();
        let actions = conn.on_packet(
            Packet::Data(DataPacket {
                ackno: 1,
                seqno: 1,
                payload: b"abc".to_vec(),
            }),
            now,
        );

        assert!(matches!(&actions[0], Action::WriteOutput(bytes) if bytes.as_slice() == b"abc"));
        assert!(matches!(
            &actions[1],
            Action::SendPacket(Packet::Ack(ack)) if ack.ackno == 2
        ));
    }
}
