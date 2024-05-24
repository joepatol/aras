use std::future::Future;
use std::sync::Arc;

use serde::{Serialize, Deserialize};

use crate::http1_1::HTTPRequestEvent;
use crate::{error::Result, http1_1::HTTPScope};

pub const ASGI_VERSION: &str = "3.0";

pub type SendFn = Arc<
    dyn Fn(ASGIMessage) -> Box<dyn Future<Output = Result<()>> + Unpin + Sync + Send> + Send + Sync,
>;

pub type ReceiveFn =
    Arc<dyn Fn() -> Box<dyn Future<Output = Result<ASGIMessage>> + Unpin + Sync + Send> + Send + Sync>;

pub trait ASGIApplication {
    fn call(
        &self,
        scope: Scope,
        receive: ReceiveFn,
        send: SendFn,
    ) -> impl Future<Output = Result<()>> + Send + Sync;
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Scope {
    HTTP(HTTPScope),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ASGIMessage {
    HTTPRequest(HTTPRequestEvent),
    HTTPResponse(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SupportedASGISpecVersion {
    #[serde(rename = "2.0")]
    V2_0,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum HTTPVersion {
    #[serde(rename = "1.1")]
    V1_1,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ASGIScope {
    version: String,
    spec_version: SupportedASGISpecVersion,
}

impl ASGIScope {
    pub fn new() -> Self {
        Self { version: ASGI_VERSION.to_string(), spec_version: SupportedASGISpecVersion::V2_0 }
    }
}