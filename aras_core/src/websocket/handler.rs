use std::sync::Arc;

use bytes::BytesMut;
use fastwebsockets::upgrade::UpgradeFut;
use fastwebsockets::{FragmentCollector, Frame, OpCode, Payload, upgrade};
use http::StatusCode;
use http_body_util::{BodyExt, Full};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use bytes::Bytes;
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
    mut req: Request,
    scope: Scope,
) -> Result<Response> {
    let app_clone = asgi_app.clone();
    let mut running_app = tokio::task::spawn(async move { app_clone.call(scope).await });

    let (accepted, app_response) = tokio::select! {
        _ = &mut running_app => Err(Error::custom("Application stopped during websocket handshake")),
        out = accept_websocket_connection(asgi_app.clone()) => out
    }?;
    
    if accepted {
        let (upgrade_response, fut) = upgrade::upgrade(&mut req)?;
        tokio::task::spawn(async move {
                if let Err(e) = tokio::select! {
                    out = running_app => {
                        match out {
                            Err(e) => error!("Application task failure: {e}"),
                            Ok(Err(e)) => error!("Error in application: {e}"),
                            _ => {},
                        };
                        Err(Error::custom("Application stopped during open websocket connection"))
                    },
                    out = run_accepted_websocket(asgi_app, fut) => out,
                } {
                    error!("Error while serving websocket; {e}")
                };
            });
        // The application might have send a body and additional headers
        // If connection is accepted, merge the application response with hyper/fastwebsocket
        // proposed response. This way we can make use of their upgrade functionality
        // while maintaining required control by the application
        return Ok(merge_responses(app_response, upgrade_response)?);
        };
    Ok(app_response)
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
            builder = builder.status(StatusCode::SWITCHING_PROTOCOLS);
            if msg.subprotocol.is_some() {
                builder = builder.header(hyper::header::SEC_WEBSOCKET_PROTOCOL, msg.subprotocol.unwrap())
            };
            for (bytes_key, bytes_value) in msg.headers.into_iter() {
                builder = builder.header(bytes_key, bytes_value);
            }
            Ok((true, builder.body(body)?))
        }
        Some(ASGIMessage::WebsocketClose(msg)) => {
            let body = Full::new(msg.reason.into()).map_err(|never| match never {}).boxed();
            builder = builder.status(StatusCode::FORBIDDEN);
            Ok((false, builder.body(body)?))
        }
        _ => Err(Error::invalid_asgi_message(Box::new(
            "Got invalid asgi message, expected 'websocket.accept', or 'websocket.close'",
        ))),
    }
}

enum WsIteration<'a> {
    ReceiveClient(std::result::Result<fastwebsockets::Frame<'a>, fastwebsockets::WebSocketError>),
    ReceiveApplication(Result<Option<ASGIMessage>>),
}

async fn run_accepted_websocket<T: ASGICallable>(asgi_app: Application<T>, upgraded_io: UpgradeFut) -> Result<()> {
    let ws = Arc::new(Mutex::new(FragmentCollector::new(upgraded_io.await?)));

    loop {
        let mut app_iter = asgi_app.clone();
        let ws_iter = ws.clone();
        let mut ws_locked = ws_iter.lock().await;

        let iteration: WsIteration<'_> = tokio::select! {
            out = ws_locked.read_frame() => WsIteration::ReceiveClient(out),
            out = app_iter.receive_from() => WsIteration::ReceiveApplication(out),
        };

        drop(ws_locked); // Drop the lock so it can be acquired for writing

        match iteration {
            WsIteration::ReceiveClient(frame) => {
                let app_clone = asgi_app.clone();
                if let false = server_next(frame?, app_clone).await? {break};
            },
            WsIteration::ReceiveApplication(msg) => {
                let ws_clone = ws.clone();
                if let false = application_next(msg?, ws_clone).await? {break};
            }
        };
    };

    asgi_app
        .send_to(ASGIMessage::WebsocketDisconnect(WebsocketDisconnectEvent::new(1005)))
        .await?;

    Ok(())
}

async fn application_next(
    msg: Option<ASGIMessage>,
    ws: Arc<Mutex<FragmentCollector<TokioIo<Upgraded>>>>,
) -> Result<bool> {
    match msg {
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
            Ok(true)
        }
        Some(ASGIMessage::WebsocketClose(msg)) => {
            let payload = Payload::Owned(msg.reason.into_bytes());
            let frame = Frame::new(true, OpCode::Close, None, payload);
            ws.lock().await.write_frame(frame).await?;
            Ok(false)
        }
        invalid => {
            error!("Got invalid ASGI message in websocket server loop! Received: {invalid:?}");
            let payload = Payload::Owned(String::from("Internal server error").into_bytes());
            let frame = Frame::new(true, OpCode::Close, None, payload);
            ws.lock().await.write_frame(frame).await?;
            Ok(false)
        }
    }
}

async fn server_next<T: ASGICallable>(
    frame: Frame<'_>,
    asgi_app: Application<T>,
) -> Result<bool> {
    let frame_bytes = frame.payload.to_vec();

    match frame.opcode {
        OpCode::Close => Ok(false),
        OpCode::Text => {
            // Text is guaranteed to be utf-8 by fastwebsockets
            let text = String::from_utf8(frame_bytes).unwrap();
            let msg = WebsocketReceiveEvent::new(None, Some(text));
            asgi_app.send_to(ASGIMessage::WebsocketReceive(msg)).await?;
            Ok(true)
        }
        OpCode::Binary => {
            let msg = WebsocketReceiveEvent::new(Some(frame_bytes), None);
            asgi_app.send_to(ASGIMessage::WebsocketReceive(msg)).await?;
            Ok(true)
        }
        _ => Ok(true),
    }
}

fn merge_responses(app_response: Response, upgrade_response: http::Response<http_body_util::Empty<Bytes>>) -> Result<Response> {
    let mut merged_response = http::Response::builder().status(upgrade_response.status());
    for (k, v) in upgrade_response.headers() {
        merged_response = merged_response.header(k, v);
    };
    for (k, v) in app_response.headers() {
        merged_response = merged_response.header(k, v);
    };
    let body = app_response.into_body();
    Ok(merged_response.body(body)?)
}
