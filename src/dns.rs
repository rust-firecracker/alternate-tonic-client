use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use hyper_util::client::legacy::connect::dns::{GaiAddrs, GaiFuture, GaiResolver, Name};
use tower::{BoxError, Service, ServiceExt, util::BoxCloneSyncService};

use crate::BoxResultFuture;

/// A DNS resolver, encapsulating either a default implementation from [hyper_util] that uses [tokio]'s
/// blocking thread pool or a [BoxCloneSyncService] wrapping a custom implementation. This struct is cheaply
/// [Clone]-able and implements [Default] for creating an instance backed by the default implementation.
///
/// In accordance with [hyper_util]'s trait contract, this is a [Service] accepting [hyper_util] [Name]s,
/// though the [Service] implementation is an internal detail that is up to change.
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
    /// Create a [DnsResolver] backed by a [Service] providing a custom implementation of DNS resolution. This
    /// [Service] must accept a [String] hostname as a request, return an implementation of an [Iterator] of
    /// [SocketAddr]s as a response and emit an error that is convertible into a boxed type-erased [std::error::Error].
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

/// A future returned by [DnsResolver]'s [Service] implementation, yielding either [DnsAddrs] or a boxed type-erased
/// [std::error::Error].
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

/// An iterator over [SocketAddr]s yielded by a successful [DnsFuture].
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
