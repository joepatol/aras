use std::fmt::Write;

use derive_more::Constructor;
use http::StatusCode;
use httparse::{Header, Request, Status};
use log::{debug, error, info};
use object_pool::Reusable;
use tokio::net::TcpStream;

use crate::app_ready::ReadyApplication;
use crate::asgispec::{ASGIMessage, HTTPVersion, Scope};
use crate::connection_info::ConnectionInfo;
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::websocket::{build_websocket_scope, WebsocketHandler};
use crate::{ASGIApplication, Error};

use super::events::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPScope};

#[derive(Constructor)]
pub struct HTTPHandler<'a, T: ASGIApplication + Send + Sync + 'static> {
    connection: &'a ConnectionInfo,
    application: &'a mut ReadyApplication<T>,
    buffer: Reusable<'a, Vec<u8>>,
}

impl<'a, T: ASGIApplication + Send + Sync + 'static> HTTPHandler<'a, T> {
    pub async fn handle(&mut self, keep_alive_s: usize, mut socket: LinesCodec) -> Result<()> {
        loop {
            let handle_or_disconnect = tokio::time::timeout(
                tokio::time::Duration::from_secs(keep_alive_s as u64),
                socket.read_message(&mut self.buffer),
            )
            .await;
            match handle_or_disconnect {
                Ok(bytes_read) => {
                    if bytes_read? == 0 {
                        debug!("Remote end closed connection");
                        break;
                    };
                    socket = match self.handle_request(keep_alive_s, socket).await {
                        Err(Error::Disconnect) => return Ok(()),
                        Err(e) => return Err(e),
                        Ok(s) => s,
                    };
                }
                Err(_) => break,
            }
        }
        debug!(
            "Dropping connection to {}:{}",
            self.connection.client_ip, self.connection.client_port
        );
        self.application
            .send_to(ASGIMessage::HTTPDisconnect(HTTPDisconnectEvent::new()))
            .await?;
        self.application.server_done();
        Ok(())
    }

    async fn stream_body(&mut self, mut skip_bytes: usize, body_length: usize, socket: &mut LinesCodec) -> Result<()> {
        let mut more_body: bool;
        let mut until_byte = if skip_bytes + body_length > self.buffer.len() {
            more_body = true;
            self.buffer.len()
        } else {
            more_body = false;
            skip_bytes + body_length
        };

        let body = self.buffer[skip_bytes..until_byte].to_vec();
        let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(body, more_body));

        self.application.send_to(msg).await?;

        skip_bytes = 0;
        loop {
            if more_body == false {
                break;
            };

            until_byte = socket.read_message(&mut self.buffer).await?;
            if until_byte <= self.buffer.len() {
                more_body = false;
            };

            let body = self.buffer[skip_bytes..until_byte].to_vec();
            let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(body, more_body));

            self.application.send_to(msg).await?;
        }

        Ok(())
    }

    async fn build_response_data(&mut self) -> Result<ResponseData> {
        let mut started = false;
        let mut status = None;
        let mut headers = Vec::new();
        let mut body = Vec::new();

        loop {
            match self.application.receive_from().await? {
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
        info!("Response sent; {}", response_data.status);
        socket.send_message(response_data.try_into()?).await?;
        Ok(())
    }

    async fn websocket_upgrade(&mut self, scope: Scope, socket: LinesCodec) -> Result<TcpStream> {
        let mut ws_handler = WebsocketHandler::new(&self.connection, self.application, &self.buffer);
        ws_handler.handle(scope, TcpStream::try_from(socket)?).await
    }

    async fn handle_request(&mut self, keep_alive_s: usize, mut socket: LinesCodec) -> Result<LinesCodec> {
        // TODO: Chunked request/response
        // TODO: encoding (gzip etc.)
        let mut headers_buffer = [httparse::EMPTY_HEADER; 32];

        let (request, bytes_read) = match parse_http_request(&self.buffer, &mut headers_buffer) {
            Ok((request, bytes_read)) => (request, bytes_read),
            Err(e) => {
                self.send_response(ResponseData::new_400(&e.to_string()), &mut socket)
                    .await?;
                return Ok(socket);
            }
        };

        if should_upgrade_to_websocket(&request.headers) == true {
            let scope = build_websocket_scope(request, self.connection.clone())?;
            return match self.websocket_upgrade(scope, socket).await {
                Ok(stream) => Ok(stream.into()),
                Err(Error::WebsocketNotAccepted {stream }) => {
                    let mut s = stream.into();
                    self.send_response(ResponseData::new_403(), &mut s).await?;
                    Ok(s)
                },
                Err(e) => Err(e)
            };
        };

        let body_length = get_content_length(&request.headers);
        let scope = build_http_scope(request, &self.connection)?;
        let (app_out, server_out) = tokio::join!(self.application.call(scope), async {
            self.stream_body(bytes_read, body_length, &mut socket).await?;
            let response = self.build_response_data().await?;
            Ok::<_, Error>(response)
        });

        let response_data = match (app_out, server_out) {
            (Ok(Ok(_)), Ok(response)) => response,
            (Ok(Err(e)), _) => {
                error!("Application error: {}", e);
                ResponseData::new_500()
            }
            (_, Err(e)) => {
                error!("Server error: {}", e);
                ResponseData::new_500()
            }
            (Err(e), Ok(_)) => {
                error!("Application error: {}", e);
                ResponseData::new_500()
            }
        };

        self.send_response(
            response_data
                .add_header("Connection", "Keep-Alive")
                .add_header("Keep-Alive", &format!("timeout={}", keep_alive_s)),
            &mut socket,
        )
        .await?;
        Ok(socket)
    }
}

#[derive(Constructor)]
struct ResponseData {
    status: StatusCode,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    body: Vec<u8>,
}

impl ResponseData {
    pub fn add_header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.as_bytes().to_vec(), value.as_bytes().to_vec()));
        self
    }

    fn new_403() -> Self {
        Self::new(StatusCode::from_u16(403).unwrap(), Vec::new(), Vec::new())
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

fn should_upgrade_to_websocket(headers: &[Header<'_>]) -> bool {
    headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case("Upgrade") && h.name.eq_ignore_ascii_case("websocket"))
}

fn get_content_length(headers: &[Header<'_>]) -> usize {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
        .and_then(|h| std::str::from_utf8(h.value).ok()?.parse::<usize>().ok())
        .unwrap_or(0)
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
