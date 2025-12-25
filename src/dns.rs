use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use hyper_util::client::legacy::connect::dns::{GaiAddrs, GaiFuture, GaiResolver, Name};
use tower::{BoxError, Service, ServiceExt, util::BoxCloneSyncService};

use crate::BoxResultFuture;

#[derive(Debug, Clone)]
pub struct DnsResolver {
    inner: DnsResolverInner,
}

#[derive(Debug, Clone)]
enum DnsResolverInner {
    Gai(GaiResolver),
    Boxed(BoxCloneSyncService<String, Box<dyn Iterator<Item = SocketAddr>>, BoxError>),
}

impl DnsResolver {
    pub fn new<R>(resolver: R) -> Self
    where
        R: Service<String> + Clone + Send + Sync + 'static,
        R::Response: Iterator<Item = SocketAddr>,
        R::Error: Into<BoxError>,
        R::Future: Send,
    {
        Self {
            inner: DnsResolverInner::Boxed(BoxCloneSyncService::new(
                resolver
                    .map_response(|iter| Box::new(iter) as _)
                    .map_err(|err: R::Error| err.into()),
            )),
        }
    }
}

impl Default for DnsResolver {
    fn default() -> Self {
        Self {
            inner: DnsResolverInner::Gai(GaiResolver::new()),
        }
    }
}

pub struct DnsFuture {
    inner: DnsFutureInner,
}

enum DnsFutureInner {
    Gai(GaiFuture),
    Boxed(BoxResultFuture<Box<dyn Iterator<Item = SocketAddr>>>),
}

impl Future for DnsFuture {
    type Output = Result<DnsAddrs, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.get_mut().inner {
            DnsFutureInner::Gai(future) => Pin::new(future)
                .poll(cx)
                .map_ok(|addrs| DnsAddrs {
                    inner: DnsAddrsInner::Gai(addrs),
                })
                .map_err(|err| Box::new(err) as BoxError),
            DnsFutureInner::Boxed(future) => Pin::new(future).poll(cx).map_ok(|iter| DnsAddrs {
                inner: DnsAddrsInner::Boxed(iter),
            }),
        }
    }
}

pub struct DnsAddrs {
    inner: DnsAddrsInner,
}

enum DnsAddrsInner {
    Gai(GaiAddrs),
    Boxed(Box<dyn Iterator<Item = SocketAddr>>),
}

impl Iterator for DnsAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            DnsAddrsInner::Gai(addrs) => addrs.next(),
            DnsAddrsInner::Boxed(iter) => iter.next(),
        }
    }
}

impl Service<Name> for DnsResolver {
    type Response = DnsAddrs;

    type Error = BoxError;

    type Future = DnsFuture;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match &mut self.inner {
            DnsResolverInner::Gai(resolver) => resolver.poll_ready(cx).map_err(|err| Box::new(err) as _),
            DnsResolverInner::Boxed(service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, name: Name) -> Self::Future {
        match &mut self.inner {
            DnsResolverInner::Gai(resolver) => DnsFuture {
                inner: DnsFutureInner::Gai(resolver.call(name)),
            },
            DnsResolverInner::Boxed(service) => DnsFuture {
                inner: DnsFutureInner::Boxed(service.call(name.to_string())),
            },
        }
    }
}
