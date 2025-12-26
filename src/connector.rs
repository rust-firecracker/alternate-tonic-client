use std::{
    task::{Context, Poll},
    time::Duration,
};

use http::Uri;
#[cfg(feature = "custom-transport")]
use tower::ServiceExt;
use tower::{BoxError, Service};

#[cfg(feature = "__transport")]
use crate::stream::GrpcStreamInner;
use crate::{BoxResultFuture, stream::GrpcStream};

/// A builder for a [GrpcConnector].
#[derive(Debug, Clone)]
pub struct GrpcConnectorBuilder {
    timeout: Option<Duration>,
    #[cfg(feature = "firecracker-handshake")]
    firecracker_handshake_port: Option<u32>,
}

impl GrpcConnectorBuilder {
    /// Create a new [GrpcConnectorBuilder].
    pub fn new() -> Self {
        Self {
            timeout: None,
            #[cfg(feature = "firecracker-handshake")]
            firecracker_handshake_port: None,
        }
    }

    /// Set a timeout [Duration] for all connection attempts made via the [GrpcConnector].
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Configure a Firecracker virtio-vsock handshake to the given guest port to be performed as part of the connection process.
    /// Trying to build a [GrpcConnector] with the DNS/TCP/TLS transport and a Firecracker handshake enabled will result in a panic
    /// in a debug build, and the handshake configuration will be silently ignored in a release build. Usually, this feature is used
    /// in combination with the Unix transport, as Firecracker uses Unix sockets on the host, but it can be combined with all other
    /// transports (except for DNS/TCP/TLS).
    #[cfg(feature = "firecracker-handshake")]
    pub fn perform_firecracker_handshake(mut self, port: u32) -> Self {
        self.firecracker_handshake_port = Some(port);
        self
    }

    /// Build a [GrpcConnector] that performs DNS resolution of a given [Uri] to an IP and connects to that
    /// IP over TCP without using TLS.
    #[cfg(feature = "dns-tcp-transport")]
    pub fn build_to_tcp_host(
        self,
        uri: Uri,
        dns_resolver: crate::dns::DnsResolver,
        tcp_config: crate::tcp::TcpConfig,
    ) -> GrpcConnector {
        self.build(GrpcConnectorInner::DnsTcp(
            uri,
            tcp_config.build_connector(dns_resolver),
        ))
    }

    /// Build a [GrpcConnector] that performs DNS resolution of a given [Uri] to an IP and connects to that
    /// IP over TCP with TLS.
    #[cfg(feature = "dns-tcp-tls-transport")]
    pub fn build_to_tcp_host_with_tls(
        self,
        uri: Uri,
        dns_resolver: crate::dns::DnsResolver,
        tcp_config: crate::tcp::TcpConfig,
        tls_config: crate::tls::TlsConfig,
    ) -> GrpcConnector {
        #[cfg(feature = "firecracker-handshake")]
        debug_assert!(
            self.firecracker_handshake_port.is_none(),
            "Firecracker handshake support in the alternate-tonic-client crate cannot be combined with the DNS/TCP/TLS transport."
        );

        let connector = hyper_rustls::HttpsConnectorBuilder::new().with_tls_config(tls_config.config);

        let connector = match tls_config.require_tls {
            true => connector.https_only(),
            false => connector.https_or_http(),
        };

        let connector = connector
            .enable_http2()
            .wrap_connector(tcp_config.build_connector(dns_resolver));

        self.build(GrpcConnectorInner::DnsTcpTls(uri, connector))
    }

    /// Build a [GrpcConnector] that connects to the Unix socket located at the given path.
    #[cfg(feature = "unix-transport")]
    pub fn build_to_unix_socket<P: Into<std::path::PathBuf>>(self, socket_path: P) -> GrpcConnector {
        self.build(GrpcConnectorInner::Unix(std::sync::Arc::new(socket_path.into())))
    }

