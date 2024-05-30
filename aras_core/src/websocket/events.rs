use crate::asgispec::ASGIScope;

#[derive(Debug)]
pub struct WebsocketScope {
    type_: String,
    asgi: ASGIScope,
    http_version: String,
    scheme: String,
    path: String,
    raw_path: Vec<u8>,
    query_string: Vec<u8>,
    root_path: String,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    client: (String, u16),
    server: (String, u16),
    subprotocols: Vec<String>,
    // State not supported
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
        client: (String, u16),
        server: (String, u16),
        subprotocols: Vec<String>, 
    ) -> Self {
        Self {
            type_: "websocket".to_string(),
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

