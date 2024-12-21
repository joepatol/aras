use std::fmt::Debug;

use http_body_util::{BodyExt, Full};
use hyper::Request;
use log::error;

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope, State};
use crate::error::{Error, Result};
use crate::types::{Response, ArasBody};

pub async fn serve_http<B, S, T>(
    asgi_app: Application<S, T>,
    request: Request<B>,
    scope: Scope<S>,
) -> Result<Response> 
where
    B: ArasBody + 'static,
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
    B: ArasBody + 'static,
    S: State + 'static,
    T: ASGICallable<S> + 'static,
    <B as hyper::body::Body>::Error: Debug,
{
    let result = tokio::try_join!(
        stream_body(asgi_app.clone(), request.into_body()),
        build_response_data(asgi_app.clone()),
    );

    asgi_app
        .send_to(ASGIMessage::new_http_disconnect())
        .await?;
    asgi_app.set_send_is_error();

    match result {
        Ok((_, response)) => Ok(response),
        Err(e) => Err(e),
        
    }
}

async fn stream_body<B, S, T>(asgi_app: Application<S, T>, body: B) -> Result<()> 
where
    B: ArasBody + 'static,
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
            },
            Some(ASGIMessage::Error(e)) => return Err(Error::custom(e.to_string())),
            None => break,
            msg => return Err(Error::invalid_asgi_message(Box::new(msg))),
        }
    }

    let body = Full::new(cache.into()).map_err(|never| match never {}).boxed();
    let response = builder.body(body);
    Ok(response?)
}

#[cfg(test)]
mod tests {
    use http::StatusCode;
    use http_body_util::BodyExt;
    use hyper::Request;

    use super::serve_http;
    use crate::http::HTTPScope;
    use crate::application::ApplicationFactory;
    use crate::asgispec::{ASGICallable, ASGIMessage, ReceiveFn, Scope, SendFn, State};
    use crate::error::{Error, Result};
    use crate::types::Response;

    #[derive(Clone, Debug)]
    struct MockState;
    impl State for MockState {}

    #[derive(Clone, Debug)]
    struct EchoApp;

    impl ASGICallable<MockState> for EchoApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> Result<()> {
            let mut body = Vec::new();
            loop {
                match (receive)().await {
                    Ok(ASGIMessage::HTTPRequest(msg)) =>  {
                        body.extend(msg.body.into_iter());
                        if msg.more_body {
                            continue
                        } else {
                            let start_msg = ASGIMessage::new_http_response_start(200, Vec::new());
                            (send)(start_msg).await?;
                            let body_msg = ASGIMessage::new_http_response_body(body, false);
                            (send)(body_msg).await?;
                            return Ok(())
                        };
                    },
                    Err(e) => return Err(e),
                    _ => return Err(Error::custom("Invalid message received from server"))
                }
            }   
        }
    }

    #[derive(Clone, Debug)]
    struct EchoAdditionalApp;

    impl ASGICallable<MockState> for EchoAdditionalApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> Result<()> {
            let mut body = Vec::new();
            let additional_body = " more body".to_string().as_bytes().to_vec();
            loop {
                match (receive)().await {
                    Ok(ASGIMessage::HTTPRequest(msg)) =>  {
                        body.extend(msg.body.into_iter());
                        if msg.more_body {
                            continue
                        } else {
                            let start_msg = ASGIMessage::new_http_response_start(200, Vec::new());
                            (send)(start_msg).await?;
                            let body_msg = ASGIMessage::new_http_response_body(body, true);
                            (send)(body_msg).await?;
                            let additional_body = ASGIMessage::new_http_response_body(additional_body, false);
                            (send)(additional_body).await?;
                            return Ok(())
                        };
                    },
                    Err(e) => return Err(e),
                    _ => return Err(Error::custom("Invalid message received from server"))
                }
            }   
        }
    }

    async fn response_to_body_string(response: Response) -> String {
        String::from_utf8(response.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
    }

    #[tokio::test]
    async fn test_echo_request_body() {
        let app = ApplicationFactory::new(EchoApp {}).build();
        let request = Request::builder().body("hello world".to_string()).expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

        let response = serve_http(app, request, scope).await.unwrap();
        assert!(response.status() == StatusCode::OK);
        let response_body = response_to_body_string(response).await;

        assert!(response_body == "hello world")
    }

    #[tokio::test]
    async fn test_body_sent_in_parts() {
        let app = ApplicationFactory::new(EchoAdditionalApp {}).build();
        let request = Request::builder().body("hello world".to_string()).expect("Failed to build request");
        let scope = Scope::HTTP(HTTPScope::from_hyper_request(&request, MockState {}));

        let response = serve_http(app, request, scope).await.unwrap();
        assert!(response.status() == StatusCode::OK);
        let response_body = response_to_body_string(response).await;

        assert!(response_body == "hello world more body")
    }
}