    /// Build a [GrpcConnector] that connects to a virtio-vsock socket identified by the given CID and port.
    #[cfg(feature = "vsock-transport")]
    pub fn build_to_vsock_socket(self, cid: u32, port: u32) -> GrpcConnector {
        self.build(GrpcConnectorInner::Vsock(cid, port))
    }

    /// Build a [GrpcConnector] that connects via a custom tower [Service]. This [Service] must accept `()` as
    /// a request, return a [GrpcStream] (initialized via either [GrpcStream::wrap_hyper_io] or [GrpcStream::wrap_tokio_io])
    /// as a response and emit an error that is convertible into a boxed type-erased [std::error::Error].
    #[cfg(feature = "custom-transport")]
    pub fn build_custom<S>(self, service: S) -> GrpcConnector
    where
        S: Service<(), Response = GrpcStream> + Send + Sync + Clone + 'static,
        S::Error: Into<BoxError>,
        S::Future: Send,
    {
        self.build(GrpcConnectorInner::Custom(tower::util::BoxCloneSyncService::new(
            service.map_err(|err| err.into()),
        )))
    }

    #[cfg(feature = "__transport")]
    fn build(self, inner: GrpcConnectorInner) -> GrpcConnector {
        GrpcConnector {
            inner,
            timeout: self.timeout,
            #[cfg(feature = "firecracker-handshake")]
            firecracker_handshake_port: self.firecracker_handshake_port,
        }
    }
}

/// A [Service] used to connect to a gRPC server, yielding a [GrpcStream].
/// An instance of a [GrpcConnector] is built via a [GrpcConnectorBuilder] and is cheaply [Clone]-able.
/// For technical reasons, this [Service] accepts [Uri] as its request type yet actually always ignores its value
/// regardless of which transport is being used.
#[derive(Debug, Clone)]
pub struct GrpcConnector {
    inner: GrpcConnectorInner,
    #[cfg_attr(not(feature = "__transport"), allow(unused))]
    timeout: Option<Duration>,
    #[cfg(feature = "firecracker-handshake")]
    firecracker_handshake_port: Option<u32>,
}

#[derive(Debug, Clone)]
enum GrpcConnectorInner {
    #[cfg(feature = "dns-tcp-transport")]
    DnsTcp(
        Uri,
        hyper_util::client::legacy::connect::HttpConnector<crate::dns::DnsResolver>,
    ),
    #[cfg(feature = "dns-tcp-tls-transport")]
    DnsTcpTls(
        Uri,
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector<crate::dns::DnsResolver>>,
    ),
    #[cfg(feature = "unix-transport")]
    Unix(std::sync::Arc<std::path::PathBuf>),
    #[cfg(feature = "vsock-transport")]
    Vsock(u32, u32),
    #[cfg(feature = "custom-transport")]
    Custom(tower::util::BoxCloneSyncService<(), GrpcStream, BoxError>),
}

impl Service<Uri> for GrpcConnector {
    type Response = GrpcStream;

    type Error = BoxError;

    type Future = BoxResultFuture<GrpcStream>;

