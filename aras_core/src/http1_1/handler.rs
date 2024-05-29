use std::fmt::Write;

use derive_more::Constructor;
use httparse::{Header, Request, Status};
use http::StatusCode;

use crate::app_ready::ReadyApplication;
use crate::asgispec::{ASGIMessage, HTTPVersion, Scope};
use crate::connection_info::ConnectionInfo;
use crate::error::{Result, Error};
use crate::lines_codec::LinesCodec;
use crate::ASGIApplication;

use super::events::{HTTPDisconnectEvent, HTTPRequestEvent, HTTPScope};

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

    async fn build_and_send_response(&mut self) -> Result<()> {
        let mut started = false;
        let mut status = None;
        let mut headers = Vec::new();
        let mut body = Vec::new();

        loop {
            match self.application.receive_from().await {
                Some(ASGIMessage::HTTPResponseStart(msg)) => {
                    if started == true {
                        return Err(format!("Received 'http.response.start' event twice").into())
                    };
                    started = true;
                    status = Some(StatusCode::from_u16(msg.status)?);
                    headers.extend(msg.headers.into_iter());
                },
                Some(ASGIMessage::HTTPResponseBody(msg)) => {
                    if started == false {
                        return Err(format!("Received 'http.response.body' before 'http.response.start'").into())
                    };
                    body.extend(msg.body.into_iter()); 
                    if msg.more_body == false {
                        break
                    }
                }
                None => break,
                _ => return Err(format!("Received invalid http event").into())
            }
        };

        // Unwrap status as this is unreachable without setting it
        let response = create_response(status.unwrap(), headers, body)?;
        self.message_broker.send_message(response.as_bytes()).await?;

        Ok(())
    }

    pub async fn handle(&mut self) -> Result<()> {
        let buffer: &mut [u8; 2056] = &mut [0; 2056];
        let mut headers_buffer = [httparse::EMPTY_HEADER; 32];

        self.message_broker.read_message(buffer).await?;
        let (request, skip_bytes) = match parse_http_request(buffer, &mut headers_buffer).await {
            Ok((request, skip_bytes)) => (request, skip_bytes),
            Err(e) => {
                let response = response_400(&e.to_string())?;
                self.message_broker.send_message(response.as_bytes()).await?;
                return Ok(())
            }
        };
        let body_length = get_content_length(&request.headers);
        let scope = build_http_scope(request, &self.connection)?;

        let app_handle = self.application.call(scope).await;
        
        // Wait for the application or the server loop to finish
        // If the server loop does not finish first (stream body to app, receive response events from app and send response)
        // it is always an error.
        tokio::select! {
            res = async {
                self.stream_body(buffer, skip_bytes, body_length).await?;
                self.build_and_send_response().await?;
                Ok::<_, Error>(())
            } => {
                res?;
            }
            _ = app_handle => {
                self.message_broker.send_message(response_500()?.as_bytes()).await?;
            }

        }
        println!(
            "Close connection to client: {}:{}",
            self.connection.client_ip, self.connection.client_port
        );

        Ok(())
    }
}

fn create_response(status: StatusCode, headers: Vec<(Vec<u8>, Vec<u8>)>, body: Vec<u8>) -> Result<String> {
    let mut response = String::new();
    write!(response, "HTTP/1.1 {} {}\r\n", status.as_u16(), status.canonical_reason().unwrap_or(""))?;
    for (name, value) in headers {
        let name_str = std::str::from_utf8(&name)?;
        let value_str = std::str::from_utf8(&value)?;
        write!(response, "{}: {}\r\n", name_str, value_str)?;
    };
    write!(response, "Content-Length: {}\r\n", body.len())?;
    write!(response, "\r\n")?;
    response.push_str(std::str::from_utf8(&body)?);
    Ok(response)
}

fn response_400(msg: &str) -> Result<String> {
    create_response(StatusCode::from_u16(500)?, Vec::new(), msg.as_bytes().to_vec())
}

fn response_500() -> Result<String> {
    create_response(StatusCode::from_u16(500)?, Vec::new(), "Internal server error".as_bytes().to_vec())
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
    if req.version != Some(1) {
        return Err(format!("Unsupported HTTP version").into());
    };
    let method = match req.method {
        Some(m) => m,
        None => return Err(format!("No HTTP method provided").into())
    };
    let full_path = req.path.unwrap_or("/").to_owned();
    let (path, query_string) = match full_path.split_once("?") {
        Some((path, query_string)) => (path, query_string),
        None => (full_path.as_str(), "".into()),
    };

    // TODO: remove shortcuts here
    Ok(Scope::HTTP(HTTPScope::new(
        HTTPVersion::V1_1,
        method.to_owned(),
        "http".to_owned(),
        path.to_owned(),
        Some(full_path.as_bytes().to_vec()),
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
