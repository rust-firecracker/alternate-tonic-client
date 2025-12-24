use std::{
    task::{Context, Poll},
    time::Duration,
};

use http::Uri;
use tower::{BoxError, Service};

use crate::{BoxResultFuture, stream::GrpcStream};

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
        S: Service<Uri, Response = GrpcStream, Error = BoxError> + Send + Sync + Clone + 'static,
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
    #[cfg(feature = "unix-transport")]
    Unix(std::sync::Arc<std::path::PathBuf>),
    #[cfg(feature = "vsock-transport")]
    Vsock(u32, u32),
    #[cfg(feature = "custom-transport")]
    Custom(tower::util::BoxCloneSyncService<Uri, GrpcStream, BoxError>),
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
            #[cfg(feature = "unix-transport")]
            GrpcConnectorInner::Unix(_) => Poll::Ready(Ok(())),
            #[cfg(feature = "vsock-transport")]
            GrpcConnectorInner::Vsock(_, _) => Poll::Ready(Ok(())),
            #[cfg(feature = "custom-transport")]
            GrpcConnectorInner::Custom(ref mut service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, #[cfg_attr(not(feature = "custom-transport"), allow(unused))] uri: Uri) -> Self::Future {
        #[cfg(not(feature = "_transport"))]
        panic!("alternate-tonic-client crate had no transport feature enabled at runtime");

        #[cfg(feature = "_transport")]
        {
            let future = match self.inner {
                #[cfg(feature = "unix-transport")]
                GrpcConnectorInner::Unix(ref socket_path) => {
                    let socket_path = socket_path.clone();

                    Box::pin(async move {
                        let stream = tokio::net::UnixStream::connect(socket_path.as_ref()).await?;
                        Ok(GrpcStream::unix(stream))
                    })
                }
                #[cfg(feature = "vsock-transport")]
                GrpcConnectorInner::Vsock(cid, port) => Box::pin(async move {
                    let stream = tokio_vsock::VsockStream::connect(tokio_vsock::VsockAddr::new(cid, port)).await?;
                    Ok(GrpcStream::vsock(stream))
                }) as BoxResultFuture<GrpcStream>,
                #[cfg(feature = "custom-transport")]
                GrpcConnectorInner::Custom(ref mut service) => service.call(uri),
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
