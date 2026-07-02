# Reliable Transport Over UDP, in Rust

Original lab: https://www.scs.stanford.edu/10au-cs144/lab/reliable/reliable.html

This folder contains the original Stanford CS144 reliable transport lab framework in C. The goal of the Rust rewrite is to keep the same learning target while replacing the C framework boundary with Rust modules, explicit data models, and testable protocol state.

Lab 1 implements a reliable stop-and-wait transport protocol over UDP. Stop-and-wait is the simplest sliding-window protocol: the sender may have only one unacknowledged data packet in flight. Lab 2 can extend the same structure to a larger sliding window and server-side demultiplexing.

## Lab 1 Scope

Lab 1 should support:

- A single direct connection between two UDP endpoints.
- Bidirectional stream transfer using standard input/output or equivalent async streams.
- Fixed-size UDP data packets carrying up to 500 bytes.
- Cumulative acknowledgements.
- Retransmission on timeout.
- Detection and rejection of corrupt packets using the lab checksum.
- EOF delivery using a data packet with zero bytes of payload.

Lab 1 does not need:

- Multiple simultaneous clients on one UDP server port.
- A send window larger than 1.
- Out-of-order receive buffering beyond recognizing duplicates.
- TCP relay mode, except as an interface design consideration.

## Protocol Overview

UDP provides only datagrams. It may drop, corrupt, duplicate, or reorder packets. The reliable layer treats UDP as an unreliable packet carrier and builds a stream abstraction on top.

In this lab, each endpoint is both a sender and a receiver:

- The sender reads bytes from an input stream, wraps them in protocol packets, and sends them over UDP.
- The receiver validates packets, writes in-order payload bytes to an output stream, and sends acknowledgements.
- Each side keeps independent send and receive sequence state.

The first data packet sequence number is `1`. Acknowledgements are cumulative: `ackno = N` means "I have received all packets before N and am waiting for packet N."

## Packet Format

There are two packet types. The type is determined by length.

| Packet type | Length | Fields |
| --- | ---: | --- |
| Ack-only | 8 bytes | `cksum`, `len`, `ackno` |
| Data | 12 to 512 bytes | `cksum`, `len`, `ackno`, `seqno`, `data` |

All multi-byte integer fields except `cksum` are encoded in network byte order, which Rust exposes through `to_be_bytes` and `from_be_bytes`.

```text
Ack packet:
0               15 16              31
+----------------+------------------+
| cksum          | len = 8          |
+----------------+------------------+
| ackno                             |
+-----------------------------------+

Data packet:
0               15 16              31
+----------------+------------------+
| cksum          | len = 12+n       |
+----------------+------------------+
| ackno                             |
+-----------------------------------+
| seqno                             |
+-----------------------------------+
| data, 0..500 bytes                |
+-----------------------------------+
```

EOF is represented as a data packet with `len = 12` and an empty payload.

## Rust Data Models

Keep packet parsing separate from socket I/O. This makes checksum, byte-order, and malformed-packet behavior easy to unit test.

```rust
pub const ACK_LEN: usize = 8;
pub const DATA_HEADER_LEN: usize = 12;
pub const MAX_PAYLOAD_LEN: usize = 500;
pub const MAX_PACKET_LEN: usize = DATA_HEADER_LEN + MAX_PAYLOAD_LEN;

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
    TooShort,
    BadLength,
    BadChecksum,
    BadPacketType,
    PayloadTooLarge,
}
```

Suggested encode/decode API:

```rust
impl Packet {
    pub fn decode(datagram: &[u8]) -> Result<Self, PacketError> {
        // 1. Require at least ACK_LEN bytes.
        // 2. Read len from bytes 2..4.
        // 3. Require len == datagram.len().
        // 4. Verify checksum over exactly len bytes.
        // 5. If len == ACK_LEN, parse Ack.
        // 6. If DATA_HEADER_LEN <= len <= MAX_PACKET_LEN, parse Data.
        unimplemented!()
    }

    pub fn encode(&self, out: &mut [u8; MAX_PACKET_LEN]) -> Result<usize, PacketError> {
        // 1. Write cksum as zero.
        // 2. Write len, ackno, and seqno if present in big-endian order.
        // 3. Copy payload.
        // 4. Compute checksum and write it into bytes 0..2.
        // 5. Return the encoded packet length.
        unimplemented!()
    }
}
```

The checksum should match the original lab's IP-style checksum. It should be implemented once and used only by packet encoding/decoding.

```rust
pub fn checksum(bytes: &[u8]) -> u16 {
    // Internet checksum:
    // - Sum 16-bit words using one's-complement arithmetic.
    // - Pad an odd trailing byte with zero.
    // - Fold carries back into the low 16 bits.
    // - Return the one's complement.
    unimplemented!()
}
```

## Component Structure

A Rust project can keep the protocol core independent from the runtime:

```text
src/
  main.rs          command-line parsing and process startup
  packet.rs        Packet, AckPacket, DataPacket, checksum, encode/decode
  protocol.rs      ReliableConnection and stop-and-wait state machine
  runtime.rs       UDP socket, stdin/stdout, timer, and event loop
  tcp_relay.rs     Lab 2 client/server TCP relay adapters
```

