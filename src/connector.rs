use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use http::Uri;
use tower::{Layer, Service, timeout::TimeoutLayer, util::BoxCloneSyncService};

use crate::stream::GrpcStream;

#[derive(Debug, Clone)]
pub struct GrpcConnector {
    inner: BoxCloneSyncService<Uri, GrpcStream, Box<dyn std::error::Error + Send + Sync>>,
}

impl GrpcConnector {
    #[cfg(feature = "unix-connector")]
    pub fn to_unix_socket<P: Into<std::path::PathBuf>>(socket_path: P) -> GrpcConnector {
        let socket_path = std::sync::Arc::new(socket_path.into());

        Self::new(move |_| {
            let socket_path = socket_path.clone();

            async move {
                let stream = tokio::net::UnixStream::connect(socket_path.as_ref()).await?;
                Ok(GrpcStream::unix(stream))
            }
        })
    }

    pub fn timeout(self, timeout: Duration) -> Self {
        self.layer(TimeoutLayer::new(timeout))
    }

    pub fn layer<L>(self, layer: L) -> Self
    where
        L: Layer<GrpcConnector>,
        L::Service: Clone
            + Send
            + Sync
            + 'static
            + Service<Uri, Response = GrpcStream, Error = Box<dyn std::error::Error + Send + Sync>>,
        <L::Service as Service<Uri>>::Future: Send,
    {
        Self {
            inner: BoxCloneSyncService::new(layer.layer(self)),
        }
    }

    pub fn option_layer<L>(self, layer: Option<L>) -> Self
    where
        L: Layer<GrpcConnector>,
        L::Service: Clone
            + Send
            + Sync
            + 'static
            + Service<Uri, Response = GrpcStream, Error = Box<dyn std::error::Error + Send + Sync>>,
        <L::Service as Service<Uri>>::Future: Send,
    {
        match layer {
            Some(layer) => self.layer(layer),
            None => self,
        }
    }

    #[cfg(feature = "_connector")]
    fn new<
        F: Send + Sync + Clone + 'static + Fn(Uri) -> Fut,
        Fut: Send + 'static + Future<Output = Result<GrpcStream, Box<dyn std::error::Error + Send + Sync + 'static>>>,
    >(
        function: F,
    ) -> GrpcConnector {
        GrpcConnector {
            inner: BoxCloneSyncService::new(tower::service_fn(function)),
        }
    }
}

impl Service<Uri> for GrpcConnector {
    type Response = GrpcStream;

    type Error = Box<dyn std::error::Error + Send + Sync>;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        self.inner.call(uri)
    }
}
