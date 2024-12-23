use std::fmt::Debug;

use bytes::Bytes;
use futures::Stream;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response};
use log::{error, info};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIReceiveEvent, ASGISendEvent, Scope, State};
use crate::error::{Error, Result};
use crate::types::{SendSyncBody};

pub async fn serve_http<B, S, T>(asgi_app: Application<S, T>, request: Request<B>, scope: Scope<S>) -> Result<Response<impl Stream<Item = Result<Vec<u8>>>>>
where
    B: SendSyncBody + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    let app_clone = asgi_app.clone();
    match tokio::try_join!(app_clone.call(scope), transport(asgi_app, request)) {
        Ok((_, response)) => Ok(response),
        Err(e) => Err(e),
    }
}

async fn transport<B, S, T>(mut asgi_app: Application<S, T>, request: Request<B>) -> Result<Response<impl Stream<Item = Result<Vec<u8>>>>>
where
    B: SendSyncBody + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    let result = tokio::try_join!(
        send_full_body(asgi_app.clone(), request.into_body()),
        build_response(asgi_app.clone()),
    );

    asgi_app.send_to(ASGIReceiveEvent::new_http_disconnect()).await?;
    asgi_app.set_send_is_error();

    match result {
        Ok((_, response)) => Ok(response),
        Err(e) => Err(e),
    }
}

async fn send_full_body<B, S, T>(asgi_app: Application<S, T>, body: B) -> Result<()>
where
    B: SendSyncBody + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    let data = body.boxed().collect().await;
    if let Err(e) = data {
        error!("Error while collecting body: {e:?}");
        return Err(Error::custom("Failed to read body"));
    };
    let to_send = data.unwrap().to_bytes().to_vec();
    let msg = ASGIReceiveEvent::new_http_request(to_send, false);
    asgi_app.send_to(msg).await?;
    Ok(())
}

async fn build_response<S, T>(mut asgi_app: Application<S, T>) -> Result<Response<impl Stream<Item = Result<Vec<u8>>>>>
where
    S: State + 'static,
    T: ASGICallable<S> + 'static,
{
    let mut builder = hyper::Response::builder();

    let body = match asgi_app.receive_from().await? {
        Some(ASGISendEvent::HTTPResponseStart(msg)) => {
            info!("{}", &msg);
            builder = builder.status(msg.status);
            for (bytes_key, bytes_value) in msg.headers.into_iter() {
                builder = builder.header(bytes_key, bytes_value);
            }
            build_body_stream(asgi_app).await
        }
        msg => return Err(Error::unexpected_asgi_message(Box::new(msg))),
    };

    Ok(builder.body(body).unwrap())
}

async fn build_body_full<S, T>(mut asgi_app: Application<S, T>) -> Result<BoxBody<Bytes, hyper::Error>>
where
    S: State + 'static,
    T: ASGICallable<S> + 'static,
{
    let mut cache = Vec::new();

    loop {
        match asgi_app.receive_from().await? {
            Some(ASGISendEvent::HTTPResponseBody(msg)) => {
                info!("{}", &msg);
                cache.extend(msg.body.into_iter());
                if msg.more_body == false {
                    break;
                }
            }
            msg => return Err(Error::unexpected_asgi_message(Box::new(msg))),
        }
    }
    Ok(Full::new(Bytes::from(cache)).map_err(|never| match never {}).boxed())
}

async fn build_body_stream<S, T>(mut asgi_app: Application<S, T>) -> impl Stream<Item = Result<Vec<u8>>>
where
    S: State + 'static,
    T: ASGICallable<S> + 'static,
{
    let stream = async_stream::stream! {
        let mut c = true;
        loop {
            if c == false {
                return 
            }
            match asgi_app.receive_from().await? {
                Some(ASGISendEvent::HTTPResponseBody(msg)) => {
                    info!("{}", &msg);
                    if msg.more_body == false {
                        c = false;
                    };
                    yield Ok(msg.body)
                }
                _ => return,
            }
        }
    };
    stream
}

// #[cfg(test)]
// mod tests {
//     use http::StatusCode;
//     use http_body_util::BodyExt;
//     use hyper::Request;

//     use super::serve_http;
//     use crate::application::ApplicationFactory;
//     use crate::asgispec::{ASGICallable, ASGIReceiveEvent, ASGISendEvent, ReceiveFn, Scope, SendFn, State};
//     use crate::error::{Error, Result};
//     use crate::http::HTTPScope;
//     use crate::types::Response;

//     #[derive(Clone, Debug)]
//     struct MockState;
//     impl State for MockState {}

//     #[derive(Clone, Debug)]
//     struct EchoApp {
//         extra_body: Option<String>,
//     }

