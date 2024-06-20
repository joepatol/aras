use std::fmt::Write;

use derive_more::Constructor;
use http::StatusCode;
use httparse::{Header, Request, Status};
use log::{debug, error, info};
use object_pool::Reusable;
use tokio::net::TcpStream;
use async_recursion::async_recursion;

use crate::app_ready::ReadyApplication;
use crate::asgispec::{ASGIMessage, HTTPVersion, Scope};
use crate::connection_info::ConnectionInfo;
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::websocket::{build_websocket_scope, WebsocketHandler};
use crate::{ASGIApplication, Error};

use super::events::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPScope};

type OwnedHeaders = Vec<(String, Vec<u8>)>;

#[derive(Constructor)]
pub struct HTTP11Handler<'a> {
    connection: &'a ConnectionInfo,
    keep_alive_seconds: usize,
}

impl<'a> HTTP11Handler<'a> {
    pub async fn connect(
        &self,
        mut socket: LinesCodec, 
        mut buffer: Reusable<'a, Vec<u8>>, 
        app: ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
        socket.read_message(&mut buffer).await?;
        self.handle_next(socket, buffer, app).await?;
        Ok(())
    }

    #[async_recursion]
    async fn handle_next(
        &self,
        mut socket: LinesCodec, 
        buffer: Reusable<'a, Vec<u8>>, 
        app: ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
        let mut headers_buf = [httparse::EMPTY_HEADER; 32];
        let (request, bytes_read) = match parse_http_request(&buffer, &mut headers_buf) {
            Ok((r, b)) => (r, b),
            Err(e) => return self.send_response(ResponseData::new_400(&e.to_string()), &mut socket).await,
        };
        let mut headers = Vec::new();
        request.headers.clone_into(&mut headers);
        let owned_headers: OwnedHeaders = headers.into_iter().map(|h| (h.name.to_owned(), h.value.to_vec())).collect();

        if should_upgrade_to_websocket(&owned_headers) == true {
            let scope = match build_websocket_scope(request, self.connection.clone()) {
                Ok(scope) => scope,
                Err(e) => return self.send_response(ResponseData::new_400(&e.to_string()), &mut socket).await
            };
            return self.websocket_upgrade(
                scope,
                owned_headers,
                socket, 
                buffer, 
                app,
            ).await
        } else {
            let scope = match build_http_scope(request, &self.connection) {
                Ok(scope) => scope,
                Err(e) => return self.send_response(ResponseData::new_400(&e.to_string()), &mut socket).await
            };
            self.handle_http_request(
                scope,
                owned_headers,
                bytes_read,
                socket,
                buffer,
                app
            ).await?
        };
        Ok(())
    }

    async fn handle_http_request(
        &self,
        scope: Scope,
        headers: OwnedHeaders,
        bytes_read: usize,
        mut socket: LinesCodec, 
        mut buffer: Reusable<'a, Vec<u8>>, 
        mut app: ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
        let body_length = get_content_length(&headers);
        let (app_out, server_out) = 
        tokio::join!(
            app.call(scope),
            self.cycle(
                bytes_read,
                body_length,
                &mut socket,
                &mut buffer,
                &mut app,
            )
        );

        let response_data = match (app_out, server_out) {
            (Ok(Ok(_)), Ok(response)) => response,
            (Ok(Err(e)), _) => {
                error!("Application error: {}", e);
                ResponseData::new_500()
            },
            (Err(e), Ok(_)) => {
                error!("Application error: {}", e);
                ResponseData::new_500()
            },
            (_, Err(e)) => {
                error!("Server error: {}", e);
                ResponseData::new_500()
            },
        };

        self.send_response(
            response_data
                .add_header("Connection", "Keep-Alive")
                .add_header("Keep-Alive", &format!("timeout={}", self.keep_alive_seconds)),
            &mut socket,
        )
        .await?;

        self.maybe_handle_more(socket, buffer, app).await
    }

