use std::{
    task::{Context, Poll},
    time::Duration,
};

use http::Uri;
use tower::{BoxError, Service};

use crate::{BoxResultFuture, stream::GrpcStream};

#[cfg(feature = "dns-tcp-tls-transport")]
type CustomDnsResolver = tower::util::BoxCloneSyncService<
    hyper_util::client::legacy::connect::dns::Name,
    Box<dyn Iterator<Item = std::net::SocketAddr>>,
    BoxError,
>;

#[cfg(feature = "dns-tcp-tls-transport")]
#[derive(Debug, Clone)]
pub struct DnsTlsConfig {
    tls_config: rustls::ClientConfig,
    enforce_tls: bool,
    custom_dns_resolver: Option<CustomDnsResolver>,
}

#[cfg(feature = "dns-tcp-tls-transport")]
impl DnsTlsConfig {
    pub fn new(tls_config: rustls::ClientConfig) -> Self {
        Self {
            tls_config,
            enforce_tls: false,
            custom_dns_resolver: None,
        }
    }

    pub fn enforce_tls(&mut self) {
        self.enforce_tls = true;
    }

    pub fn set_custom_dns_resolver<R>(&mut self, resolver: R)
    where
        R: Service<String> + Clone + Send + Sync + 'static,
        R::Response: Iterator<Item = std::net::SocketAddr>,
        R::Error: Into<BoxError>,
        R::Future: Send,
    {
        self.custom_dns_resolver = Some(CustomDnsResolver::new(
            tower::ServiceBuilder::new()
                .map_request(|name: hyper_util::client::legacy::connect::dns::Name| name.to_string())
                .map_response(|iterator| Box::new(iterator) as Box<dyn Iterator<Item = std::net::SocketAddr>>)
                .map_err(|err: R::Error| err.into())
                .service(resolver),
        ));
    }
}

pub struct GrpcConnectorBuilder {
    timeout: Option<Duration>,
}

impl GrpcConnectorBuilder {
    pub fn new() -> Self {
        Self { timeout: None }
    }
}

impl GrpcConnectorBuilder {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    #[cfg(feature = "dns-tcp-transport")]
    pub fn build_to_tcp_host(self, uri: Uri) -> GrpcConnector {
        self.build(GrpcConnectorInner::DnsTcp(
            uri,
            hyper_util::client::legacy::connect::HttpConnector::new(),
        ))
    }

    #[cfg(feature = "dns-tcp-tls-transport")]
    pub fn build_to_tcp_host_with_tls(self, uri: Uri, dns_tls_config: DnsTlsConfig) -> GrpcConnector {
        let connector = hyper_rustls::HttpsConnectorBuilder::new().with_tls_config(dns_tls_config.tls_config);

        let connector = match dns_tls_config.enforce_tls {
            true => connector.https_only(),
            false => connector.https_or_http(),
        };

        let connector = connector.enable_http2();

        match dns_tls_config.custom_dns_resolver {
            Some(resolver) => self.build(GrpcConnectorInner::DnsTcpTlsWithCustomDnsResolver(
                uri,
                connector.wrap_connector(hyper_util::client::legacy::connect::HttpConnector::new_with_resolver(
                    resolver,
                )),
            )),
            None => self.build(GrpcConnectorInner::DnsTcpTlsWithDefaultDnsResolver(
                uri,
                connector.build(),
            )),
        }
    }

    #[cfg(feature = "unix-transport")]
    pub fn build_to_unix_socket<P: Into<std::path::PathBuf>>(self, socket_path: P) -> GrpcConnector {
        self.build(GrpcConnectorInner::Unix(std::sync::Arc::new(socket_path.into())))
    }

    #[cfg(feature = "vsock-transport")]
    pub fn build_to_vsock_socket(self, cid: u32, port: u32) -> GrpcConnector {
        self.build(GrpcConnectorInner::Vsock(cid, port))
    }

