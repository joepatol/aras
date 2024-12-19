use derive_more::derive::Constructor;
use hyper::service::Service;

use crate::application::ApplicationFactory;
use crate::asgispec::{ASGICallable, Scope};
use crate::error::Error;
use crate::server::ConnectionInfo;
use crate::types::{Request, Response, ServiceFuture};
use crate::websocket::serve_websocket;
use crate::http::serve_http;


#[derive(Constructor, Clone)]
pub struct ASGIService<T: ASGICallable> {
    app_factory: ApplicationFactory<T>,
    conn_info: ConnectionInfo,
}

impl<T: ASGICallable + 'static> Service<Request> for ASGIService<T> {
    type Error = Error;
    type Response = Response;
    type Future = ServiceFuture;

    fn call(&self, req: Request) -> Self::Future {
        let asgi_app = self.app_factory.build();
        match Scope::from(&req) {
            Scope::HTTP(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                Box::pin(serve_http(asgi_app, req, Scope::HTTP(scope)))
            }
            Scope::Websocket(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                Box::pin(serve_websocket(asgi_app, req, Scope::Websocket(scope)))
            }
            _ => unreachable!(), // Lifespan protocol is never initiated from a request
        }
    }
}