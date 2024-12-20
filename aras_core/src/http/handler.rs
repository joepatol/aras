use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use log::error;

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope, State};
use crate::error::{Error, Result};
use crate::types::{Request, Response};

pub async fn serve_http<S: State + 'static, T: ASGICallable<S> + 'static>(
    asgi_app: Application<S, T>,
    request: Request,
    scope: Scope<S>,
) -> Result<Response> {
    let app_clone = asgi_app.clone();
    let (app_result, server_result) = tokio::join!(app_clone.call(scope), transport(asgi_app, request));

    if let Err(e) = app_result {
        error!("Application error during http connection; {e}");
    };

    Ok(server_result?)
}

async fn transport<S: State + 'static, T: ASGICallable<S> + 'static>(mut asgi_app: Application<S, T>, request: Request) -> Result<Response> {
    let (stream_out, response) = tokio::join!(
        stream_body(asgi_app.clone(), request.into_body()),
        build_response_data(asgi_app.clone()),
    );

    asgi_app
        .send_to(ASGIMessage::new_http_disconnect())
        .await?;
    asgi_app.set_send_is_error();

    stream_out?;
    response
}

async fn stream_body<S: State + 'static, T: ASGICallable<S> + 'static>(asgi_app: Application<S, T>, body: Incoming) -> Result<()> {
    let data = body.boxed().collect().await;
    if let Err(e) = data {
        error!("Error while collecting body: {e}");
        return Err(Error::custom("Failed to read body"));
    };
    let to_send = data.unwrap().to_bytes().to_vec();
    let msg = ASGIMessage::new_http_request(to_send, false);
    asgi_app.send_to(msg).await?;
    Ok(())
}

async fn build_response_data<S: State + 'static, T: ASGICallable<S> + 'static>(mut asgi_app: Application<S, T>) -> Result<Response> {
    let mut started = false;
    let mut builder = hyper::Response::builder();
    let mut cache = Vec::new();

    loop {
        match asgi_app.receive_from().await? {
            Some(ASGIMessage::HTTPResponseStart(msg)) => {
                if started == true {
                    return Err(Error::state_change("http.response.start", vec!["http.response.body"]));
                };
                started = true;
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

    let body = Full::new(cache.into()).map_err(|never| match never {}).boxed();
    let response = builder.body(body);
    Ok(response?)
}
