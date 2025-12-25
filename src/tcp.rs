use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use hyper_util::client::legacy::connect::HttpConnector;

use crate::dns::DnsResolver;

/// Keepalive options for a TCP connection.
#[derive(Debug, Clone, Copy)]
pub struct TcpKeepaliveConfig {
    pub duration: Duration,
    pub interval: Duration,
    pub retries: u32,
}

/// One or multiple IPv4 and/or IPv6 local addresses to use for a TCP connection.
#[derive(Debug, Clone, Copy)]
pub enum TcpLocalAddress {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
    Both(Ipv4Addr, Ipv6Addr),
}

/// Configuration for TCP connections. This struct is cheaply [Clone]-able and implements [Default],
/// as all its settings are optional.
#[derive(Debug, Clone, Default)]
pub struct TcpConfig {
    pub keepalive: Option<TcpKeepaliveConfig>,
    pub nodelay: Option<bool>,
    pub send_buffer_size: Option<usize>,
    pub recv_buffer_size: Option<usize>,
    pub local_address: Option<TcpLocalAddress>,
    pub internal_timeout: Option<Duration>,
    pub happy_eyeballs_timeout: Option<Duration>,
    pub reuse_address: Option<bool>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub interface: Option<String>,
    #[cfg(target_os = "linux")]
    pub user_timeout: Option<Duration>,
}

impl TcpConfig {
    pub(crate) fn build_connector(self, dns_resolver: DnsResolver) -> HttpConnector<DnsResolver> {
        let mut connector = HttpConnector::new_with_resolver(dns_resolver);

        if let Some(keepalive) = self.keepalive {
            connector.set_keepalive(Some(keepalive.duration));
            connector.set_keepalive_interval(Some(keepalive.interval));
            connector.set_keepalive_retries(Some(keepalive.retries));
        }

        if let Some(nodelay) = self.nodelay {
            connector.set_nodelay(nodelay);
        }

        if let Some(send_buffer_size) = self.send_buffer_size {
            connector.set_send_buffer_size(Some(send_buffer_size));
        }

        if let Some(recv_buffer_size) = self.recv_buffer_size {
            connector.set_recv_buffer_size(Some(recv_buffer_size));
        }

        match self.local_address {
            Some(TcpLocalAddress::V4(v4)) => connector.set_local_address(Some(IpAddr::V4(v4))),
            Some(TcpLocalAddress::V6(v6)) => connector.set_local_address(Some(IpAddr::V6(v6))),
            Some(TcpLocalAddress::Both(v4, v6)) => connector.set_local_addresses(v4, v6),
            None => (),
        }

        if let Some(internal_timeout) = self.internal_timeout {
            connector.set_connect_timeout(Some(internal_timeout));
        }

        if let Some(happy_eyeballs_timeout) = self.happy_eyeballs_timeout {
            connector.set_happy_eyeballs_timeout(Some(happy_eyeballs_timeout));
        }

        if let Some(reuse_address) = self.reuse_address {
            connector.set_reuse_address(reuse_address);
        }

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Some(interface) = self.interface {
            connector.set_interface(interface);
        }

        #[cfg(target_os = "linux")]
        if let Some(user_timeout) = self.user_timeout {
            connector.set_tcp_user_timeout(Some(user_timeout));
        }

        connector
    }
}
