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
pub const ASGI_SPEC_VERSION: &str = "2.4";

pub type SendFn = Arc<dyn Fn(ASGIMessage) -> Box<dyn Future<Output = Result<()>> + Unpin + Sync + Send> + Send + Sync>;

pub type ReceiveFn = Arc<dyn Fn() -> Box<dyn Future<Output = Result<ASGIMessage>> + Unpin + Sync + Send> + Send + Sync>;

pub trait ASGICallable: Send + Sync {
    fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> impl Future<Output = Result<()>> + Send + Sync;
}

#[derive(Debug, Clone)]
pub enum Scope {
    HTTP(HTTPScope),
    Lifespan(LifespanScope),
    Websocket(WebsocketScope),
}

impl From<&Request<hyper::body::Incoming>> for Scope {
    fn from(value: &Request<hyper::body::Incoming>) -> Self {
        if let Some(header_value) = value.headers().get("upgrade") {
            if header_value == "websocket" {
                return Self::Websocket(WebsocketScope::from(value));
            }
        };
        Self::HTTP(HTTPScope::from(value))
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scope::HTTP(s) => write!(f, "{}", s),
            Scope::Websocket(s) => write!(f, "{:?}", s),
            Scope::Lifespan(s) => write!(f, "{:?}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ASGIScope {
    pub version: String,
    pub spec_version: String,
}

impl ASGIScope {
    pub fn new() -> Self {
        Self {
            version: ASGI_VERSION.to_string(),
            spec_version: ASGI_SPEC_VERSION.to_string(),
        }
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

impl std::fmt::Display for ASGIMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ASGIMessage::Startup(s) => write!(f, "{:?}", s),
            ASGIMessage::StartupComplete(s) => write!(f, "{:?}", s),
            ASGIMessage::StartupFailed(s) => write!(f, "{:?}", s),
            ASGIMessage::Shutdown(s) => write!(f, "{:?}", s),
            ASGIMessage::ShutdownComplete(s) => write!(f, "{:?}", s),
            ASGIMessage::ShutdownFailed(s) => write!(f, "{:?}", s),
            ASGIMessage::HTTPRequest(s) => write!(f, "{}", s),
            ASGIMessage::HTTPResponseStart(s) => write!(f, "{:?}", s),
            ASGIMessage::HTTPResponseBody(s) => write!(f, "{}", s),
            ASGIMessage::HTTPDisconnect(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketAccept(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketClose(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketConnect(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketDisconnect(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketReceive(s) => write!(f, "{:?}", s),
            ASGIMessage::WebsocketSend(s) => write!(f, "{:?}", s),
        }
    }
}