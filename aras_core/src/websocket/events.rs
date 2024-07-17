use crate::asgispec::{ASGIScope, HTTPVersion};

#[derive(Debug)]
pub struct WebsocketScope {
    pub type_: String,
    pub asgi: ASGIScope,
    pub http_version: HTTPVersion,
    pub scheme: String,
    pub path: String,
    pub raw_path: Vec<u8>,
    pub query_string: Vec<u8>,
    pub root_path: String,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub client: (String, u16),
    pub server: (String, u16),
    pub subprotocols: Vec<String>,
    // State not supported for now
}

impl WebsocketScope {
    pub fn new(
        http_version: HTTPVersion,
        scheme: String,
        path: String,
        raw_path: Vec<u8>,
        query_string: Vec<u8>,
        root_path: String,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
        client: (String, u16),
        server: (String, u16),
        subprotocols: Vec<String>,
    ) -> Self {
        Self {
            type_: String::from("websocket"),
            asgi: ASGIScope::new(),
            http_version,
            scheme,
            path,
            raw_path,
            query_string,
            root_path,
            headers,
            client,
            server,
            subprotocols,
        }
    }
}

#[derive(Debug)]
pub struct WebsocketConnectEvent {
    pub type_: String,
}

impl WebsocketConnectEvent {
    pub fn new() -> Self {
        Self { type_: "websocket.connect".into() }
    }
}

#[derive(Debug)]
pub struct WebsocketAcceptEvent {
    pub type_: String,
    pub subprotocol: Option<String>,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
}

impl WebsocketAcceptEvent {
    pub fn new(
        subprotocol: Option<String>,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Self {
        Self { type_:  "websocket.accept".into(), subprotocol, headers }
    }
}

#[derive(Debug)]
pub struct WebsocketReceiveEvent {
    pub type_: String,
    pub bytes: Option<Vec<u8>>,
    pub text: Option<String>,
}

impl WebsocketReceiveEvent {
    pub fn new(
        bytes: Option<Vec<u8>>,
        text: Option<String>,
    ) -> Self {
        // TODO: at least one of bytes or text should be present
        Self { type_: "websocket.receive".into(), bytes, text }
    } 
}

#[derive(Debug)]
pub struct WebsocketSendEvent {
    pub type_: String,
    pub bytes: Option<Vec<u8>>,
    pub text: Option<String>,
}

impl WebsocketSendEvent {
    pub fn new(
        bytes: Option<Vec<u8>>,
        text: Option<String>,
    ) -> Self {
        // TODO: at least one of bytes or text should be present
        Self { type_: "websocket.send".into(), bytes, text }
    } 
}

#[derive(Debug)]
pub struct WebsocketDisconnectEvent {
    pub type_: String,
    pub code: usize,
}

impl WebsocketDisconnectEvent {
    pub fn new(code: usize) -> Self {
        Self { type_: "websocket.disconnect".into(), code }
    }
}

impl Default for WebsocketDisconnectEvent {
    fn default() -> Self {
        Self { type_: "websocket.disconnect".into(), code: 1005 }
    }
}

#[derive(Debug)]
pub struct WebsocketCloseEvent {
    pub type_: String,
    pub code: usize,
    pub reason: String,
}

impl WebsocketCloseEvent {
    pub fn new(code: Option<usize>, reason: String) -> Self {
        Self { type_: "websocket.close".into(), code: code.unwrap_or(1000), reason }
    }
}