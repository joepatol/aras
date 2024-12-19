use crate::{asgispec::ASGIScope, server::ConnectionInfo};

use hyper::Request;

#[derive(Debug, Clone)]
pub struct WebsocketScope {
    pub type_: String,
    pub asgi: ASGIScope,
    pub http_version: String,
    pub scheme: String,
    pub path: String,
    pub raw_path: Vec<u8>,
    pub query_string: Vec<u8>,
    pub root_path: String,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub client: Option<(String, u16)>,
    pub server: Option<(String, u16)>,
    pub subprotocols: Vec<String>,
    // State not supported for now
}

impl From<&Request<hyper::body::Incoming>> for WebsocketScope {
    fn from(value: &Request<hyper::body::Incoming>) -> Self {
        let subprotocols = 
            value
            .headers()
            .into_iter()
            .filter(|(k, _)| k.as_str().to_lowercase() == "sec-websocket-protocol")
            .map(|(_, v)| {
                // TODO: is default here desirable?
                let mut txt = String::from_utf8(v.as_bytes().to_vec()).unwrap_or("".to_string());
                txt.retain(|c| !c.is_whitespace());
                txt
            })
            .map(|s| s.split(",").map(|substr| substr.to_owned()).collect::<Vec<String>>())
            .flatten()
            .collect();
        
        Self {
            type_: String::from("websocket"),
            asgi: ASGIScope::new(),
            http_version: format!("{:?}", value.version()),
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
            subprotocols,
        }
    }
}

impl WebsocketScope {
    pub fn new(
        http_version: String,
        scheme: String,
        path: String,
        raw_path: Vec<u8>,
        query_string: Vec<u8>,
        root_path: String,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
        client: Option<(String, u16)>,
        server: Option<(String, u16)>,
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

    pub fn set_conn_info(&mut self, info: &ConnectionInfo) {
        self.client = Some((info.client_ip.to_owned(), info.client_port));
        self.server = Some((info.server_ip.to_owned(), info.server_port));
    }
}
