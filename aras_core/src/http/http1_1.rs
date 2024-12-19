use bytes::Bytes;
use derive_more::derive::Constructor;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::service::Service;
use log::{error, info, warn};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope};
use crate::error::{Error, Result};
use crate::server::ConnectionInfo;
use crate::types::{Request, Response, ServiceFuture};
use crate::websocket::serve_websocket;

use super::HTTPRequestEvent;

#[derive(Constructor, Clone)]
pub struct HTTP11Handler<T: ASGICallable> {
    asgi_app: Application<T>,
    conn_info: ConnectionInfo,
}

impl<T: ASGICallable + 'static> Service<Request> for HTTP11Handler<T> {
    type Error = Error;
    type Response = Response;
    type Future = ServiceFuture;

    fn call(&self, req: Request) -> Self::Future {
        info!("{:?}", req.headers());
        match Scope::from(&req) {
            Scope::HTTP(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                info!("Serving HTTP");
                Box::pin(serve_http(
                    self.asgi_app.clone(),
                    req.into_body().boxed(),
                    Scope::HTTP(scope),
                ))
            }
            Scope::Websocket(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                info!("Serving websocket");
                Box::pin(serve_websocket(self.asgi_app.clone(), req, Scope::Websocket(scope)))
            }
            _ => unreachable!(), // Lifespan protocol is never initiated from a request
        }
    }
}

async fn serve_http<T: ASGICallable + 'static>(
    asgi_app: Application<T>,
    body: BoxBody<Bytes, hyper::Error>,
    scope: Scope,
) -> Result<Response> {
    let app_clone = asgi_app.clone();
    let running_app = tokio::task::spawn(async move { app_clone.call(scope).await });

    let response = tokio::select! {
        _ = running_app => Err(Error::custom("Application stopped during open http connection")),
        out = transport(asgi_app, body) => out,
    }.map_err(|e| {
        error!("Error serving HTTP. {e}");
        e
    })?;

    Ok(response)
}

async fn transport<T: ASGICallable>(
    asgi_app: Application<T>,
    body: BoxBody<Bytes, hyper::Error>,
) -> Result<Response> {
    let (stream_out, response) = tokio::join!(
        stream_body(asgi_app.clone(), body),
        build_response_data(asgi_app.clone()),
    );

    stream_out?;
    response
}

async fn stream_body<T: ASGICallable>(asgi_app: Application<T>, body: BoxBody<Bytes, hyper::Error>) -> Result<()> {
    let data = body.collect().await?.to_bytes();
    let msg = ASGIMessage::HTTPRequest(HTTPRequestEvent::new(data.to_vec(), false));
    asgi_app.send_to(msg).await?;
    Ok(())
}

async fn build_response_data<T: ASGICallable>(mut asgi_app: Application<T>) -> Result<Response> {
    let mut started = false;
    let mut builder = hyper::Response::builder();
    let mut cache = Vec::new();
    let mut trailers = false;

    loop {
        match asgi_app.receive_from().await? {
            Some(ASGIMessage::HTTPResponseStart(msg)) => {
                if started == true {
                    return Err(Error::state_change("http.response.start", vec!["http.response.body"]));
                };
                started = true;
                trailers = msg.trailers;
                builder = builder.status(msg.status);
                for (bytes_key, bytes_value) in msg.headers.into_iter() {
                    builder = builder.header(bytes_key, bytes_value);
                }
            }
            Some(ASGIMessage::HTTPResponseBody(msg)) => {
                if started == false {
                    return Err(Error::state_change("http.response.body", vec!["http.response.start"]));
                };
                cache.extend(msg.body.into_iter());
                if msg.more_body == false {
                    break;
                }
            }
            None => break,
            msg => return Err(Error::invalid_asgi_message(Box::new(msg))),
        }
    }

    if trailers == true {
        warn!("Expecting HTTP trailers, but not fully implemented yet! Trailers will be ignored.")
    }

    let body = Full::new(cache.into()).map_err(|never| match never {}).boxed();
    let response = builder.body(body);
    Ok(response?)
}
