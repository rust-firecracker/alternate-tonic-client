#[cfg(feature = "pooled-channel")]
mod pooled;
#[cfg(feature = "singleton-channel")]
mod singleton;

use http::{Request, Response, Uri};
use hyper::body::Incoming;
#[cfg(feature = "pooled-channel")]
pub use pooled::{PooledGrpcChannel, PooledGrpcChannelBuilder};
#[cfg(feature = "singleton-channel")]
pub use singleton::{SingletonGrpcChannel, SingletonGrpcChannelBuilder};
use tonic::body::Body;

use crate::BoxFuture;

type BoxResponseFuture = BoxFuture<Response<Incoming>>;

fn set_request_uri_scheme_and_authority(request: &mut Request<Body>) {
    *request.uri_mut() = Uri::builder()
        .scheme("http")
        .authority("localhost")
        .path_and_query(
            request
                .uri()
                .path_and_query()
                .expect("No path and query were specified for a gRPC request")
                .clone(),
        )
        .build()
        .expect("Uri builder failed");
}
