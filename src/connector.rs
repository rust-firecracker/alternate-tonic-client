use std::{
    pin::Pin,
    task::{Context, Poll},
};

use http::Uri;
use tower::{BoxError, Service};

use crate::stream::GrpcStream;

#[derive(Debug, Clone)]
pub struct GrpcConnector {
    inner: GrpcConnectorInner,
}

#[derive(Debug, Clone)]
enum GrpcConnectorInner {
    #[cfg(feature = "unix-transport")]
    Unix(std::sync::Arc<std::path::PathBuf>),
    #[cfg(feature = "custom-transport")]
    Custom(tower::util::BoxCloneSyncService<Uri, GrpcStream, BoxError>),
}

impl GrpcConnector {
    #[cfg(feature = "unix-transport")]
    pub fn to_unix_socket<P: Into<std::path::PathBuf>>(socket_path: P) -> GrpcConnector {
        GrpcConnector {
            inner: GrpcConnectorInner::Unix(std::sync::Arc::new(socket_path.into())),
        }
    }
}

impl Service<Uri> for GrpcConnector {
    type Response = GrpcStream;

    type Error = BoxError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match self.inner {
            #[cfg(feature = "unix-transport")]
            GrpcConnectorInner::Unix(_) => Poll::Ready(Ok(())),
            #[cfg(feature = "custom-transport")]
            GrpcConnectorInner::Custom(ref mut service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, #[cfg_attr(not(feature = "_transport"), allow(unused))] uri: Uri) -> Self::Future {
        match self.inner {
            #[cfg(feature = "unix-transport")]
            GrpcConnectorInner::Unix(ref socket_path) => {
                let socket_path = socket_path.clone();

                Box::pin(async move {
                    let stream = tokio::net::UnixStream::connect(socket_path.as_ref()).await?;
                    Ok(GrpcStream::unix(stream))
                })
            }
            #[cfg(feature = "custom-transport")]
            GrpcConnectorInner::Custom(ref mut service) => service.call(uri),
        }
    }
}
