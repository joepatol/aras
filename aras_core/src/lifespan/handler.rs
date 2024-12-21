use derive_more::Constructor;
use futures::TryFutureExt;
use log::{error, info, warn};
use tokio::task::JoinHandle;

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope, State};
use crate::error::{Error, Result};

use super::LifespanScope;

#[derive(Constructor)]
pub struct LifespanHandler<S: State + 'static, T: ASGICallable<S> + 'static> {
    application: Application<S, T>,
}

impl<S, T> LifespanHandler<S, T>
where
    S: State,
    T: ASGICallable<S>,
{
    pub async fn startup(self, state: S) -> Result<StartedLifespanHandler<S, T>> {
        info!("Application starting");

        let app_clone = self.application.clone();
        let state_clone = state.clone();
        let mut running_app =
            tokio::task::spawn(async move { app_clone.call(Scope::Lifespan(LifespanScope::new(state_clone))).await });

        let result = tokio::select! {
            out = startup_loop(self.application.clone()) => out,
            _ = &mut running_app => Err(Error::custom("Application stopped during startup")),
        };

        match result {
            Ok(use_lifespan) => {
                info!("Application startup complete");
                Ok(StartedLifespanHandler::new(self.application, running_app, use_lifespan))
            }
            Err(e) => {
                info!("Application startup failed; {e}");
                Err(e)
            }
        }
    }
}

#[derive(Constructor)]
pub struct StartedLifespanHandler<S: State + 'static, T: ASGICallable<S> + 'static> {
    application: Application<S, T>,
    app_task: JoinHandle<Result<()>>,
    enabled: bool,
}

