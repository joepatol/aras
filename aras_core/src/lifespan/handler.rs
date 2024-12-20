use log::{error, info, warn};

use crate::application::Application;
use crate::asgispec::{ASGICallable, ASGIMessage, Scope, State};
use crate::error::{Error, Result};

use super::LifespanScope;

pub struct LifespanHandler<S: State, T: ASGICallable<S>> {
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
        let result = tokio::select! {
            out = startup_loop(self.application.clone()) => out,
            _ = app_clone.call(Scope::Lifespan(LifespanScope::new(state))) => Err(Error::custom("Application stopped during startup")),
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