    async fn maybe_handle_more(
        &self,
        mut socket: LinesCodec, 
        mut buffer: Reusable<'a, Vec<u8>>, 
        mut app: ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
        let handle_next_or_disconnect = tokio::time::timeout(
            tokio::time::Duration::from_secs(self.keep_alive_seconds as u64),
            socket.read_message(&mut buffer),
        )
        .await;

        match handle_next_or_disconnect {
            Ok(Ok(bytes_read)) => {
                if bytes_read == 0 {
                    debug!("Remote end closed connection; {:?}", self.connection);
                    app.send_to(ASGIMessage::HTTPDisconnect(HTTPDisconnectEvent::new())).await?;
                    app.server_done();
                } else {
                    debug!("Handling next connection for {:?}", self.connection);
                    self.handle_next(socket, buffer, app).await?;
                }
            },
            Ok(Err(e)) => {
                debug!("Error for connection {:?}. {}", self.connection, e);
                return Err(e.into())
            },
            Err(_) => {
                debug!("Dropping connection {:?}.", self.connection);
                app.send_to(ASGIMessage::HTTPDisconnect(HTTPDisconnectEvent::new())).await?;
                app.server_done();
            },
        };
        Ok(())
    }

    async fn cycle(
        &self, 
        skip_bytes: usize, 
        body_length: usize, 
        socket: &mut LinesCodec,
        buffer: &mut Reusable<'a, Vec<u8>>,
        app: &mut ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<ResponseData> {
        self.stream_body(skip_bytes, body_length, socket, buffer, app).await?;
        self.build_response_data(app).await
    }

    async fn stream_body(
        &self, 
        mut skip_bytes: usize, 
        body_length: usize, 
        socket: &mut LinesCodec,
        buffer: &mut Reusable<'a, Vec<u8>>,
        app: &ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
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

        app.send_to(msg).await?;

        skip_bytes = 0;
        loop {
            if more_body == false {
                break;
            };

            until_byte = socket.read_message(buffer).await?;
            if until_byte <= buffer.len() {
                more_body = false;
            };

            let body = buffer[skip_bytes..until_byte].to_vec();
            let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(body, more_body));

            app.send_to(msg).await?;
        }

        Ok(())
    }

    async fn build_response_data(
        &self,
        app: &mut ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<ResponseData> {
        let mut started = false;
        let mut status = None;
        let mut headers = Vec::new();
        let mut body = Vec::new();

        loop {
            match app.receive_from().await? {
                Some(ASGIMessage::HTTPResponseStart(msg)) => {
                    if started == true {
                        return Err(Error::state_change("http.response.start", vec!["http.response.body"]));
                    };
                    started = true;
                    status =
                        Some(StatusCode::from_u16(msg.status).map_err(|_| Error::invalid_status_code(msg.status))?);
                    headers.extend(msg.headers.into_iter());
                }
                Some(ASGIMessage::HTTPResponseBody(msg)) => {
                    if started == false {
                        return Err(Error::state_change("http.response.body", vec!["http.response.start"]));
                    };
                    body.extend(msg.body.into_iter());
                    if msg.more_body == false {
                        break;
                    }
                }
                None => break,
                msg => return Err(Error::invalid_asgi_message(Box::new(msg))),
            }
        }

        Ok(ResponseData::new(
            status.ok_or(Error::MissingStatusCode)?,
            headers,
            body,
        ))
    }

    async fn send_response(&self, response_data: ResponseData, socket: &mut LinesCodec) -> Result<()> {
        debug!("Response data: {}", response_data);
        info!("Response sent; {}", response_data.status);
        socket.send_message(response_data.try_into()?).await?;
        Ok(())
    }

    async fn websocket_upgrade(
        &self, 
        scope: Scope,
        _headers: OwnedHeaders,
        socket: LinesCodec,
        buffer: Reusable<'a, Vec<u8>>, 
        mut app: ReadyApplication<impl ASGIApplication + Send + Sync + 'static>
    ) -> Result<()> {
        
        let mut ws_handler = WebsocketHandler::new(&self.connection, &mut app, &buffer);
        ws_handler.handle(scope, TcpStream::try_from(socket)?).await
    }
}

#[derive(Constructor)]
struct ResponseData {
    status: StatusCode,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    body: Vec<u8>,
}

impl std::fmt::Display for ResponseData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut human_readable_headers = Vec::new();
        for header in self.headers.iter() {
            let name = String::from_utf8(header.0.clone()).unwrap();
            let value = String::from_utf8(header.1.clone()).unwrap();
            human_readable_headers.push((name, value));
        }

