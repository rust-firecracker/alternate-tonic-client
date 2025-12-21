use std::{
    pin::Pin,
    task::{Context, Poll},
};

use hyper::rt::{Read, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};

#[cfg(feature = "custom-connector")]
pub trait HyperIo: hyper::rt::Read + hyper::rt::Write + Unpin + Send {}

#[cfg(feature = "custom-connector")]
impl<S> HyperIo for S where S: hyper::rt::Read + hyper::rt::Write + Unpin + Send {}

pub struct GrpcStream {
    #[cfg_attr(not(feature = "_connector"), allow(unused))]
    inner: GrpcStreamInner,
}

impl GrpcStream {
    #[cfg(feature = "unix-connector")]
    pub(crate) fn unix(stream: tokio::net::UnixStream) -> Self {
        Self {
            inner: GrpcStreamInner::Unix(hyper_util::rt::TokioIo::new(stream)),
        }
    }
}

pub enum GrpcStreamInner {
    #[cfg(feature = "unix-connector")]
    Unix(hyper_util::rt::TokioIo<tokio::net::UnixStream>),
    #[cfg(feature = "custom-connector")]
    Custom(Box<dyn HyperIo>),
}

impl Read for GrpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] cx: &mut Context<'_>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(feature = "_connector"))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_connector")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-connector")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(feature = "custom-connector")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl Write for GrpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] cx: &mut Context<'_>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        #[cfg(not(feature = "_connector"))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_connector")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-connector")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
            #[cfg(feature = "custom-connector")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(any(feature = "unix-connector", feature = "custom-connector")))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_connector")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-connector")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(feature = "custom-connector")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_connector"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(any(feature = "unix-connector", feature = "custom-connector")))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_connector")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-connector")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(feature = "custom-connector")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl Connection for GrpcStream {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}
