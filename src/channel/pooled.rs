use std::{
    pin::Pin,
    task::{Context, Poll},
};

use http::{Request, Response};
use hyper::body::Incoming;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tonic::body::Body;
use tower::{BoxError, Service};

use crate::{GrpcConnector, channel::set_request_uri_scheme_and_authority};

pub struct PooledGrpcChannelBuilder {}

impl PooledGrpcChannelBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self, connector: GrpcConnector) -> PooledGrpcChannel {
        let mut client_builder = Client::builder(TokioExecutor::new());
        client_builder.http2_only(true);

        let client = client_builder.build(connector);

        PooledGrpcChannel { client }
    }
}

pub struct PooledGrpcChannel {
    client: Client<GrpcConnector, Body>,
}

impl Service<Request<Body>> for PooledGrpcChannel {
    type Response = Response<Incoming>;

    type Error = BoxError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.client.poll_ready(cx).map_err(|err| Box::new(err) as BoxError)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        set_request_uri_scheme_and_authority(&mut request);
        let future = self.client.request(request);

        Box::pin(async { future.await.map_err(|err| Box::new(err) as BoxError) })
    }
}
