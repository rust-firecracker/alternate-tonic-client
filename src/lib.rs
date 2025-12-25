#[cfg(feature = "_channel")]
mod channel;
mod connector;
mod stream;

use std::pin::Pin;

#[cfg(feature = "_channel")]
pub use channel::*;
pub use connector::*;
pub use stream::GrpcStream;

#[cfg(any(feature = "dns-tcp-transport", feature = "dns-tcp-tls-transport"))]
mod dns;
#[cfg(any(feature = "dns-tcp-transport", feature = "dns-tcp-tls-transport"))]
pub use dns::*;

#[cfg(any(feature = "dns-tcp-transport", feature = "dns-tcp-tls-transport"))]
mod tcp;
#[cfg(any(feature = "dns-tcp-transport", feature = "dns-tcp-tls-transport"))]
pub use tcp::*;

#[cfg(feature = "dns-tcp-tls-transport")]
mod tls;
#[cfg(feature = "dns-tcp-tls-transport")]
pub use tls::*;

type BoxResultFuture<O> =
    Pin<Box<dyn Future<Output = Result<O, Box<dyn std::error::Error + Send + Sync>>> + Send + 'static>>;