//     impl EchoApp {
//         pub fn new() -> Self {
//             Self { extra_body: None }
//         }

//         pub fn new_with_body(body: &str) -> Self {
//             Self {
//                 extra_body: Some(body.to_string()),
//             }
//         }
//     }

//     impl ASGICallable<MockState> for EchoApp {
//         async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> Result<()> {
//             let mut body = Vec::new();
//             loop {
//                 match (receive)().await {
//                     Ok(ASGIReceiveEvent::HTTPRequest(msg)) => {
//                         body.extend(msg.body.into_iter());
//                         if msg.more_body {
//                             continue;
//                         } else {
//                             let start_msg = ASGISendEvent::new_http_response_start(200, Vec::new());
//                             (send)(start_msg).await?;
//                             let more_body = self.extra_body.is_some();
//                             let body_msg = ASGISendEvent::new_http_response_body(body, more_body);
//                             (send)(body_msg).await?;
//                             if let Some(b) = &self.extra_body {
//                                 let next_msg =
//                                     ASGISendEvent::new_http_response_body(b.to_string().as_bytes().to_vec(), false);
//                                 (send)(next_msg).await?;
//                             }
//                             return Ok(());
//                         };
//                     }
//                     Err(e) => return Err(e),
//                     _ => return Err(Error::custom("Invalid message received from server")),
//                 }
//             }
//         }
//     }

//     #[derive(Clone, Debug)]
//     struct ImmediateReturnApp;

//     impl ASGICallable<MockState> for ImmediateReturnApp {
//         async fn call(&self, _scope: Scope<MockState>, _receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
//             Ok(())
//         }
//     }

//     #[derive(Clone, Debug)]
//     struct ErrorOnCallApp;

//     impl ASGICallable<MockState> for ErrorOnCallApp {
//         async fn call(&self, _scope: Scope<MockState>, _receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
//             Err(Error::custom("Immediate error"))
//         }
//     }

//     #[derive(Clone, Debug)]
//     struct ErrorInLoopApp;

//     impl ASGICallable<MockState> for ErrorInLoopApp {
//         async fn call(&self, _scope: Scope<MockState>, _receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
//             _ = (_receive)().await?;
//             Err(Error::custom("Error in loop"))
//         }
//     }

//     async fn response_to_body_string(response: Response) -> String {
//         String::from_utf8(response.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
//     }

//     #[tokio::test]
//     async fn test_echo_request_body() {
//         let app = ApplicationFactory::new(EchoApp::new()).build();
//         let request = Request::builder()
//             .body("hello world".to_string())
//             .expect("Failed to build request");
//         let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

//         let response = serve_http(app, request, scope).await.unwrap();
//         assert!(response.status() == StatusCode::OK);
//         let response_body = response_to_body_string(response).await;

//         assert!(response_body == "hello world")
//     }

//     #[tokio::test]
//     async fn test_body_sent_in_parts() {
//         let app = ApplicationFactory::new(EchoApp::new_with_body(" more body")).build();
//         let request = Request::builder()
//             .body("hello world".to_string())
//             .expect("Failed to build request");
//         let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

//         let response = serve_http(app, request, scope).await.unwrap();
//         assert!(response.status() == StatusCode::OK);
//         let response_body = response_to_body_string(response).await;
//         println!("{}", response_body);
//         assert!(response_body == "hello world more body")
//     }

//     #[tokio::test]
//     async fn test_app_returns_when_called() {
//         let app = ApplicationFactory::new(ImmediateReturnApp {}).build();
//         let request = Request::builder()
//             .body("hello world".to_string())
//             .expect("Failed to build request");
//         let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
//         let response = serve_http(app, request, scope).await;

//         assert!(response.is_err_and(
//             |e| e.to_string() == "Unexpected ASGI message received. Some(AppReturned)"
//         ));
//     }

//     #[tokio::test]
//     async fn test_app_fails_when_called() {
//         let app = ApplicationFactory::new(ErrorOnCallApp {}).build();
//         let request = Request::builder()
//             .body("hello world".to_string())
//             .expect("Failed to build request");
//         let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
//         let response = serve_http(app, request, scope).await;

//         assert!(response.is_err_and(|e| e.to_string() == "Immediate error"));
//     }

//     #[tokio::test]
//     async fn test_app_raises_error_while_communicating() {
//         let app = ApplicationFactory::new(ErrorInLoopApp {}).build();
//         let request = Request::builder()
//             .body("hello world".to_string())
//             .expect("Failed to build request");
//         let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
//         let response = serve_http(app, request, scope).await;

//         assert!(response.is_err_and(|e| e.to_string() == "Error in loop"));
//     }
// }
