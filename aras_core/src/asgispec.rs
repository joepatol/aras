use std::future::Future;
use std::sync::Arc;

use crate::http1_1::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent};
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

pub enum Scope {
    HTTP(HTTPScope),
}

pub enum ASGIMessage {
    // Temporary events for testing
    
    // Actual ASGI events
    HTTPRequest(HTTPRequestEvent),
    HTTPResponseStart(HTTPResponseStartEvent),
    HTTPResponseBody(HTTPResonseBodyEvent),
    HTTPDisconnect(HTTPDisconnectEvent),
}

pub enum SupportedASGISpecVersion {
    V2_0,
}

impl From<SupportedASGISpecVersion> for String {
    fn from(value: SupportedASGISpecVersion) -> Self {
        match value {
            SupportedASGISpecVersion::V2_0 => "2.0".into(),
        }
    }
}

pub enum HTTPVersion {
    V1_1,
}

impl From<HTTPVersion> for String {
    fn from(value: HTTPVersion) -> Self {
        match value {
            HTTPVersion::V1_1 => "1.1".into(),
        }
    }
}

pub struct ASGIScope {
    pub version: String,
    pub spec_version: SupportedASGISpecVersion,
}

impl ASGIScope {
    pub fn new() -> Self {
        Self { version: ASGI_VERSION.to_string(), spec_version: SupportedASGISpecVersion::V2_0 }
    }
}