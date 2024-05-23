use derive_more::Constructor;
use httparse::{Request, Status, Header};
use serde::{Serialize, Deserialize};

use crate::asgispec::{HTTPVersion, ASGIScope, Scope};
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::connection_info::ConnectionInfo;
use crate::app_ready::ReadyApplication;
use crate::ASGIApplication;

#[derive(Serialize, Deserialize, Debug, Constructor)]
pub struct HTTPRequestEvent<'a> {
    #[serde(rename = "type")]
    type_: String,
    body: &'a [u8],
    more_body: bool,
}

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
    client: (String, u16),
    server: (String, u16),
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

#[derive(Constructor)]
pub struct HTTPHandler<T: ASGIApplication + Send + Sync + 'static> {
    message_broker: LinesCodec,
    connection: ConnectionInfo,
    application: ReadyApplication<T>,
}

impl<T: ASGIApplication + Send + Sync + 'static> HTTPHandler<T> {
    pub async fn handle(&mut self) -> Result<()> {
        let buffer: &mut [u8; 2056] = &mut [0; 2056];
        let mut headers_buffer = [httparse::EMPTY_HEADER; 32];
        
        self.message_broker.read_message(buffer).await?;
        let (request, mut skip_bytes) = parse_http_request(buffer, &mut headers_buffer).await?;
        let body_length = get_content_length(&request.headers);
        let scope = build_http_scope(request, &self.connection);

        if skip_bytes + body_length > buffer.len() {
            loop {
                self
                .application
                .send_to(buffer[skip_bytes..].to_vec())
                .await?;
                
                skip_bytes = 0;
                let bytes_read = self.message_broker.read_message(buffer).await?;
                if bytes_read == 0 {
                    break;
                }
            }
        } else {
            self
            .application
            .send_to(buffer[skip_bytes..skip_bytes + body_length].to_vec())
            .await?;
        };

        self.application.call(scope).await?;

        loop {
            match self.application.try_receive_from() {
                Ok(msg) => self.message_broker.send_message(msg.as_slice()).await?,
                Err(_) => break,
            }
        }

        Ok(())
    }
}

fn build_http_request_message(body: &[u8], more_body: bool) -> HTTPRequestEvent {
    HTTPRequestEvent::new("http.request".into(), body, more_body)
}

pub async fn parse_http_request<'a>(buffer: &'a [u8], headers_buf: &'a mut [Header<'a>]) -> Result<(Request<'a, 'a>, usize)> {
    let mut request = Request::new(headers_buf);
    match request.parse(buffer) {
        Ok(Status::Complete(bytes_read)) => {
            return Ok((request, bytes_read))
        }
        Ok(Status::Partial) => {
            return Err("Incomplete HTTP request".into());
        }
        Err(e) => {
            return Err(format!("Failed to read HTTP request, {}", e).into());
        }
    };
}

fn get_content_length(headers: &[Header<'_>]) -> usize {
    headers.iter()
        .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
        .and_then(|h| std::str::from_utf8(h.value).ok()?.parse::<usize>().ok())
        .unwrap_or(0)
}

fn build_http_scope(
    req: Request<'_, '_>, 
    connection_info: &ConnectionInfo,
) -> Scope {
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
            (connection_info.client_ip.clone(), connection_info.client_port), 
            (connection_info.server_ip.clone(), connection_info.server_port),
        )   
    )
}