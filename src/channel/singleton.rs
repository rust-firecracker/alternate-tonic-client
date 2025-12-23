use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use http::{Request, Response};
use hyper::body::Incoming;
use hyper_util::rt::TokioExecutor;
use tonic::body::Body;
use tower::{
    BoxError, Service, ServiceBuilder, buffer::Buffer, reconnect::Reconnect, timeout::TimeoutLayer,
    util::BoxCloneSyncService,
};

use crate::{GrpcConnector, channel::set_request_uri_scheme_and_authority};

type Http2ConnectionBuilder = hyper::client::conn::http2::Builder<TokioExecutor>;

#[derive(Clone)]
struct SingletonService {
    send_request: hyper::client::conn::http2::SendRequest<Body>,
    timeout: Option<Duration>,
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

        match self.timeout {
            Some(timeout) => Box::pin(async move {
                match tokio::time::timeout(timeout, future).await {
                    Ok(Ok(response)) => Ok(response),
                    Ok(Err(err)) => Err(Box::new(err) as BoxError),
                    Err(err) => Err(Box::new(err) as BoxError),
                }
            }),
            None => Box::pin(async move { future.await.map_err(|err| Box::new(err) as BoxError) }),
        }
    }
}

struct SingletonConnectService {
    connector: GrpcConnector,
    connection_builder: Http2ConnectionBuilder,
    timeout: Option<Duration>,
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
        let connection_builder = self.connection_builder.clone();
        let timeout = self.timeout;

        Box::pin(async move {
            let stream = connector.call(http::Uri::from_static("http://localhost")).await?;
            let (send_request, connection) = connection_builder.handshake(stream).await?;

            tokio::task::spawn(connection);

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(SingletonService { send_request, timeout })
        })
    }
}

pub struct SingletonGrpcChannelBuilder {
    buffer_size: usize,
    connection_builder: Http2ConnectionBuilder,
    timeout: Option<Duration>,
}

impl SingletonGrpcChannelBuilder {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            connection_builder: Http2ConnectionBuilder::new(TokioExecutor::new()),
            timeout: None,
        }
    }

    pub fn configure_http2_connection<F: FnOnce(&mut Http2ConnectionBuilder)>(mut self, function: F) -> Self {
        function(&mut self.connection_builder);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self, connector: GrpcConnector) -> SingletonGrpcChannel {
        let service = ServiceBuilder::new()
            .option_layer(self.timeout.map(TimeoutLayer::new))
            .service(Reconnect::new(
                SingletonConnectService {
                    connector,
                    connection_builder: self.connection_builder,
                    timeout: self.timeout,
                },
                (),
            ));

        let buffer = BoxCloneSyncService::new(Buffer::new(service, self.buffer_size));

        SingletonGrpcChannel { buffer }
    }
}

pub struct SingletonGrpcChannel {
    buffer: BoxCloneSyncService<Request<Body>, Response<Incoming>, BoxError>,
}

impl Service<Request<Body>> for SingletonGrpcChannel {
    type Response = Response<Incoming>;

    type Error = BoxError;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.buffer.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        self.buffer.call(request)
    }
}
