use log::{error, info, warn};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope, State};
use crate::error::{Error, Result};

use super::LifespanScope;

pub struct LifespanHandler<S: State + 'static, T: ASGICallable<S> + 'static> {
    application: Application<S, T>,
    enabled: bool,
}

impl<S, T> LifespanHandler<S, T>
where
    S: State,
    T: ASGICallable<S>,
{
    pub fn new(application: Application<S, T>) -> Self {
        Self {
            application,
            enabled: true,
        }
    }

    pub async fn startup(&mut self, state: S) -> Result<()> {
        info!("Application starting");

        let app_clone = self.application.clone();
        let state_clone = state.clone();
        let running_app = tokio::task::spawn(async move { 
            app_clone.call(Scope::Lifespan(LifespanScope::new(state_clone))).await 
        });

        let result = tokio::select! {
            out = startup_loop(self.application.clone()) => out,
            _ = running_app => Err(Error::custom("Application stopped during startup")),
        };

        match result {
            Ok(use_lifespan) => {
                info!("Application startup complete");
                self.enabled = use_lifespan;
                Ok(())
            }
            Err(e) => {
                info!("Application startup failed; {e}");
                Err(e)
            }
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Application shutting down");
        if self.enabled == false {
            return Ok(());
        };
        shutdown_loop(self.application.clone()).await
    }
}

async fn startup_loop<S, T>(mut application: Application<S, T>) -> Result<bool>
where
    S: State,
    T: ASGICallable<S>,
{
    application.send_to(ASGIMessage::new_lifespan_startup()).await?;
    match application.receive_from().await? {
        Some(ASGIMessage::StartupComplete(_)) => Ok(true),
        Some(ASGIMessage::StartupFailed(event)) => {
            Err(Error::custom(event.message))
        }
        _ => {
            warn!("Lifespan protocol appears unsupported");
            Ok(false)
        }
    }
}

async fn shutdown_loop<S, T>(mut application: Application<S, T>) -> Result<()>
where
    S: State,
    T: ASGICallable<S>,
{
    application.send_to(ASGIMessage::new_lifespan_shutdown()).await?;
    match application.receive_from().await? {
        Some(ASGIMessage::ShutdownComplete(_)) => {
            info!("Application shutdown complete");
            Ok(())
        }
        Some(ASGIMessage::ShutdownFailed(event)) => {
            error!("Application shutdown failed; {}", &event.message);
            Ok(())
        }
        msg => Err(Error::invalid_asgi_message(Box::new(msg))),
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use crate::application::{Application, ApplicationFactory};
    use crate::asgispec::{ASGICallable, ASGIMessage, State, SendFn, ReceiveFn, Scope};
    use crate::lifespan::LifespanHandler;

    #[derive(Clone, Debug)]
    struct MockState;
    impl State for MockState {}

    #[derive(Clone, Debug)]
    struct TestASGIApp(bool);

    impl TestASGIApp {
        pub fn new() -> Self {
            Self(true)
        }

        pub fn new_unsupported() -> Self {
            Self(false)
        }
    }

    impl ASGICallable<MockState> for TestASGIApp {
        async fn call(&self, scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> super::Result<()>{
            if let Scope::Lifespan(_) = scope {
                loop {
                    if self.0 == false {
                        send(ASGIMessage::new_http_disconnect()).await?;
                    };

                    match receive().await {
                        Ok(ASGIMessage::Startup(_)) => {
                            send(ASGIMessage::new_startup_complete()).await?;
                        },
                        Ok(ASGIMessage::Shutdown(_)) => {
                            return send(ASGIMessage::new_shutdown_complete()).await
                        },
                        _ => return Err(super::Error::custom("Invalid message")),
                    }
                }
            };
            Err(super::Error::custom("Invalid scope"))
        }
    }

    fn create_application(protocol_supported: bool) -> Application<MockState, TestASGIApp> {
        if protocol_supported{
            ApplicationFactory::new(TestASGIApp::new(), PhantomData).build()
        } else {
            ApplicationFactory::new(TestASGIApp::new_unsupported(), PhantomData).build()
        }
    }

    #[tokio::test]
    async fn test_lifespan_startup() {
        let mut lifespan_handler = LifespanHandler::new(create_application(true));
        let result = lifespan_handler.startup(MockState{}).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_shutdown_ok_if_disabled() {
        let lifespan_handler = LifespanHandler { application: create_application(true), enabled: false };
        let result = lifespan_handler.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_shutdown() {
        let mut lifespan_handler = LifespanHandler::new(create_application(true));
        let _ = lifespan_handler.startup(MockState{}).await; 
        let result = lifespan_handler.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_disabled_if_protocol_unsupported() {
        let mut lifespan_handler = LifespanHandler::new(create_application(false));
        let startup_result = lifespan_handler.startup(MockState{}).await;
        assert!(startup_result.is_ok());
        assert!(lifespan_handler.enabled == false);
        let shutdown_result = lifespan_handler.shutdown().await;
        assert!(shutdown_result.is_ok());
    }
}