### `packet`

Responsibilities:

- Convert datagrams to `Packet`.
- Convert `Packet` values to datagram bytes.
- Enforce packet length rules.
- Validate checksums.
- Hide byte-order details from the rest of the program.

### `protocol`

Responsibilities:

- Track send and receive sequence numbers.
- Decide when a packet may be sent.
- Process inbound data and ack packets.
- Produce outbound UDP packets.
- Produce application output bytes.
- Decide when retransmission is required.
- Track EOF state and connection teardown.

The protocol layer should not own a `UdpSocket`, read stdin, write stdout, or sleep.

### `runtime`

Responsibilities:

- Bind the local UDP port.
- Send encoded packets to the peer address.
- Read UDP datagrams and pass decoded packets into the protocol.
- Read application input when the protocol has send capacity.
- Write application output produced by the protocol.
- Call the protocol timer periodically.

This can be implemented with blocking sockets plus polling, or with an async runtime such as Tokio. For learning purposes, a small blocking event loop is often easier to debug first.

### `tcp_relay`

Lab 1 can ignore this module at runtime, but the interface should anticipate it. In Lab 2, TCP sockets become another source/sink for the same reliable byte stream:

- Client mode accepts local TCP connections and relays each over UDP.
- Server mode receives UDP connections and opens TCP connections to the configured destination.
- The reliable protocol still sees only input bytes, output bytes, UDP packets, and timers.

## Core Interface

A useful pattern is to make the protocol state machine return actions for the runtime to perform.

```rust
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum Action {
    SendPacket(Packet),
    WriteOutput(Vec<u8>),
    OutputEof,
    Close,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub window: usize,
    pub timeout: Duration,
}

pub struct ReliableConnection {
    config: Config,
    send: SenderState,
    recv: ReceiverState,
    input_eof: bool,
    output_eof: bool,
}
```

For Lab 1, `config.window` should be `1`.

```rust
impl ReliableConnection {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            send: SenderState::new(),
            recv: ReceiverState::new(),
            input_eof: false,
            output_eof: false,
        }
    }

    pub fn on_input(&mut self, bytes: &[u8], now: Instant) -> Vec<Action> {
        // Called when application bytes are available.
        // Lab 1: send only if no unacknowledged packet is in flight.
        unimplemented!()
    }

    pub fn on_input_eof(&mut self, now: Instant) -> Vec<Action> {
        // Send one zero-length data packet when send capacity is available.
        unimplemented!()
    }

    pub fn on_packet(&mut self, packet: Packet, now: Instant) -> Vec<Action> {
        // Process ack information first, then data if present.
        unimplemented!()
    }

    pub fn on_timer(&mut self, now: Instant) -> Vec<Action> {
        // Retransmit the outstanding data packet if it has timed out.
        unimplemented!()
    }
}
```

## Sender State

Stop-and-wait sender state is small:

```rust
#[derive(Debug)]
struct SenderState {
    next_seqno: u32,
    inflight: Option<InflightPacket>,
}

#[derive(Debug, Clone)]
struct InflightPacket {
    seqno: u32,
    packet: DataPacket,
    sent_at: Instant,
    retransmits: usize,
}

impl SenderState {
    fn new() -> Self {
        Self {
            next_seqno: 1,
            inflight: None,
        }
    }

    fn can_send(&self) -> bool {
        self.inflight.is_none()
    }
}
```

When the sender receives an acknowledgement:

- If `ackno > inflight.seqno`, the packet has been acknowledged.
- Clear `inflight`.
- Advance `next_seqno` if needed.
- Read and send more input if available.

For Lab 1, there is only one outstanding packet. In Lab 2, replace `Option<InflightPacket>` with an ordered queue keyed by sequence number.

## Receiver State

Stop-and-wait receiver state is also small:

```rust
#[derive(Debug)]
struct ReceiverState {
    expected_seqno: u32,
}

impl ReceiverState {
    fn new() -> Self {
        Self { expected_seqno: 1 }
    }
}
```

When a data packet arrives:

- If checksum or length validation failed, drop it before it reaches the protocol.
- If `seqno == expected_seqno`, deliver the payload and increment `expected_seqno`.
- If the payload is empty, deliver EOF instead of bytes.
- Send an ack with `ackno = expected_seqno`.
- If `seqno < expected_seqno`, it is probably a duplicate. Re-send the current ack.
- If `seqno > expected_seqno`, Lab 1 can drop it and re-send the current ack.

Because Lab 1 has window size 1, packet reordering mostly appears as duplicates or unexpected future packets. Lab 2 should buffer future packets up to the receive window.

## Stop-and-Wait Flow

Sending one data chunk:

```text
application input -> Data(seqno = 1, ackno = recv.expected_seqno)
sender stores packet as inflight
sender sends UDP datagram

receiver validates datagram
receiver writes payload
receiver increments expected_seqno to 2
receiver sends Ack(ackno = 2)

sender receives Ack(ackno = 2)
sender clears inflight
sender may send seqno = 2
```

