use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use derive_more::derive::Constructor;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{service::Service, Request};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope};
use crate::error::{Error, Result};
use crate::server::ConnectionInfo;

use super::HTTPRequestEvent;

type Response = hyper::Response<BoxBody<Bytes, hyper::Error>>;

#[derive(Constructor)]
pub struct HTTP11Handler<T: ASGICallable> {
    asgi_app: Application<T>,
    conn_info: ConnectionInfo,
}

impl<T: ASGICallable + 'static> Service<Request<hyper::body::Incoming>> for HTTP11Handler<T> {
    type Error = Error;
    type Response = Response;
    type Future = Pin<Box<dyn Future<Output = std::result::Result<Self::Response, Self::Error>> + Send + Sync>>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        let mut scope = match Scope::from(&req) {
            Scope::HTTP(scope) => scope,
            Scope::Websocket(_scope) => panic!("Websocket not supported"),
            _ => unreachable!(), // Lifespan protocol is never initiated from a request
        };
        scope.set_conn_info(&self.conn_info);
        
        Box::pin(transport(self.asgi_app.clone(), req.into_body().boxed(), Scope::HTTP(scope)))
    }
}

async fn transport<T: ASGICallable>(asgi_app: Application<T>, body: BoxBody<Bytes, hyper::Error>, scope: Scope) -> Result<Response> {
    let (stream_out, response, app_out) = tokio::join!(
        stream_body(asgi_app.clone(), body),
        build_response_data(asgi_app.clone()),
        asgi_app.call(scope),
    );

    app_out?;
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

    let body = Full::new(cache.into())
        .map_err(|never| match never {})
        .boxed();
    let response = builder.body(body);
    Ok(response?)
}