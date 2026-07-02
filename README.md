# Networking

Networking labs and experiments.

## Reliable UDP Lab

The `udp/` folder contains the original Stanford CS144 reliable transport lab
framework in C plus a Rust rewrite scaffold.

The Rust crate uses a layered architecture with ports and adapters:

- `src/interface.rs`: protocol-facing traits and runtime actions.
- `src/model.rs`: packet, config, and error data models.
- `src/protocol/`: reliability state machine and stop-and-wait behavior.
- `src/runtime/`: concrete socket/runtime adapters.
- `src/utils/`: shared checksum, packet printing, and networking helpers.

See `udp/README.md` for the Lab 1 protocol plan.
