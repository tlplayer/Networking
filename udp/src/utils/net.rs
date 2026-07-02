use std::net::{SocketAddr, ToSocketAddrs};

pub fn parse_socket_addr(input: &str) -> std::io::Result<SocketAddr> {
    input
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no socket address"))
}

pub fn same_endpoint(a: &SocketAddr, b: &SocketAddr) -> bool {
    a == b
}
