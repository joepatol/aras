use std::pin::Pin;

use bytes::Bytes;
use futures::Future;
use http_body_util::combinators::BoxBody;
use hyper::body::Body;

use crate::error::Error;

pub trait SendSyncBody: Body + Send + Sync {}

impl SendSyncBody for hyper::body::Incoming {}

impl SendSyncBody for String {}


pub type Response = hyper::Response<BoxBody<Bytes, hyper::Error>>;
pub type ServiceFuture = Pin<Box<dyn Future<Output = std::result::Result<Response, Error>> + Send + Sync>>;
