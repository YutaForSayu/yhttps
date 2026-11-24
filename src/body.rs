//! Response body helpers.
//!
//! Every response produced by `yhtps` uses the same body type so that the
//! hyper service is monomorphic: [`ResBody`].

use std::io;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Empty, Full, StreamBody};
use hyper::body::Frame;
use tokio::io::AsyncRead;
use tokio_util::io::ReaderStream;

/// The one body type used everywhere.
pub type ResBody = UnsyncBoxBody<Bytes, io::Error>;

/// A body from an in-memory buffer.
pub fn full(data: impl Into<Bytes>) -> ResBody {
    Full::new(data.into())
        .map_err(|never| match never {})
        .boxed_unsync()
}

/// An empty body (used for HEAD, 204, 304…).
pub fn empty() -> ResBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed_unsync()
}

/// A body streamed from any `AsyncRead` (files, decompressors, proxied bodies…).
pub fn stream<R>(reader: R) -> ResBody
where
    R: AsyncRead + Send + 'static,
{
    let stream = ReaderStream::new(reader).map_ok(Frame::data);
    StreamBody::new(stream).boxed_unsync()
}

/// Adapt an upstream (`hyper::body::Incoming`) body used by the proxy.
pub fn from_incoming(body: hyper::body::Incoming) -> ResBody {
    body.map_err(io::Error::other).boxed_unsync()
}
