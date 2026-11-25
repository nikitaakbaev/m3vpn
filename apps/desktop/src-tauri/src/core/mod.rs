// Packets from m3vpn front/backend processes carry this mark so the TUN
// helper can route them through the main table instead of back into TUN.
pub const ROUTING_MARK: u32 = 0x2022;

pub mod front;
pub mod singbox;
pub mod supervisor;
pub mod xray;

pub use supervisor::{
    connect_singbox, connect_xray, disconnect, front_pid, restart_front, rtt, shutdown,
    start_front, status, ConnectionStatus, CoreState,
};
