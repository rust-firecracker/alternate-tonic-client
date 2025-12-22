use std::{
    pin::Pin,
    task::{Context, Poll},
};

use http::{Request, Response};
use hyper::body::Incoming;
use tonic::body::Body;
use tower::{
    BoxError, Service,
    buffer::Buffer,
    reconnect::{Reconnect, ResponseFuture},
};

use crate::{GrpcConnector, channel::set_request_uri_scheme_and_authority};

#[derive(Clone)]
struct SingletonService {
    send_request: hyper::client::conn::http2::SendRequest<Body>,
}

impl tower::Service<Request<Body>> for SingletonService {
    type Response = http::Response<hyper::body::Incoming>;

    type Error = Box<dyn std::error::Error + Send + Sync>;

    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.send_request
            .poll_ready(cx)
            .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        set_request_uri_scheme_and_authority(&mut request);
        let future = self.send_request.send_request(request);

        Box::pin(async {
            future
                .await
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error + Send + Sync>)
        })
    }
}

struct SingletonConnectService {
    connector: GrpcConnector,
}

impl tower::Service<()> for SingletonConnectService {
    type Response = SingletonService;

    type Error = Box<dyn std::error::Error + Send + Sync>;

    type Future = std::pin::Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.connector.poll_ready(cx)
    }

    fn call(&mut self, _: ()) -> Self::Future {
        let mut connector = self.connector.clone();

        Box::pin(async move {
            let stream = connector.call(http::Uri::from_static("http://localhost")).await?;

            let (send_request, connection) =
                hyper::client::conn::http2::handshake::<_, _, Body>(hyper_util::rt::TokioExecutor::new(), stream)
                    .await?;

            tokio::task::spawn(connection);

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(SingletonService { send_request })
        })
    }
}

pub struct SingletonGrpcChannelBuilder {
    buffer_size: usize,
}

impl SingletonGrpcChannelBuilder {
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    pub fn build(self, connector: GrpcConnector) -> SingletonGrpcChannel {
        let service = Reconnect::new(SingletonConnectService { connector }, ());
        let buffer = Buffer::new(service, self.buffer_size);

        SingletonGrpcChannel { buffer }
    }
}

pub struct SingletonGrpcChannel {
    buffer: Buffer<
        Request<Body>,
        ResponseFuture<Pin<Box<dyn Future<Output = Result<Response<Incoming>, BoxError>> + Send + 'static>>, BoxError>,
    >,
}

impl Service<Request<Body>> for SingletonGrpcChannel {
    type Response = Response<Incoming>;

    type Error = BoxError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.buffer.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        Box::pin(self.buffer.call(request))
    }
}
