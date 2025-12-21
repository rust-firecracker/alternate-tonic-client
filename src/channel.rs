use std::{
    pin::Pin,
    task::{Context, Poll},
};

use http::{Request, Response, Uri};
use hyper::body::Incoming;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tonic::body::Body;
use tower::{Service, ServiceExt, util::BoxCloneSyncService};

use crate::connector::GrpcConnector;

#[derive(Debug, Clone)]
pub struct GrpcChannel {
    inner: BoxCloneSyncService<Request<Body>, Response<Incoming>, Box<dyn std::error::Error + Send + Sync>>,
}

impl GrpcChannel {
    pub fn new(connector: GrpcConnector) -> Self {
        let mut client_builder = Client::builder(TokioExecutor::new());
        client_builder.http2_only(true);

        let client = client_builder
            .build(connector)
            .map_request(|mut request: Request<Body>| {
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

                request
            })
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>);

        GrpcChannel {
            inner: BoxCloneSyncService::new(client),
        }
    }
}

impl Service<Request<Body>> for GrpcChannel {
    type Response = Response<Incoming>;

    type Error = Box<dyn std::error::Error + Send + Sync>;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        self.inner.call(request)
    }
}
