use derive_more::Constructor;
use httparse::{Header, Request, Status};
use serde::{Deserialize, Serialize};

use crate::app_ready::ReadyApplication;
use crate::asgispec::{ASGIMessage, ASGIScope, HTTPVersion, Scope};
use crate::connection_info::ConnectionInfo;
use crate::error::{Result, Error};
use crate::lines_codec::LinesCodec;
use crate::ASGIApplication;

#[derive(Serialize, Deserialize, Debug)]
pub struct HTTPRequestEvent {
    #[serde(rename = "type")]
    type_: String,
    body: Vec<u8>,
    more_body: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HTTPResponseStartEvent {
    #[serde(rename = "type")]
    type_: String,
    status: u16,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    trailers: bool,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct HTTPResonseBodyEvent {
    #[serde(rename = "type")]
    type_: String,
    body: Vec<u8>,
    more_body: bool,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct HTTPDisconnectEvent {
    #[serde(rename = "type")]
    type_: String,
}

impl HTTPDisconnectEvent {
    pub fn new() -> Self {
        Self { type_: "http.disconnect".into() }
    }
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
    async fn stream_body(&mut self, buffer: &mut [u8], mut skip_bytes: usize, body_length: usize) -> Result<()> {
        let mut more_body: bool;
        let mut until_byte = if skip_bytes + body_length > buffer.len() {
            more_body = true;
            buffer.len()
        } else {
            more_body = false;
            skip_bytes + body_length
        };

        let body = buffer[skip_bytes..until_byte].to_vec();
        let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(body, more_body));

        self.application.send_to(msg).await?;

        skip_bytes = 0;
        loop {
            if more_body == false {
                break;
            };

            until_byte = self.message_broker.read_message(buffer).await?;
            if until_byte < buffer.len() {
                more_body = false;
            };

            let body = buffer[skip_bytes..until_byte].to_vec();
            let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(body, more_body));

            self.application.send_to(msg).await?;
        };

        self.application.send_to(ASGIMessage::HTTPDisconnect(HTTPDisconnectEvent::new())).await?;
        self.application.server_done();

        Ok(())
    }

    // pub async fn build_response(&self) -> Result<()> {

    // }

    pub async fn handle(&mut self) -> Result<()> {
        let buffer: &mut [u8; 2056] = &mut [0; 2056];
        let mut headers_buffer = [httparse::EMPTY_HEADER; 32];

        self.message_broker.read_message(buffer).await?;
        let (request, skip_bytes) = parse_http_request(buffer, &mut headers_buffer).await?;
        let body_length = get_content_length(&request.headers);
        let scope = build_http_scope(request, &self.connection)?;

        let app_handle = self.application.call(scope).await;
        
        // Wait for the application or the server loop to finish
        // If the server loop does not finish first (stream body to app, receive response events from app and send response)
        // it is always an error.
        tokio::select! {
            res = async {
                self.stream_body(buffer, skip_bytes, body_length).await?;

                loop {
                    match self.application.receive_from().await {
                        Some(msg) => {
                            println!("Received: {:?}", &msg);
                            match msg {
                                ASGIMessage::HTTPResponse(msg) => self.message_broker.send_message(msg.as_bytes()).await?,
                                _ => panic!("Invalid message received from app"),
                            };
                            break
                        }
                        None => continue,
                    }
                }
                Ok::<_, Error>(())
            } => {
                res?;
            }
            _ = app_handle => {
                self.message_broker.send_message("Internal server error".as_bytes()).await?;
            }

        }
        println!(
            "Close connection to client: {}:{}",
            self.connection.client_ip, self.connection.client_port
        );

        Ok(())
    }
}

async fn parse_http_request<'a>(
    buffer: &'a [u8],
    headers_buf: &'a mut [Header<'a>],
) -> Result<(Request<'a, 'a>, usize)> {
    let mut request = Request::new(headers_buf);
    match request.parse(buffer) {
        Ok(Status::Complete(bytes_read)) => return Ok((request, bytes_read)),
        Ok(Status::Partial) => {
            return Err("Incomplete HTTP request".into());
        }
        Err(e) => {
            return Err(format!("Failed to read HTTP request, {}", e).into());
        }
    };
}

fn get_content_length(headers: &[Header<'_>]) -> usize {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
        .and_then(|h| std::str::from_utf8(h.value).ok()?.parse::<usize>().ok())
        .unwrap_or(0)
}

fn build_http_scope(req: Request<'_, '_>, connection_info: &ConnectionInfo) -> Result<Scope> {
    Ok(Scope::HTTP(HTTPScope::new(
        HTTPVersion::V1_1,
        req.method.unwrap().to_owned(),
        "http".to_owned(),
        req.path.unwrap().to_owned(),
        None,
        "".as_bytes().to_vec(),
        "".to_owned(),
        req.headers
            .into_iter()
            .map(|header| (header.name.as_bytes().to_vec(), header.value.to_vec()))
            .collect(),
        (connection_info.client_ip.clone(), connection_info.client_port),
        (connection_info.server_ip.clone(), connection_info.server_port),
    )))
}
