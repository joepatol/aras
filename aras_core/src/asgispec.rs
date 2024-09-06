use std::future::Future;
use std::sync::Arc;

use hyper::Request;

use crate::http::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope};
use crate::lifespan::{
    LifespanScope, LifespanShutdown, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartup,
    LifespanStartupComplete, LifespanStartupFailed,
};
use crate::websocket::{
    WebsocketAcceptEvent, WebsocketCloseEvent, WebsocketConnectEvent, WebsocketDisconnectEvent, WebsocketReceiveEvent,
    WebsocketScope, WebsocketSendEvent,
};
use crate::error::Result;

pub const ASGI_VERSION: &str = "3.0";

pub type SendFn = Arc<dyn Fn(ASGIMessage) -> Box<dyn Future<Output = Result<()>> + Unpin + Sync + Send> + Send + Sync>;

pub type ReceiveFn = Arc<dyn Fn() -> Box<dyn Future<Output = Result<ASGIMessage>> + Unpin + Sync + Send> + Send + Sync>;

pub trait ASGICallable: Send + Sync {
    fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> impl Future<Output = Result<()>> + Send + Sync;
}

#[derive(Debug)]
pub enum Scope {
    HTTP(HTTPScope),
    Lifespan(LifespanScope),
    Websocket(WebsocketScope),
}

impl From<&Request<hyper::body::Incoming>> for Scope {
    fn from(value: &Request<hyper::body::Incoming>) -> Self {
        if let Some(header_value) = value.headers().get("Upgrade") {
            if header_value == "Websocket" {
                return Self::Websocket(WebsocketScope::from(value));
            }
        };
        Self::HTTP(HTTPScope::from(value))
    }
}

#[derive(Debug)]
pub enum ASGIMessage {
    Startup(LifespanStartup),
    StartupComplete(LifespanStartupComplete),
    StartupFailed(LifespanStartupFailed),
    Shutdown(LifespanShutdown),
    ShutdownComplete(LifespanShutdownComplete),
    ShutdownFailed(LifespanShutdownFailed),
    HTTPRequest(HTTPRequestEvent),
    HTTPResponseStart(HTTPResponseStartEvent),
    HTTPResponseBody(HTTPResonseBodyEvent),
    HTTPDisconnect(HTTPDisconnectEvent),
    WebsocketAccept(WebsocketAcceptEvent),
    WebsocketClose(WebsocketCloseEvent),
    WebsocketConnect(WebsocketConnectEvent),
    WebsocketDisconnect(WebsocketDisconnectEvent),
    WebsocketReceive(WebsocketReceiveEvent),
    WebsocketSend(WebsocketSendEvent),
}

#[derive(Debug)]
pub enum SupportedASGISpecVersion {
    V2_4,
}

impl From<SupportedASGISpecVersion> for String {
    fn from(value: SupportedASGISpecVersion) -> Self {
        match value {
            SupportedASGISpecVersion::V2_4 => "2.4".into(),
        }
    }
}

#[derive(Debug)]
pub struct ASGIScope {
    pub version: String,
    pub spec_version: SupportedASGISpecVersion,
}

impl ASGIScope {
    pub fn new() -> Self {
        Self {
            version: ASGI_VERSION.to_string(),
            spec_version: SupportedASGISpecVersion::V2_4,
        }
    }
}
