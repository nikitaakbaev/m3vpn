use anyhow::{bail, Result};
use std::net::TcpListener;

/// Find the first port on 127.0.0.1 that can be bound, starting at `start`.
pub fn find_free_port(start: u16) -> Result<u16> {
    let end = start.saturating_add(64);
    for port in start..=end {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    bail!("no free TCP port in {start}..={end}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn skips_an_occupied_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
        let occupied = listener.local_addr().expect("local addr").port();
        let found = find_free_port(occupied).expect("find a free port");
        assert_ne!(found, occupied);
    }

    #[test]
    fn finds_a_port_in_a_normal_range() {
        assert!(find_free_port(20000).is_ok());
    }
}
