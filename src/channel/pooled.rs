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

#[derive(Debug, Clone)]
pub struct PooledGrpcChannelBuilder {
    timeout: Option<Duration>,
    client_builder: Builder,
}

impl PooledGrpcChannelBuilder {
    pub fn new() -> Self {
        Self {
            timeout: None,
            client_builder: Builder::new(TokioExecutor::new()),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(mut self, connector: GrpcConnector) -> PooledGrpcChannel {
        self.client_builder.http2_only(true);
        let client = self.client_builder.build(connector);

        PooledGrpcChannel {
            client,
            timeout: self.timeout,
        }
    }
}

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
