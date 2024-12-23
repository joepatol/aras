use std::fmt::Debug;

use bytes::{Buf, Bytes};
use futures::StreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Body, Frame};
use hyper::Request;
use log::info;

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIReceiveEvent, ASGISendEvent, Scope, State};
use crate::error::{Error, Result};
use crate::types::Response;

pub async fn serve_http<B, S, T>(asgi_app: Application<S, T>, request: Request<B>, scope: Scope<S>) -> Result<Response>
where
    B: Body + Send + 'static,
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

async fn transport<B, S, T>(mut asgi_app: Application<S, T>, request: Request<B>) -> Result<Response>
where
    B: Body + Send + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    let result = tokio::try_join!(
        stream_request_body(asgi_app.clone(), request.into_body()),
        build_response(asgi_app.clone()),
    );

    asgi_app.send_to(ASGIReceiveEvent::new_http_disconnect()).await?;
    asgi_app.set_send_is_error();

    match result {
        Ok((_, response)) => Ok(response),
        Err(e) => Err(e),
    }
}

async fn stream_request_body<B, S, T>(asgi_app: Application<S, T>, body: B) -> Result<()>
where
    B: Body + Send + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    // This implementation will always send an additional ASGI message with an
    // empty body once the stream is finished.
    let mut stream = body.into_data_stream().boxed();
    let mut part;
    let mut more_body = true;

    loop {
        if more_body == false {
            break;
        }
        
        part = stream.next().await;

        let data = part.map_or_else(
            || {
                more_body = false;
                Ok(Vec::new())
            },
            |part_result| {
                part_result
                    .map(|mut data| data.copy_to_bytes(data.remaining()).to_vec())
                    .map_err(|e| Error::custom(format!("Failed to read body: {e:?}")))
            },
        )?;

        asgi_app
            .send_to(ASGIReceiveEvent::new_http_request(data, false))
            .await?;
    }
    Ok(())
}

async fn build_response<S, T>(mut asgi_app: Application<S, T>) -> Result<Response>
where
    S: State + 'static,
    T: ASGICallable<S> + 'static,
{
    let mut builder = hyper::Response::builder();

    let body = match asgi_app.receive_from().await? {
        Some(ASGISendEvent::HTTPResponseStart(msg)) => {
            builder = builder.status(msg.status);
            for (bytes_key, bytes_value) in msg.headers.into_iter() {
                builder = builder.header(bytes_key, bytes_value);
            }
            build_body_stream(asgi_app).await
        }
        msg => return Err(Error::unexpected_asgi_message(Box::new(msg))),
    };

    Ok(builder.body(body)?)
}

async fn build_body_stream<S, T>(mut asgi_app: Application<S, T>) -> BoxBody<Bytes, Error>
where
    S: State + 'static,
    T: ASGICallable<S> + 'static,
{
    let stream = async_stream::stream! {
        let mut more_data = true;
        loop {
            if more_data == false {
                break
            }
            match asgi_app.receive_from().await? {
                Some(ASGISendEvent::HTTPResponseBody(msg)) => {
                    info!("{}", &msg);
                    if msg.more_body == false {
                        more_data = false;
                    };
                    yield Ok(msg.body)
                }
                msg => yield Err(Error::unexpected_asgi_message(Box::new(msg))),
            }
        }
    };

    let byte_frame_stream = stream.map(|item| match item {
        Ok(data) => Ok(Frame::data(Bytes::from(data))),
        Err(e) => Err(e),
    });

    BoxBody::new(StreamBody::new(byte_frame_stream))
}

#[cfg(test)]
mod tests {
    use http::StatusCode;
    use http_body_util::BodyExt;
    use hyper::Request;

    use super::serve_http;
    use crate::application::ApplicationFactory;
    use crate::asgispec::{ASGICallable, ASGIReceiveEvent, ASGISendEvent, ReceiveFn, Scope, SendFn, State};
    use crate::error::{Error, Result};
    use crate::http::HTTPScope;
    use crate::types::Response;

    #[derive(Clone, Debug)]
    struct MockState;
    impl State for MockState {}

    #[derive(Clone, Debug)]
    struct EchoApp {
        extra_body: Option<String>,
    }

    impl EchoApp {
        pub fn new() -> Self {
            Self { extra_body: None }
        }

        pub fn new_with_body(body: &str) -> Self {
            Self {
                extra_body: Some(body.to_string()),
            }
        }
    }

