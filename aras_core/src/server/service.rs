use derive_more::derive::Constructor;
use http_body_util::BodyExt;
use hyper::service::Service;

use crate::application::Application;
use crate::asgispec::{ASGICallable, Scope};
use crate::error::Error;
use crate::server::ConnectionInfo;
use crate::types::{Request, Response, ServiceFuture};
use crate::websocket::serve_websocket;
use crate::http::serve_http;


#[derive(Constructor, Clone)]
pub struct ASGIService<T: ASGICallable> {
    asgi_app: Application<T>,
    conn_info: ConnectionInfo,
}

impl<T: ASGICallable + 'static> Service<Request> for ASGIService<T> {
    type Error = Error;
    type Response = Response;
    type Future = ServiceFuture;

    fn call(&self, req: Request) -> Self::Future {
        match Scope::from(&req) {
            Scope::HTTP(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                Box::pin(serve_http(
                    self.asgi_app.clone(),
                    req.into_body().boxed(),
                    Scope::HTTP(scope),
                ))
            }
            Scope::Websocket(mut scope) => {
                scope.set_conn_info(&self.conn_info);
                Box::pin(serve_websocket(self.asgi_app.clone(), req, Scope::Websocket(scope)))
            }
            _ => unreachable!(), // Lifespan protocol is never initiated from a request
        }
    }
}