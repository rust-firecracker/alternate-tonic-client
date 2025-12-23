use std::{
    pin::Pin,
    task::{Context, Poll},
};

use hyper::rt::{Read, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};

#[cfg(feature = "custom-transport")]
trait HyperIo: hyper::rt::Read + hyper::rt::Write + Unpin + Send {}

#[cfg(feature = "custom-transport")]
impl<S> HyperIo for S where S: hyper::rt::Read + hyper::rt::Write + Unpin + Send {}

pub struct GrpcStream {
    #[cfg_attr(not(feature = "_transport"), allow(unused))]
    inner: GrpcStreamInner,
}

enum GrpcStreamInner {
    #[cfg(feature = "unix-transport")]
    Unix(hyper_util::rt::tokio::WithHyperIo<tokio::net::UnixStream>),
    #[cfg(feature = "custom-transport")]
    Custom(Box<dyn HyperIo>),
}

impl GrpcStream {
    #[cfg(feature = "custom-transport")]
    pub fn wrap_hyper_io<IO: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static>(io: IO) -> Self {
        Self {
            inner: GrpcStreamInner::Custom(Box::new(io)),
        }
    }

    #[cfg(feature = "custom-transport")]
    pub fn wrap_tokio_io<IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static>(io: IO) -> Self {
        Self::wrap_hyper_io(hyper_util::rt::tokio::WithHyperIo::new(io))
    }

    #[cfg(feature = "unix-transport")]
    pub(crate) fn unix(stream: tokio::net::UnixStream) -> Self {
        Self {
            inner: GrpcStreamInner::Unix(hyper_util::rt::tokio::WithHyperIo::new(stream)),
        }
    }
}

impl Read for GrpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] cx: &mut Context<'_>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(feature = "_transport"))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_transport")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-transport")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
            #[cfg(feature = "custom-transport")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl Write for GrpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] cx: &mut Context<'_>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        #[cfg(not(feature = "_transport"))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_transport")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-transport")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
            #[cfg(feature = "custom-transport")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(any(feature = "unix-transport", feature = "custom-transport")))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_transport")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-transport")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_flush(cx),
            #[cfg(feature = "custom-transport")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        #[cfg_attr(not(feature = "_transport"), allow(unused))] cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        #[cfg(not(any(feature = "unix-transport", feature = "custom-transport")))]
        panic!("pooled-tonic-client crate had no enabled connector feature at runtime");

        #[cfg(feature = "_transport")]
        match &mut self.get_mut().inner {
            #[cfg(feature = "unix-transport")]
            GrpcStreamInner::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
            #[cfg(feature = "custom-transport")]
            GrpcStreamInner::Custom(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl Connection for GrpcStream {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}
