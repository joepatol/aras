use std::net::SocketAddr;

use httparse::{Request, Status};
use serde::{Serialize, Deserialize};

use crate::asgispec::{HTTPVersion, ASGIScope, Scope};
use crate::error::Result;
use crate::lines_codec::LinesCodec;

#[derive(Serialize, Deserialize, Debug)]
pub struct HTTPScope {
    #[serde(rename = "type")]
    type_: String,
    asgi: ASGIScope,
    http_version: HTTPVersion,
    method: String,
    scheme: String,
    path: String,
    raw_path: Option<Vec<u8>>,
    query_string: Vec<u8>,
    root_path: String,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    client: (String, u64),
    server: (String, u64),
    // State not supported for now
}

impl HTTPScope {
    pub fn new(
        http_version: HTTPVersion,
        method: String,
        scheme: String,
        path: String,
        raw_path: Option<Vec<u8>>,
        query_string: Vec<u8>,
        root_path: String,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
        client: (String, u64),
        server: (String, u64),
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

pub async fn parse_http(codec: &mut LinesCodec, client: SocketAddr, server: SocketAddr) -> Result<(Scope, Vec<u8>)> {
    let buffer: &mut [u8; 2056] = &mut [0; 2056];
    codec.read_message(buffer).await?;

    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut request = Request::new(&mut headers);
    match request.parse(buffer) {
        Ok(Status::Complete(used_bytes)) => {
            let body_length = request.headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
                .and_then(|h| std::str::from_utf8(h.value).ok()?.parse::<usize>().ok())
                .unwrap_or(0);
            let body = &buffer[used_bytes..used_bytes + body_length];
            return Ok((build_http_scope(request, client, server), body.to_vec()))
        }
        Ok(Status::Partial) => {
            return Err("Incomplete request".into());
        }
        Err(e) => {
            return Err(format!("Failed to read request, {}", e).into());
        }
    };
}

fn build_http_scope(req: Request<'_, '_>, client: SocketAddr, server: SocketAddr) -> Scope {
    Scope::HTTP(
        HTTPScope::new(
            HTTPVersion::V1_1, 
            req.method.unwrap().to_owned(),
            "http".to_owned(),
            req.path.unwrap().to_owned(), 
            None, 
            "".as_bytes().to_vec(), 
            "".to_owned(), 
            req.headers.into_iter().map(|header| {
                (header.name.as_bytes().to_vec(), header.value.to_vec())
            }).collect(), 
            (client.ip().to_string(), client.port() as u64), 
            (server.ip().to_string(), server.port() as u64),
        )   
    )
}