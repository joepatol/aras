use std::future::Future;
use std::sync::Arc;

use crate::http::*;
use crate::lifespan::*;
use crate::websocket::*;
use crate::error::Result;

pub const ASGI_VERSION: &str = "3.0";
pub const ASGI_SPEC_VERSION: &str = "2.4";

pub type SendFn = Arc<dyn Fn(ASGIMessage) -> Box<dyn Future<Output = Result<()>> + Unpin + Sync + Send> + Send + Sync>;

pub type ReceiveFn = Arc<dyn Fn() -> Box<dyn Future<Output = Result<ASGIMessage>> + Unpin + Sync + Send> + Send + Sync>;

pub trait ASGICallable: Send + Sync + Clone {
    fn call(&self, scope: Scope, receive: ReceiveFn, send: SendFn) -> impl Future<Output = Result<()>> + Send + Sync;
}

#[derive(Debug, Clone)]
pub enum Scope {
    HTTP(HTTPScope),
    Lifespan(LifespanScope),
    Websocket(WebsocketScope),
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

impl ASGIMessage {
    pub fn new_lifespan_startup() -> Self {
        ASGIMessage::Startup(LifespanStartup::new())
    }

    pub fn new_startup_complete() -> Self {
        ASGIMessage::StartupComplete(LifespanStartupComplete::new())
    }

    pub fn new_startup_failed(message: String) -> Self {
        ASGIMessage::StartupFailed(LifespanStartupFailed::new(message))
    }

    pub fn new_lifespan_shutdown() -> Self {
        ASGIMessage::Shutdown(LifespanShutdown::new())
    }

    pub fn new_shutdown_complete() -> Self {
        ASGIMessage::ShutdownComplete(LifespanShutdownComplete::new())
    }

    pub fn new_shutdown_failed(message: String) -> Self {
        ASGIMessage::ShutdownFailed(LifespanShutdownFailed::new(message))
    }

    pub fn new_http_request(data: Vec<u8>, more_body: bool) -> Self {
        ASGIMessage::HTTPRequest(HTTPRequestEvent::new(data, more_body))
    }

    pub fn new_http_response_start(status: u16, headers: Vec<(Vec<u8>, Vec<u8>)>)-> Self {
        ASGIMessage::HTTPResponseStart(HTTPResponseStartEvent::new(status, headers))
    }

    pub fn new_http_response_body(data: Vec<u8>, more_body: bool) -> Self {
        ASGIMessage::HTTPResponseBody(HTTPResonseBodyEvent::new(data, more_body))
    }

    pub fn new_http_disconnect() -> Self {
        ASGIMessage::HTTPDisconnect(HTTPDisconnectEvent::new())
    }

    pub fn new_websocket_accept(subprotocol: Option<String>, headers: Vec<(Vec<u8>, Vec<u8>)>,) -> Self {
        ASGIMessage::WebsocketAccept(WebsocketAcceptEvent::new(subprotocol, headers))
    }

    pub fn new_websocket_close(code: Option<usize>, reason: String) -> Self {
        ASGIMessage::WebsocketClose(WebsocketCloseEvent::new(code, reason))
    }

    pub fn new_websocket_connect() -> Self {
        ASGIMessage::WebsocketConnect(WebsocketConnectEvent::new())
    }

    pub fn new_websocket_disconnect(code: usize) -> Self {
        ASGIMessage::WebsocketDisconnect(WebsocketDisconnectEvent::new(code))
    }

    pub fn new_websocket_receive(bytes: Option<Vec<u8>>, text: Option<String>) -> Self {
        ASGIMessage::WebsocketReceive(WebsocketReceiveEvent::new(bytes, text))
    }

    pub fn new_websocket_send(bytes: Option<Vec<u8>>, text: Option<String>) -> Self {
        ASGIMessage::WebsocketSend(WebsocketSendEvent::new(bytes, text))
    }
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