    #[cfg(feature = "custom-transport")]
    pub fn build_custom<S>(self, service: S) -> GrpcConnector
    where
        S: Service<(), Response = GrpcStream, Error = BoxError> + Send + Sync + Clone + 'static,
        S::Future: Send,
    {
        self.build(GrpcConnectorInner::Custom(tower::util::BoxCloneSyncService::new(
            service,
        )))
    }

    #[cfg(feature = "_transport")]
    fn build(self, inner: GrpcConnectorInner) -> GrpcConnector {
        GrpcConnector {
            inner,
            timeout: self.timeout,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcConnector {
    inner: GrpcConnectorInner,
    #[cfg_attr(not(feature = "_transport"), allow(unused))]
    timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
enum GrpcConnectorInner {
    #[cfg(feature = "dns-tcp-transport")]
    DnsTcp(Uri, hyper_util::client::legacy::connect::HttpConnector),
    #[cfg(feature = "dns-tcp-tls-transport")]
    DnsTcpTlsWithDefaultDnsResolver(
        Uri,
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
    ),
    #[cfg(feature = "dns-tcp-tls-transport")]
    DnsTcpTlsWithCustomDnsResolver(
        Uri,
        hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector<CustomDnsResolver>>,
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
            GrpcConnectorInner::DnsTcpTlsWithDefaultDnsResolver(_, ref mut connector) => connector.poll_ready(cx),
            #[cfg(feature = "dns-tcp-tls-transport")]
            GrpcConnectorInner::DnsTcpTlsWithCustomDnsResolver(_, ref mut connector) => connector.poll_ready(cx),
            #[cfg(feature = "unix-transport")]
            GrpcConnectorInner::Unix(_) => Poll::Ready(Ok(())),
            #[cfg(feature = "vsock-transport")]
            GrpcConnectorInner::Vsock(_, _) => Poll::Ready(Ok(())),
            #[cfg(feature = "custom-transport")]
            GrpcConnectorInner::Custom(ref mut service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, _uri: Uri) -> Self::Future {
        #[cfg(not(feature = "_transport"))]
        panic!("alternate-tonic-client crate had no transport feature enabled at runtime");

        #[cfg(feature = "_transport")]
        {
            let future: BoxResultFuture<GrpcStream> = match self.inner {
                #[cfg(feature = "dns-tcp-transport")]
                GrpcConnectorInner::DnsTcp(ref uri, ref mut connector) => {
                    let future = connector.call(uri.clone());

                    Box::pin(async move {
                        future
                            .await
                            .map(GrpcStream::dns_tcp)
                            .map_err(|err| Box::new(err) as BoxError)
                    })
                }
                #[cfg(feature = "dns-tcp-tls-transport")]
                GrpcConnectorInner::DnsTcpTlsWithDefaultDnsResolver(ref uri, ref mut connector) => {
                    let future = connector.call(uri.clone());
                    Box::pin(async move { future.await.map(GrpcStream::dns_tcp_tls) })
                }
                #[cfg(feature = "dns-tcp-tls-transport")]
                GrpcConnectorInner::DnsTcpTlsWithCustomDnsResolver(ref uri, ref mut connector) => {
                    let future = connector.call(uri.clone());
                    Box::pin(async move { future.await.map(GrpcStream::dns_tcp_tls) })
                }
                #[cfg(feature = "unix-transport")]
                GrpcConnectorInner::Unix(ref socket_path) => {
                    let socket_path = socket_path.clone();

                    Box::pin(async move {
                        tokio::net::UnixStream::connect(socket_path.as_ref())
                            .await
                            .map(GrpcStream::unix)
                            .map_err(|err| Box::new(err) as BoxError)
                    })
                }
                #[cfg(feature = "vsock-transport")]
                GrpcConnectorInner::Vsock(cid, port) => Box::pin(async move {
                    tokio_vsock::VsockStream::connect(tokio_vsock::VsockAddr::new(cid, port))
                        .await
                        .map(GrpcStream::vsock)
                        .map_err(|err| Box::new(err) as BoxError)
                }),
                #[cfg(feature = "custom-transport")]
                GrpcConnectorInner::Custom(ref mut service) => service.call(()),
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
