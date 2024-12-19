use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use log::{error, warn};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope};
use crate::error::{Error, Result};
use crate::types::Response;

use super::HTTPRequestEvent;

pub async fn serve_http<T: ASGICallable + 'static>(
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
