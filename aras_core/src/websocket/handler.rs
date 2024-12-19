use std::sync::Arc;

use bytes::BytesMut;
use fastwebsockets::{FragmentCollector, Frame, OpCode, Payload, Role, WebSocket};
use http::StatusCode;
use http_body_util::{BodyExt, Full};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use log::error;
use tokio::sync::Mutex;

use crate::asgispec::Scope;
use crate::error::Result;
use crate::types::{Request, Response};
use crate::{application::Application, ASGICallable};
use crate::{ASGIMessage, Error};

use super::{WebsocketConnectEvent, WebsocketDisconnectEvent, WebsocketReceiveEvent};

pub async fn serve_websocket<T: ASGICallable + 'static>(
    asgi_app: Application<T>,
    req: Request,
    scope: Scope,
) -> Result<Response> {
    let app_clone = asgi_app.clone();
    let mut running_app = tokio::task::spawn(async move { app_clone.call(scope).await });

    let (accepted, response) = tokio::select! {
        _ = &mut running_app => Err(Error::custom("Application stopped during websocket handshake")),
        out = accept_websocket_connection(asgi_app.clone()) => out
    }?;

    if accepted {
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(u) => {
                    if let Err(e) = tokio::select! {
                        _ = running_app => Err(Error::custom("Application stopped during open websocket connection")),
                        out = run_accepted_websocket(asgi_app, u) => out,
                    } {
                        error!("Error while serving websocket; {e}")
                    };
                }
                Err(e) => {
                    error!("Websocket upgrade failed; {e}")
                }
            };
        });
    }
    Ok(response)
}

async fn accept_websocket_connection<T: ASGICallable>(mut asgi_app: Application<T>) -> Result<(bool, Response)> {
    let mut builder = hyper::Response::builder();
    asgi_app
        .send_to(ASGIMessage::WebsocketConnect(WebsocketConnectEvent::new()))
        .await?;

    match asgi_app.receive_from().await? {
        Some(ASGIMessage::WebsocketAccept(msg)) => {
            let body = Full::new(Vec::<u8>::new().into())
                .map_err(|never| match never {})
                .boxed();
            builder = builder.status(StatusCode::SWITCHING_PROTOCOLS).header(
                hyper::header::SEC_WEBSOCKET_PROTOCOL,
                msg.subprotocol.unwrap_or_default(),
            );
            for (bytes_key, bytes_value) in msg.headers.into_iter() {
                builder = builder.header(bytes_key, bytes_value);
            }
            Ok((true, builder.body(body)?))
        }
        Some(ASGIMessage::WebsocketClose(msg)) => {
            let body = Full::new(msg.reason.into()).map_err(|never| match never {}).boxed();
            builder = builder.status(StatusCode::FORBIDDEN);
            Ok((true, builder.body(body)?))
        }
        _ => Err(Error::invalid_asgi_message(Box::new(
            "Got invalid asgi message, expected 'websocket.accept', or 'websocket.close'",
        ))),
    }
}

async fn run_accepted_websocket<T: ASGICallable>(asgi_app: Application<T>, upgraded_io: Upgraded) -> Result<()> {
    let upgraded = TokioIo::new(upgraded_io);
    let mut ws = WebSocket::after_handshake(upgraded, Role::Server);

    ws.set_writev(true);
    ws.set_auto_close(true);
    ws.set_auto_pong(true);

    let ws = Arc::new(Mutex::new(FragmentCollector::new(ws)));

    let (server_loop, app_loop) = tokio::join!(
        server_loop(asgi_app.clone(), ws.clone()),
        application_loop(asgi_app.clone(), ws.clone()),
    );

    server_loop?;
    app_loop?;

    Ok(())
}

async fn application_loop<T: ASGICallable>(
    mut asgi_app: Application<T>,
    ws: Arc<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
) -> Result<()> {
    loop {
        match asgi_app.receive_from().await? {
            Some(ASGIMessage::WebsocketSend(msg)) => {
                if let Some(data) = msg.text {
                    let payload = Payload::Owned(data.into_bytes());
                    let frame = Frame::new(true, OpCode::Text, None, payload);
                    ws.lock().await.write_frame(frame).await?;
                }
                if let Some(data) = msg.bytes {
                    let payload = Payload::Bytes(BytesMut::from(&data[..]));
                    let frame = Frame::new(true, OpCode::Binary, None, payload);
                    ws.lock().await.write_frame(frame).await?;
                }
            }
            Some(ASGIMessage::WebsocketClose(msg)) => {
                let payload = Payload::Owned(msg.reason.into_bytes());
                let frame = Frame::new(true, OpCode::Close, None, payload);
                ws.lock().await.write_frame(frame).await?;
                break;
            }
            invalid => {
                error!("Got invalid ASGI message in websocket server loop! Received: {invalid:?}");
                let payload = Payload::Owned(String::from("Internal server error").into_bytes());
                let frame = Frame::new(true, OpCode::Close, None, payload);
                ws.lock().await.write_frame(frame).await?;
                break;
            }
        }
    }
    Ok(())
}

async fn server_loop<T: ASGICallable>(
    asgi_app: Application<T>,
    ws: Arc<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
) -> Result<()> {
    loop {
        let frame = ws.lock().await.read_frame().await?;
        let frame_bytes = frame.payload.to_vec();

        match frame.opcode {
            OpCode::Close => break,
            OpCode::Text => {
                // TODO: remove unwrap() here
                let text = String::from_utf8(frame_bytes).unwrap();
                let msg = WebsocketReceiveEvent::new(None, Some(text));
                if let Err(e) = asgi_app.send_to(ASGIMessage::WebsocketReceive(msg)).await {
                    error!("{e}");
                    break;
                };
            }
            OpCode::Binary => {
                let msg = WebsocketReceiveEvent::new(Some(frame_bytes), None);
                if let Err(e) = asgi_app.send_to(ASGIMessage::WebsocketReceive(msg)).await {
                    error!("{e}");
                    break;
                };
            }
            _ => continue,
        };
    }

    asgi_app
        .send_to(ASGIMessage::WebsocketDisconnect(WebsocketDisconnectEvent::new(1000)))
        .await?;
    Ok(())
}
