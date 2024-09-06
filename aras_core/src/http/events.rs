use crate::{asgispec::ASGIScope, server::ConnectionInfo};

use hyper::Request;

#[derive(Debug)]
pub struct HTTPScope {
    pub type_: String,
    pub asgi: ASGIScope,
    pub http_version: String,
    pub method: String,
    pub scheme: String,
    pub path: String,
    pub raw_path: Vec<u8>,
    pub query_string: Vec<u8>,
    pub root_path: String,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub client: Option<(String, u16)>,
    pub server: Option<(String, u16)>,
    // State not supported for now
}

impl HTTPScope {
    pub fn set_conn_info(&mut self, info: &ConnectionInfo) {
        self.client = Some((info.client_ip.to_owned(), info.client_port));
        self.server = Some((info.server_ip.to_owned(), info.server_port));
    }
}

impl From<&Request<hyper::body::Incoming>> for HTTPScope {
    fn from(value: &Request<hyper::body::Incoming>) -> Self {
        Self {
            type_: String::from("http"),
            asgi: ASGIScope::new(),
            http_version: format!("{:?}", value.version()),
            method: value.method().as_str().to_owned(),
            scheme: String::from("http"),
            path: value.uri().path().to_owned(),
            raw_path: value.uri().to_string().as_bytes().to_vec(),
            query_string: value.uri().query().unwrap_or("").as_bytes().to_vec(),
            root_path: String::from(""), // Optional, default for now
            headers: value
                .headers()
                .into_iter()
                .map(
                    |(name, value)| {
                        (name.as_str().as_bytes().to_vec(), value.as_bytes().to_vec())
                    }
                )
                .collect(),
            client: None,
            server: None,
        }
    }
}

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub struct HTTPDisconnectEvent {
    pub type_: String,
}

impl HTTPDisconnectEvent {
    pub fn new() -> Self {
        Self { type_: "http.disconnect".into() }
    }
}