impl<S, T> StartedLifespanHandler<S, T>
where
    S: State,
    T: ASGICallable<S>,
{
    pub async fn shutdown(self) -> Result<()> {
        info!("Application shutting down");
        if !self.enabled {
            return Ok(());
        };
        let result = tokio::try_join!(
            shutdown_loop(self.application.clone()),
            self.app_task.map_err(|e| Error::custom(format!("{e}")))
        );
        match result {
            Ok((_, Ok(_))) => {
                info!("Application shutdown complete");
                Ok(())
            }
            Ok((_, Err(e))) => {
                error!("Application shutdown failed; {e}");
                Err(e)
            }
            Err(e) => {
                error!("Application shutdown failed; {e}");
                Err(e)
            }
        }
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
        Some(ASGIMessage::StartupFailed(event)) => Err(Error::custom(event.message)),
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
            error!("Application shutdown failed");
            Err(Error::custom(event.message))
        }
        msg => Err(Error::invalid_asgi_message(Box::new(msg))),
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use tokio::task::JoinHandle;

    use super::{LifespanHandler, StartedLifespanHandler};
    use crate::application::{Application, ApplicationFactory};
    use crate::asgispec::{ASGICallable, ASGIMessage, ReceiveFn, Scope, SendFn, State};
    use crate::error::{Error, Result};

    #[derive(Clone, Debug)]
    struct MockState;
    impl State for MockState {}

    #[derive(Clone, Debug)]
    struct LifespanApp;

    impl ASGICallable<MockState> for LifespanApp {
        async fn call(&self, scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> super::Result<()> {
            if let Scope::Lifespan(_) = scope {
                loop {
                    match receive().await {
                        Ok(ASGIMessage::Startup(_)) => {
                            send(ASGIMessage::new_startup_complete()).await?;
                        }
                        Ok(ASGIMessage::Shutdown(_)) => return send(ASGIMessage::new_shutdown_complete()).await,
                        _ => return Err(Error::custom("Invalid message")),
                    }
                }
            };
            Err(Error::custom("Invalid scope"))
        }
    }

    #[derive(Clone, Debug)]
    struct LifespanUnsupportedApp;

    impl ASGICallable<MockState> for LifespanUnsupportedApp {
        async fn call(&self, scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> super::Result<()> {
            if let Scope::Lifespan(_) = scope {
                loop {
                    _ = receive().await?;
                    // Send an unrelated message, to mimick the protocol not being supported
                    send(ASGIMessage::new_http_disconnect()).await?;
                }
            };
            Err(Error::custom("Invalid scope"))
        }
    }

    #[derive(Clone, Debug)]
    struct ErrorApp;

    impl ASGICallable<MockState> for ErrorApp {
        async fn call(&self, _scope: Scope<MockState>, receive: ReceiveFn, _send: SendFn) -> super::Result<()> {
            _ = receive().await;
            Err(Error::custom("Test app raises error"))
        }
    }

    #[derive(Clone, Debug)]
    struct LifespanFailedApp;

    impl ASGICallable<MockState> for LifespanFailedApp {
        async fn call(&self, scope: Scope<MockState>, receive: ReceiveFn, send: SendFn) -> super::Result<()> {
            if let Scope::Lifespan(_) = scope {
                loop {
                    match receive().await {
                        Ok(ASGIMessage::Startup(_)) => {
                            send(ASGIMessage::new_startup_failed("test".to_string())).await?;
                        }
                        Ok(ASGIMessage::Shutdown(_)) => return send(ASGIMessage::new_shutdown_failed("test".to_string())).await,
                        _ => return Err(Error::custom("Invalid message")),
                    }
                }
            };
            Err(Error::custom("Invalid scope"))
        }
    }

    fn start_application<T: ASGICallable<MockState> + 'static>(application: Application<MockState, T>) -> JoinHandle<Result<()>> {
        tokio::task::spawn(async move {
            application
                .call(Scope::Lifespan(super::LifespanScope::new(MockState {})))
                .await
        })
    }

    #[tokio::test]
    async fn test_lifespan_startup() {
        let app = ApplicationFactory::new(LifespanApp {}, PhantomData).build();
        let lifespan_handler = LifespanHandler::new(app);
        let result = lifespan_handler.startup(MockState {}).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_shutdown_ok_if_disabled() {
        let app = ApplicationFactory::new(LifespanApp {}, PhantomData).build();
        let lifespan_handler = StartedLifespanHandler::new(app.clone(), start_application(app), false);
        let result = lifespan_handler.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_shutdown() {
        let app = ApplicationFactory::new(LifespanApp {}, PhantomData).build();
        let lifespan_handler = StartedLifespanHandler::new(app.clone(), start_application(app), true);
        let result = lifespan_handler.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lifespan_disabled_if_protocol_unsupported() {
        let app = ApplicationFactory::new(LifespanUnsupportedApp {}, PhantomData).build();
        let lifespan_handler = LifespanHandler::new(app);
        let lifespan_handler = lifespan_handler.startup(MockState {}).await.unwrap();
        assert!(lifespan_handler.enabled == false);
    }

    #[tokio::test]
    async fn test_error_on_startup() {
        let app = ApplicationFactory::new(ErrorApp {}, PhantomData).build();
        let lifespan_handler = LifespanHandler::new(app);
        let result = lifespan_handler.startup(MockState {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_startup_fails() {
        let app = ApplicationFactory::new(LifespanFailedApp {}, PhantomData).build();
        let lifespan_handler = LifespanHandler::new(app);
        let result = lifespan_handler.startup(MockState {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_shutdown_fails() {
        let app = ApplicationFactory::new(LifespanFailedApp {}, PhantomData).build();
        let lifespan_handler = StartedLifespanHandler::new(app.clone(), start_application(app), true);
        let result = lifespan_handler.shutdown().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_error_on_shutdown() {
        let app = ApplicationFactory::new(ErrorApp {}, PhantomData).build();
        let lifespan_handler = StartedLifespanHandler::new(app.clone(), start_application(app), true);
        let result = lifespan_handler.shutdown().await;
        println!("{:?}", result);
        assert!(result.is_err());
    }
}
