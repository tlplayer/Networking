use crate::model::Packet;

pub fn packet_summary(packet: &Packet, op: &str, len: usize) -> String {
    match packet {
        Packet::Ack(packet) => format!("{op}({len:3}): len = 0008, ack = {:08x}", packet.ackno),
        Packet::Data(packet) => format!(
            "{op}({len:3}): len = {:04x}, ack = {:08x}, seq = {:08x}",
            len, packet.ackno, packet.seqno
        ),
    }
}

pub fn print_packet(packet: &Packet, op: &str, len: usize) {
    eprintln!("{}", packet_summary(packet, op, len));
}
