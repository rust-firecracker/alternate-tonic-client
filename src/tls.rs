pub struct TlsConfig {
    pub(crate) config: rustls::ClientConfig,
    pub(crate) require_tls: bool,
}

impl TlsConfig {
    pub fn new(config: rustls::ClientConfig, require_tls: bool) -> Self {
        Self { config, require_tls }
    }
}
