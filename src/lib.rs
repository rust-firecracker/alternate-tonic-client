#[cfg(feature = "_channel")]
mod channel;
mod connector;
mod stream;

use std::pin::Pin;

#[cfg(feature = "_channel")]
pub use channel::*;
pub use connector::{GrpcConnector, GrpcConnectorBuilder};
pub use stream::GrpcStream;

type BoxResultFuture<O> =
    Pin<Box<dyn Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static>>;
