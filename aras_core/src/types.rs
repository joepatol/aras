use std::pin::Pin;

use futures::Future;

use http_body_util::combinators::BoxBody;
use bytes::Bytes;

use crate::error::Error;

pub type Response = hyper::Response<BoxBody<Bytes, hyper::Error>>;
pub type Request = hyper::Request<hyper::body::Incoming>;
pub type ServiceFuture = Pin<Box<dyn Future<Output = std::result::Result<Response, Error>> + Send + Sync>>;