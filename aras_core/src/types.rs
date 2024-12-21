use std::pin::Pin;

use futures::Future;
use hyper::body::Body;
use http_body_util::combinators::BoxBody;
use bytes::Bytes;

use crate::error::Error;

pub trait ArasBody: Body + Send + Sync {}

impl ArasBody for hyper::body::Incoming {}

impl ArasBody for String {}

pub type Response = hyper::Response<BoxBody<Bytes, hyper::Error>>;
pub type ServiceFuture = Pin<Box<dyn Future<Output = std::result::Result<Response, Error>> + Send + Sync>>;