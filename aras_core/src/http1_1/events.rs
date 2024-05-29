use crate::asgispec::{ASGIScope, HTTPVersion};

pub struct HTTPRequestEvent {
    pub type_: String,
    pub body: Vec<u8>,
    pub more_body: bool,
}

impl HTTPRequestEvent {
    pub fn new(body: Vec<u8>, more_body: bool) -> Self {
        Self {
            type_: "http.request".into(),
            body,
            more_body,
        }
    }
}

pub struct HTTPResponseStartEvent {
    pub type_: String,
    pub status: u16,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub trailers: bool,
}

impl HTTPResponseStartEvent {
    pub fn new(status: u16, headers: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
        Self {
            type_: "http.response.start".into(),
            status,
            headers,
            trailers: false // Not supported for now
        }
    }
}

pub struct HTTPResonseBodyEvent {
    pub type_: String,
    pub body: Vec<u8>,
    pub more_body: bool,
}

impl HTTPResonseBodyEvent {
    pub fn new(body: Vec<u8>, more_body: bool) -> Self {
        Self {
            type_: "http.response.body".into(),
            body,
            more_body,
        }
    }
}

pub struct HTTPDisconnectEvent {
    pub type_: String,
}

impl HTTPDisconnectEvent {
    pub fn new() -> Self {
        Self { type_: "http.disconnect".into() }
    }
}

pub struct HTTPScope {
    pub type_: String,
    pub asgi: ASGIScope,
    pub http_version: HTTPVersion,
    pub method: String,
    pub scheme: String,
    pub path: String,
    pub raw_path: Vec<u8>,
    pub query_string: Vec<u8>,
    pub root_path: String,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub client: (String, u16),
    pub server: (String, u16),
    // State not supported for now
}

impl HTTPScope {
    pub fn new(
        http_version: HTTPVersion,
        method: String,
        scheme: String,
        path: String,
        raw_path: Vec<u8>,
        query_string: Vec<u8>,
        root_path: String,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
        client: (String, u16),
        server: (String, u16),
    ) -> Self {
        Self {
            type_: String::from("http"),
            asgi: ASGIScope::new(),
            http_version,
            method,
            scheme,
            path,
            raw_path,
            query_string,
            root_path,
            headers,
            client,
            server,
        }
    }
}
