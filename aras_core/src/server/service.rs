use derive_more::derive::Constructor;
use http_body_util::{BodyExt, Full};
use hyper::service::Service;
use hyper::Request;
use hyper::body::Incoming;
use log::error;

use crate::application::ApplicationFactory;
use crate::asgispec::{ASGICallable, Scope, State};
use crate::error::{Error, Result};
use crate::http::{serve_http, HTTPScope};
use crate::server::ConnectionInfo;
use crate::types::{Response, ServiceFuture};
use crate::websocket::{serve_websocket, WebsocketScope};

#[derive(Constructor, Clone)]
pub struct ASGIService<S: State, T: ASGICallable<S>> {
    app_factory: ApplicationFactory<S, T>,
    conn_info: ConnectionInfo,
    state: S,
}

impl<S: State + 'static, T: ASGICallable<S> + 'static> Service<Request<Incoming>> for ASGIService<S, T> {
    type Error = Error;
    type Response = Response;
    type Future = ServiceFuture;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let asgi_app = self.app_factory.build();
        if is_websocket_request(&req) {
            let mut scope = WebsocketScope::from_hyper_request(&req, self.state.clone());
            scope.set_conn_info(&self.conn_info);
            Box::pin(finalize(Box::pin(serve_websocket(asgi_app, req, Scope::Websocket(scope)))))
        } else {
            let mut scope = HTTPScope::from_hyper_request(&req, self.state.clone());
            scope.set_conn_info(&self.conn_info);
            Box::pin(finalize(Box::pin(serve_http(asgi_app, req, Scope::HTTP(scope)))))
        }
    }
}

fn is_websocket_request(value: &Request<Incoming>) -> bool {
    if let Some(header_value) = value.headers().get("upgrade") {
        if header_value == "websocket" {
            return true;
        }
    };
    false
}

async fn finalize(result: ServiceFuture) -> Result<Response> {
    match result.await {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Error serving request: {error}");
            let body_text = "Internal Server Error";
            let body = Full::new(body_text.as_bytes().to_vec().into())
                .map_err(|never| match never {})
                .boxed();
            let response = hyper::Response::builder()
                .status(500)
                .header(hyper::header::CONTENT_LENGTH, body_text.len())
                .header(hyper::header::CONTENT_TYPE, "text/plain")
                .body(body);
            Ok(response?)
        }
    }
}
