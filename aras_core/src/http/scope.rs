use crate::{asgispec::ASGIScope, server::ConnectionInfo};

use hyper::Request;

#[derive(Debug, Clone)]
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

impl std::fmt::Display for &HTTPScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "type: {}", self.type_)?;
        writeln!(f, "asgi: {:?}", self.asgi)?;
        writeln!(f, "http_version: {}", self.http_version)?;
        writeln!(f, "method: {}", self.method)?;
        writeln!(f, "scheme: {}", self.scheme)?;
        writeln!(f, "path: {}", self.path)?;
        writeln!(f, "raw_path: {}", String::from_utf8_lossy(&self.raw_path))?;
        writeln!(f, "query_string: {}", String::from_utf8_lossy(&self.query_string))?;
        writeln!(f, "root_path: {}", self.root_path)?;
        
        writeln!(f, "headers:")?;
        for (name, value) in &self.headers {
            writeln!(f, "  {}: {}", String::from_utf8_lossy(name), String::from_utf8_lossy(value))?;
        }
        
        if let Some((ip, port)) = &self.client {
            writeln!(f, "client: {}:{}", ip, port)?;
        } else {
            writeln!(f, "client: None")?;
        }
        
        if let Some((ip, port)) = &self.server {
            writeln!(f, "server: {}:{}", ip, port)?;
        } else {
            writeln!(f, "server: None")?;
        }

        Ok(())
    }
}