    impl ASGICallable<MockState> for EchoApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> Result<()> {
            let mut body = Vec::new();
            loop {
                match (receive)().await {
                    Ok(ASGIReceiveEvent::HTTPRequest(msg)) => {
                        body.extend(msg.body.into_iter());
                        if msg.more_body {
                            continue;
                        } else {
                            let start_msg = ASGISendEvent::new_http_response_start(200, Vec::new());
                            (send)(start_msg).await?;
                            let more_body = self.extra_body.is_some();
                            let body_msg = ASGISendEvent::new_http_response_body(body, more_body);
                            (send)(body_msg).await?;
                            if let Some(b) = &self.extra_body {
                                let next_msg =
                                    ASGISendEvent::new_http_response_body(b.to_string().as_bytes().to_vec(), false);
                                (send)(next_msg).await?;
                            }
                            return Ok(());
                        };
                    }
                    Err(e) => return Err(e),
                    _ => return Err(Error::custom("Invalid message received from server")),
                }
            }
        }
    }

    #[derive(Clone, Debug)]
    struct ImmediateReturnApp;

    impl ASGICallable<MockState> for ImmediateReturnApp {
        async fn call(&self, _scope: Scope<MockState>, _receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct ErrorOnCallApp;

    impl ASGICallable<MockState> for ErrorOnCallApp {
        async fn call(&self, _scope: Scope<MockState>, _receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
            Err(Error::custom("Immediate error"))
        }
    }

    #[derive(Clone, Debug)]
    struct ErrorInLoopApp;

    impl ASGICallable<MockState> for ErrorInLoopApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
            _ = receive().await?;
            Err(Error::custom("Error in loop"))
        }
    }

    #[derive(Clone, Debug)]
    struct ErrorInDataStreamApp;

    impl ASGICallable<MockState> for ErrorInDataStreamApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> super::Result<()> {
            let headers = Vec::from([(
                String::from("a").as_bytes().to_vec(),
                String::from("header").as_bytes().to_vec(),
            )]);
            _ = receive().await?;
            let res_start_msg = ASGISendEvent::new_http_response_start(200, headers);
            send(res_start_msg).await?;
            let first_body = ASGISendEvent::new_http_response_body(String::from("hello").as_bytes().to_vec(), true);
            send(first_body).await?;
            // Instead of more body an invalid message is sent to mimick the error
            let invalid = ASGISendEvent::new_startup_complete();
            send(invalid).await?;
            Ok(())
        }
    }

    async fn response_to_body_string(response: Response) -> String {
        String::from_utf8(response.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
    }

    #[tokio::test]
    async fn test_echo_request_body() {
        let app = ApplicationFactory::new(EchoApp::new()).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

        let response = serve_http(app, request, scope).await.unwrap();
        assert!(response.status() == StatusCode::OK);
        let response_body = response_to_body_string(response).await;

        assert!(response_body == "hello world")
    }

    #[tokio::test]
    async fn test_body_sent_in_parts() {
        let app = ApplicationFactory::new(EchoApp::new_with_body(" more body")).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

        let response = serve_http(app, request, scope).await.unwrap();
        assert!(response.status() == StatusCode::OK);
        let response_body = response_to_body_string(response).await;
        println!("{}", response_body);
        assert!(response_body == "hello world more body")
    }

    #[tokio::test]
    async fn test_app_returns_when_called() {
        let app = ApplicationFactory::new(ImmediateReturnApp {}).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
        let response = serve_http(app, request, scope).await;

        assert!(response.is_err_and(|e| e.to_string() == "Unexpected ASGI message received. Some(AppReturned)"));
    }

    #[tokio::test]
    async fn test_app_fails_when_called() {
        let app = ApplicationFactory::new(ErrorOnCallApp {}).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
        let response = serve_http(app, request, scope).await;

        assert!(response.is_err_and(|e| e.to_string() == "Immediate error"));
    }

    #[tokio::test]
    async fn test_app_raises_error_while_communicating() {
        let app = ApplicationFactory::new(ErrorInLoopApp {}).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));
        let response = serve_http(app, request, scope).await;

        assert!(response.is_err_and(|e| e.to_string() == "Error in loop"));
    }

    #[tokio::test]
    async fn test_error_while_streaming_body() {
        let app = ApplicationFactory::new(ErrorInDataStreamApp {}).build();
        let request = Request::builder()
            .body("hello world".to_string())
            .expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

        let response = serve_http(app, request, scope).await.unwrap();
        let body = response.into_body().collect().await;

        assert!(body.is_err_and(|e| e.to_string() == "Unexpected ASGI message received. Some(StartupComplete(LifespanStartupComplete { type_: \"lifespan.startup.complete\" }))"));
    }
}