        write!(f, "status: {}\n", self.status)?;
        write!(f, "headers: {:?}\n", human_readable_headers)?;
        write!(f, "body: {:?}", String::from_utf8(self.body.clone()).unwrap())?;

        Ok(())
    }
}

impl ResponseData {
    pub fn add_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.as_bytes().to_vec(), value.as_bytes().to_vec()));
        self
    }

    fn new_500() -> Self {
        Self::new(
            StatusCode::from_u16(500).unwrap(),
            Vec::new(),
            "Internal server error".as_bytes().to_vec(),
        )
    }

    fn new_400(body: &str) -> Self {
        Self::new(StatusCode::from_u16(400).unwrap(), Vec::new(), body.as_bytes().to_vec())
    }
}

impl TryFrom<ResponseData> for String {
    type Error = Error;

    fn try_from(value: ResponseData) -> std::prelude::v1::Result<Self, Self::Error> {
        let mut response = String::new();
        let mut content_length_present = false;
        write!(
            response,
            "HTTP/1.1 {} {}\r\n",
            value.status.as_u16(),
            value.status.canonical_reason().unwrap_or("")
        )?;
        for (name, value) in value.headers {
            let name_str = std::str::from_utf8(&name)?;
            if name_str.eq_ignore_ascii_case("content-length") {
                content_length_present = true;
            }
            let value_str = std::str::from_utf8(&value)?;
            write!(response, "{}: {}\r\n", name_str, value_str)?;
        }
        
        if content_length_present == false {
            write!(response, "Content-Length: {}\r\n", value.body.len())?;
        }
        write!(response, "Connection: Keep-Alive\r\n")?;
        write!(response, "\r\n{}", String::from_utf8(value.body).unwrap())?;
        Ok(response)
    }
}

fn parse_http_request<'a>(buffer: &'a [u8], headers_buf: &'a mut [Header<'a>]) -> Result<(Request<'a, 'a>, usize)> {
    let mut request = Request::new(headers_buf);
    match request.parse(buffer) {
        Ok(Status::Complete(bytes_read)) => return Ok((request, bytes_read)),
        // TODO: if partial retry with bigger buffer?
        Ok(Status::Partial) => {
            return Err(Error::from("Incomplete http request"));
        }
        Err(e) => {
            return Err(Error::from(e));
        }
    };
}

fn get_content_length(headers: &OwnedHeaders) -> usize {
    headers
        .iter()
        .find(|h| h.0.eq_ignore_ascii_case("Content-Length"))
        .and_then(|h| std::str::from_utf8(&h.1).ok()?.parse::<usize>().ok())
        .unwrap_or(0)
}

fn should_upgrade_to_websocket(headers: &OwnedHeaders) -> bool {
    headers
        .iter()
        .any(|h| h.0.eq_ignore_ascii_case("Upgrade") && std::str::from_utf8(&h.1).unwrap_or("").eq_ignore_ascii_case("websocket"))
}

fn build_http_scope(req: Request<'_, '_>, connection_info: &ConnectionInfo) -> Result<Scope> {
    if req.version != Some(1) {
        return Err(Error::not_supported("HTTP version"));
    };
    let method = req.method.unwrap_or("GET");
    let full_path = req.path.unwrap_or("/").to_owned();
    let (path, query_string) = match full_path.split_once("?") {
        Some((path, query_string)) => (path, query_string),
        None => (full_path.as_str(), "".into()),
    };

    Ok(Scope::HTTP(HTTPScope::new(
        HTTPVersion::V1_1,
        method.to_owned(),
        "http".to_owned(),
        path.to_owned(),
        full_path.as_bytes().to_vec(),
        query_string.as_bytes().to_vec(),
        "".to_owned(), // Optional, just provide default for now
        req.headers
            .into_iter()
            .map(|header| (header.name.as_bytes().to_vec(), header.value.to_vec()))
            .collect(),
        (connection_info.client_ip.clone(), connection_info.client_port),
        (connection_info.server_ip.clone(), connection_info.server_port),
    )))
}
