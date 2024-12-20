use derive_more::derive::Constructor;
use hyper::service::Service;

use crate::application::ApplicationFactory;
use crate::asgispec::{ASGICallable, Scope, State};
use crate::error::Error;
use crate::server::ConnectionInfo;
use crate::types::{Request, Response, ServiceFuture};
use crate::websocket::{serve_websocket, WebsocketScope};
use crate::http::{serve_http, HTTPScope};


#[derive(Constructor, Clone)]
pub struct ASGIService<S: State, T: ASGICallable<S>> {   
    app_factory: ApplicationFactory<S, T>,
    conn_info: ConnectionInfo,
    state: S,
}

impl<S: State + 'static, T: ASGICallable<S> + 'static> Service<Request> for ASGIService<S, T> {
    type Error = Error;
    type Response = Response;
    type Future = ServiceFuture;

    fn call(&self, req: Request) -> Self::Future {
        let asgi_app = self.app_factory.build();
        if is_websocket_request(&req) {
            let mut scope = WebsocketScope::from_hyper_request(&req, self.state.clone());
            scope.set_conn_info(&self.conn_info);
            Box::pin(serve_websocket(asgi_app, req, Scope::Websocket(scope)))
        } else {
            let mut scope = HTTPScope::from_hyper_request(&req, self.state.clone());
            scope.set_conn_info(&self.conn_info);
            Box::pin(serve_http(asgi_app, req, Scope::HTTP(scope)))
        }
    }
}

fn is_websocket_request(value: &hyper::Request<hyper::body::Incoming>) -> bool {
    if let Some(header_value) = value.headers().get("upgrade") {
        if header_value == "websocket" {
           return true
        }
    };
    false
}