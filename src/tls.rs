/// Configuration for TLS connections backed by the [rustls] crate.
pub struct TlsConfig {
    pub(crate) config: rustls::ClientConfig,
    pub(crate) require_tls: bool,
}

impl TlsConfig {
    /// Create a new [TlsConfig] from a [rustls::ClientConfig] and a [bool] specifying whether
    /// connections not using TLS on the server side should fail or proceed without TLS.
    pub fn new(config: rustls::ClientConfig, require_tls: bool) -> Self {
        Self { config, require_tls }
    }
}
