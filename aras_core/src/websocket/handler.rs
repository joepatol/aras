use std::sync::Arc;

use derive_more::Constructor;
use futures_util::{StreamExt, TryStreamExt};
use http::{HeaderName, HeaderValue};
use log::debug;
use object_pool::Reusable;
use tokio::net::TcpStream;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use tungstenite::handshake::server::{Request, Response};

use crate::app_ready::ReadyApplication;
use crate::asgispec::{ASGIMessage, HTTPVersion, Scope};
use crate::connection_info::ConnectionInfo;
use crate::error::{Error, Result};
use crate::ASGIApplication;

use super::{WebsocketConnectEvent, WebsocketScope};

#[derive(Constructor)]
pub struct WebsocketHandler<'a, T: ASGIApplication + Send + Sync + 'static> {
    connection: &'a ConnectionInfo,
    application: &'a mut ReadyApplication<T>,
    buffer: &'a Reusable<'a, Vec<u8>>,
}

impl<'a, T: ASGIApplication + Send + Sync + 'static> WebsocketHandler<'a, T> {
    pub async fn handle(&mut self, scope: Scope, socket: TcpStream) -> Result<()> {
        Ok(())
        // debug!(
        //     "Websocket upgrade for: {}:{}",
        //     self.connection.client_ip, self.connection.client_port
        // );

        // let (app_out, server_out) = tokio::join!(self.application.call(scope), async {
        //     self.ws_loop(socket).await?;
        //     Ok::<_, Error>(())
        // });

        // Err(Error::Disconnect) // When the websocket loop exits, we have disconnected
    }

    async fn accept_new(&mut self, socket: TcpStream) -> Result<WebSocketStream<TcpStream>> {
        self.application
            .send_to(ASGIMessage::WebsocketConnect(WebsocketConnectEvent::new()))
            .await?;
        let accept_msg = match self.application.receive_from().await? {
            Some(ASGIMessage::WebsocketAccept(msg)) => msg,
            _ => return Err(Error::websocket_denied(socket)),
        };

        Ok(accept_hdr_async(socket, |_: &Request, mut response: Response| {
            let headers = response.headers_mut();
            if let Some(subprotocols) = accept_msg.subprotocol {
                headers.append("Sec-websocket-protocol", HeaderValue::from_str(&subprotocols).unwrap());
            };
            for (header_name, header_value) in accept_msg.headers.into_iter() {
                headers.append(
                    HeaderName::from_bytes(&header_name).unwrap(),
                    HeaderValue::from_bytes(&header_value).unwrap(),
                );
            }

            Ok(response)
        })
        .await?)
    }

    async fn ws_loop(&mut self, socket: TcpStream) -> Result<()> {
        let websocket = self.accept_new(socket).await?;

        let (mut write, mut read) = websocket.split();

        Ok(())
    }
}

fn get_subprotocols(headers: &[httparse::Header<'_>]) -> Vec<String> {
    headers
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Sec-WebSocket-Protocol"))
        .map(|h| std::str::from_utf8(h.value).ok().unwrap_or("").to_owned())
        .filter(|v| v != &String::from(""))
        .collect()
}

pub fn build_websocket_scope(req: httparse::Request<'_, '_>, connection_info: ConnectionInfo) -> Result<Scope> {
    if req.version != Some(1) {
        return Err(Error::not_supported("HTTP version"));
    };
    let full_path = req.path.unwrap_or("/").to_owned();
    let (path, query_string) = match full_path.split_once("?") {
        Some((path, query_string)) => (path, query_string),
        None => (full_path.as_str(), "".into()),
    };

    Ok(Scope::Websocket(WebsocketScope::new(
        HTTPVersion::V1_1,
        "ws".to_owned(),
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
        get_subprotocols(&req.headers),
    )))
}