    fn poll_ready(
        &mut self,
        #[cfg_attr(not(feature = "custom-transport"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match self.inner {
            #[cfg(feature = "dns-tcp-transport")]
            GrpcConnectorInner::DnsTcp(_, ref mut connector) => {
                connector.poll_ready(cx).map_err(|err| Box::new(err) as BoxError)
            }
            #[cfg(feature = "dns-tcp-tls-transport")]
            GrpcConnectorInner::DnsTcpTls(_, ref mut connector) => connector.poll_ready(cx),
            #[cfg(feature = "unix-transport")]
            GrpcConnectorInner::Unix(_) => Poll::Ready(Ok(())),
            #[cfg(feature = "vsock-transport")]
            GrpcConnectorInner::Vsock(_, _) => Poll::Ready(Ok(())),
            #[cfg(feature = "custom-transport")]
            GrpcConnectorInner::Custom(ref mut service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, _uri: Uri) -> Self::Future {
        #[cfg(not(feature = "__transport"))]
        panic!("alternate-tonic-client crate had no transport feature enabled at runtime");

        #[cfg(feature = "__transport")]
        {
            #[cfg(feature = "firecracker-handshake")]
            let firecracker_handshake_port = self.firecracker_handshake_port;

            let future: BoxResultFuture<GrpcStream> = match self.inner {
                #[cfg(feature = "dns-tcp-transport")]
                GrpcConnectorInner::DnsTcp(ref uri, ref mut connector) => {
                    let future = connector.call(uri.clone());

                    Box::pin(async move {
                        #[cfg_attr(not(feature = "firecracker-handshake"), allow(unused_mut))]
                        let mut stream = future.await?;
                        #[cfg(feature = "firecracker-handshake")]
                        perform_firecracker_handshake(firecracker_handshake_port, stream.inner_mut()).await?;

                        Ok(GrpcStream {
                            inner: GrpcStreamInner::DnsTcp(stream),
                        })
                    })
                }
                #[cfg(feature = "dns-tcp-tls-transport")]
                GrpcConnectorInner::DnsTcpTls(ref uri, ref mut connector) => {
                    let future = connector.call(uri.clone());
                    Box::pin(async move { future.await.map(GrpcStream::dns_tcp_tls) })
                }
                #[cfg(feature = "unix-transport")]
                GrpcConnectorInner::Unix(ref socket_path) => {
                    let socket_path = socket_path.clone();

                    Box::pin(async move {
                        #[cfg_attr(not(feature = "firecracker-handshake"), allow(unused_mut))]
                        let mut stream = tokio::net::UnixStream::connect(socket_path.as_ref()).await?;
                        #[cfg(feature = "firecracker-handshake")]
                        perform_firecracker_handshake(firecracker_handshake_port, &mut stream).await?;

                        Ok(GrpcStream {
                            inner: GrpcStreamInner::Unix(hyper_util::rt::tokio::WithHyperIo::new(stream)),
                        })
                    })
                }
                #[cfg(feature = "vsock-transport")]
                GrpcConnectorInner::Vsock(cid, port) => Box::pin(async move {
                    #[cfg_attr(not(feature = "firecracker-handshake"), allow(unused_mut))]
                    let mut stream = tokio_vsock::VsockStream::connect(tokio_vsock::VsockAddr::new(cid, port)).await?;
                    #[cfg(feature = "firecracker-handshake")]
                    perform_firecracker_handshake(firecracker_handshake_port, &mut stream).await?;

                    Ok(GrpcStream {
                        inner: GrpcStreamInner::Vsock(hyper_util::rt::tokio::WithHyperIo::new(stream)),
                    })
                }),
                #[cfg(feature = "custom-transport")]
                GrpcConnectorInner::Custom(ref mut service) => {
                    #[cfg(feature = "firecracker-handshake")]
                    {
                        let future = service.call(());

                        Box::pin(async move {
                            let mut stream = future.await?;

                            perform_firecracker_handshake(
                                firecracker_handshake_port,
                                &mut hyper_util::rt::tokio::WithTokioIo::new(&mut stream),
                            )
                            .await?;

                            Ok(stream)
                        })
                    }

                    #[cfg(not(feature = "firecracker-handshake"))]
                    service.call(())
                }
            };

            match self.timeout {
                Some(timeout) => Box::pin(async move {
                    tokio::time::timeout(timeout, future)
                        .await
                        .map_err(|err| Box::new(err) as BoxError)
                        .flatten()
                }),
                None => future,
            }
        }
    }
}

#[cfg(feature = "firecracker-handshake")]
async fn perform_firecracker_handshake<IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>(
    port: Option<u32>,
    io: &mut IO,
) -> Result<(), std::io::Error> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let Some(port) = port else {
        return Ok(());
    };

    io.write_all(format!("CONNECT {port}\n").as_bytes()).await?;

    let mut lines = BufReader::new(io).lines();
    match lines.next_line().await {
        Ok(Some(line)) => {
            if !line.starts_with("OK") {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "Firecracker refused to establish a tunnel to the given guest port",
                ));
            }
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Could not read Firecracker response",
            ));
        }
    };

    Ok(())
}