If the data packet or ack is lost, the sender's timer retransmits the stored inflight packet after `timeout`.

## Runtime Sketch

This sketch uses synchronous standard library types. A production version may use `mio` or `tokio`, but the protocol interface should stay similar.

```rust
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

fn run_single_connection(
    socket: UdpSocket,
    peer: SocketAddr,
    mut conn: ReliableConnection,
) -> std::io::Result<()> {
    socket.set_nonblocking(true)?;
    let mut udp_buf = [0u8; MAX_PACKET_LEN];
    let mut enc_buf = [0u8; MAX_PACKET_LEN];

    loop {
        let now = Instant::now();

        match socket.recv_from(&mut udp_buf) {
            Ok((n, from)) if from == peer => {
                if let Ok(packet) = Packet::decode(&udp_buf[..n]) {
                    for action in conn.on_packet(packet, now) {
                        perform_action(&socket, peer, action, &mut enc_buf)?;
                    }
                }
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => return Err(err),
        }

        for action in conn.on_timer(now) {
            perform_action(&socket, peer, action, &mut enc_buf)?;
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn perform_action(
    socket: &UdpSocket,
    peer: SocketAddr,
    action: Action,
    enc_buf: &mut [u8; MAX_PACKET_LEN],
) -> std::io::Result<()> {
    match action {
        Action::SendPacket(packet) => {
            let n = packet.encode(enc_buf).expect("protocol built invalid packet");
            socket.send_to(&enc_buf[..n], peer)?;
        }
        Action::WriteOutput(bytes) => {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
            std::io::stdout().flush()?;
        }
        Action::OutputEof | Action::Close => {}
    }

    Ok(())
}
```

Real Lab 1 code also needs to feed stdin into `conn.on_input`. The important rule is the same as the C lab: do not consume unlimited input when the protocol has no send capacity. With stop-and-wait, read at most one 500-byte chunk when `send.can_send()` is true.

## UDP vs. TCP Responsibilities

UDP is the unreliable network substrate for this protocol:

- Carries encoded `Packet` datagrams.
- Preserves packet boundaries.
- Does not provide reliability, ordering, stream semantics, or trusted checksums for this lab.

TCP is not part of Lab 1's transport mechanism. It appears in Lab 2 as an application-side adapter:

- TCP input bytes can be carried over the reliable UDP protocol.
- TCP output receives bytes reconstructed from reliable UDP packets.
- TCP connection boundaries map to reliable protocol connection state.

The reliable protocol should therefore be written against a generic byte-stream interface rather than hard-coding stdin/stdout:

```rust
pub trait AppStream {
    fn read_chunk(&mut self, max_len: usize) -> std::io::Result<Option<Vec<u8>>>;
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn write_eof(&mut self) -> std::io::Result<()>;
}
```

For Lab 1, `AppStream` can wrap stdin/stdout. For Lab 2, it can wrap a `TcpStream`.

## Suggested Lab 1 Milestones

1. Implement `packet.rs` with encode/decode/checksum tests.
2. Implement `ReliableConnection::on_packet` for ack-only and in-order data.
3. Implement stop-and-wait sending from one input chunk.
4. Add timeout-based retransmission.
5. Add EOF send/receive behavior.
6. Build the single-connection UDP runtime.
7. Test against packet loss, duplicate packets, corruption, and delayed acks.

## Test Cases

Start with unit tests before end-to-end UDP tests:

- Encode then decode an ack packet.
- Encode then decode a data packet with 0, 1, 499, and 500 bytes.
- Reject packets whose datagram length does not match the `len` field.
- Reject data packets larger than 512 bytes.
- Reject packets with invalid checksums.
- Deliver only the expected sequence number.
- Re-ack duplicate data packets.
- Retransmit exactly after timeout, not on every timer tick.
- Send EOF as `Data { payload: [] }`.

## Mapping From Original C Framework

| C framework concept | Rust rewrite concept |
| --- | --- |
| `packet_t` | `Packet`, `AckPacket`, `DataPacket` |
| `rel_t` / `struct reliable_state` | `ReliableConnection` |
| `rel_create` | `ReliableConnection::new` |
| `rel_recvpkt` | `ReliableConnection::on_packet` |
| `rel_read` | `ReliableConnection::on_input` |
| `rel_output` | runtime performs `Action::WriteOutput` |
| `rel_timer` | `ReliableConnection::on_timer` |
| `conn_sendpkt` | runtime sends `Action::SendPacket` through `UdpSocket` |
| `conn_input` | `AppStream::read_chunk` |
| `conn_output` | `AppStream::write_all` |
| `rel_demux` | Lab 2 `HashMap<SocketAddr, ReliableConnection>` |

The main design rule is to keep reliability decisions in `protocol.rs` and keep sockets in `runtime.rs`. That split makes Lab 1 easier to finish and gives Lab 2 a clear place to add sliding windows and TCP-backed connection demultiplexing.
