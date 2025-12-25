use std::{
    task::{Context, Poll},
    time::Duration,
};

use http::{Request, Response};
use hyper::body::Incoming;
use hyper_util::{
    client::legacy::{Builder, Client},
    rt::TokioExecutor,
};
use tonic::body::Body;
use tower::{BoxError, Service};

use crate::{BoxResultFuture, GrpcConnector, channel::set_request_uri_scheme_and_authority};

/// A builder for a [PooledGrpcChannel].
#[derive(Debug, Clone)]
pub struct PooledGrpcChannelBuilder {
    timeout: Option<Duration>,
    client_builder: Builder,
}

impl PooledGrpcChannelBuilder {
    /// Create a new [PooledGrpcChannelBuilder].
    pub fn new() -> Self {
        Self {
            timeout: None,
            client_builder: Builder::new(TokioExecutor::new()),
        }
    }

    /// Set a timeout [Duration] for all requests performed on the resulting [PooledGrpcChannel].
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.client_builder.pool_idle_timeout(timeout);
        self
    }

    pub fn max_idle_connections(mut self, max: usize) -> Self {
        self.client_builder.pool_max_idle_per_host(max);
        self
    }

    pub fn http2_max_pending_accept_reset_streams(mut self, max: usize) -> Self {
        self.client_builder.http2_max_pending_accept_reset_streams(max);
        self
    }

    pub fn http2_initial_stream_window_size(mut self, size: u32) -> Self {
        self.client_builder.http2_initial_stream_window_size(size);
        self
    }

    pub fn http2_initial_connection_window_size(mut self, size: u32) -> Self {
        self.client_builder.http2_initial_connection_window_size(size);
        self
    }

    pub fn http2_initial_max_send_streams(mut self, initial: usize) -> Self {
        self.client_builder.http2_initial_max_send_streams(initial);
        self
    }

    pub fn http2_adaptive_window(mut self, enabled: bool) -> Self {
        self.client_builder.http2_adaptive_window(enabled);
        self
    }

    pub fn http2_max_frame_size(mut self, size: u32) -> Self {
        self.client_builder.http2_max_frame_size(size);
        self
    }

    pub fn http2_max_header_list_size(mut self, size: u32) -> Self {
        self.client_builder.http2_max_header_list_size(size);
        self
    }

    pub fn http2_keep_alive_interval(mut self, interval: Duration) -> Self {
        self.client_builder.http2_keep_alive_interval(interval);
        self
    }

    pub fn http2_keep_alive_timeout(mut self, timeout: Duration) -> Self {
        self.client_builder.http2_keep_alive_timeout(timeout);
        self
    }

    pub fn http2_keep_alive_while_idle(mut self, enabled: bool) -> Self {
        self.client_builder.http2_keep_alive_while_idle(enabled);
        self
    }

    pub fn http2_max_concurrent_reset_streams(mut self, max: usize) -> Self {
        self.client_builder.http2_max_concurrent_reset_streams(max);
        self
    }

    /// Build a [PooledGrpcChannel] backed by the given [GrpcConnector].
    pub fn build(mut self, connector: GrpcConnector) -> PooledGrpcChannel {
        self.client_builder.http2_only(true);
        let client = self.client_builder.build(connector);

        PooledGrpcChannel {
            client,
            timeout: self.timeout,
        }
    }
}

/// A gRPC channel [Service] compatible with [tonic] that is backed by a dynamic HTTP/2 connection
/// pool. Currently, [hyper_util]'s legacy client is used as this pool, but this may change in the future while
/// preserving the backwards compatibility and functionality of [PooledGrpcChannel]. To use this channel with
/// [tonic] for performing requests, create a [tonic::client::Grpc] instance wrapping it or a code-generated client
/// struct wrapping it.
#[derive(Debug, Clone)]
pub struct PooledGrpcChannel {
    client: Client<GrpcConnector, Body>,
    timeout: Option<Duration>,
}

impl Service<Request<Body>> for PooledGrpcChannel {
    type Response = Response<Incoming>;

    type Error = BoxError;

    type Future = BoxResultFuture<Response<Incoming>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx).map_err(|err| Box::new(err) as BoxError)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        set_request_uri_scheme_and_authority(&mut request);
        let future = self.client.request(request);

        match self.timeout {
            Some(timeout) => Box::pin(async move {
                match tokio::time::timeout(timeout, future).await {
                    Ok(Ok(response)) => Ok(response),
                    Ok(Err(err)) => Err(Box::new(err) as BoxError),
                    Err(err) => Err(Box::new(err) as BoxError),
                }
            }),
            None => Box::pin(async { future.await.map_err(|err| Box::new(err) as BoxError) }),
        }
    }
}
