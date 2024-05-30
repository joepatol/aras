use log::{info, error};

use crate::error::{Result, Error};
use crate::asgispec::{ASGIApplication, Scope, ASGIMessage};
use crate::app_ready::ReadyApplication;

use super::{LifespanScope, LifespanStartup, LifespanShutdown};

pub struct LifespanHandler<T: ASGIApplication + Send + Sync + 'static> {
    application: ReadyApplication<T>,
    in_use: bool,
}

impl<T: ASGIApplication + Send + Sync + 'static> LifespanHandler<T> {
    pub fn new(application: ReadyApplication<T>) -> Self {
        Self { application, in_use: true }
    }

    async fn startup_loop(&mut self) -> Result<()> {
        self.application.send_to(ASGIMessage::Startup(LifespanStartup::new())).await?;
        match self.application.receive_from().await? {
            Some(ASGIMessage::StartupComplete(_)) => Ok(()),
            Some(ASGIMessage::StartupFailed(event)) => {
                error!("{}", &event.message);
                Err("startup failed".into())
            },
            _ => {
                error!("Lifespan protocol appears unsupported");
                self.in_use = false;
                Ok(())
            }
        }
    }

    async fn shutdown_loop(&mut self) -> Result<()> {
        if self.in_use == true {
            self.application.send_to(ASGIMessage::Shutdown(LifespanShutdown::new())).await?;
            match self.application.receive_from().await? {
                Some(ASGIMessage::ShutdownComplete(_)) => {
                    info!("Application shutdown complete");
                    Ok(())
                },
                Some(ASGIMessage::ShutdownFailed(event)) => {
                    error!("Application shutdown failed; {}", &event.message);
                    Ok(())
                },
                msg => Err(Error::invalid_asgi_message(Box::new(msg))),
            }
        } else {
            Ok(())
        }
    }

    pub async fn handle_startup(&mut self) -> Result<()> {
        info!("Application starting");
        let app_handle = self.application.call(Scope::Lifespan(LifespanScope::new()));

        let res = tokio::select! {
            res = async {
                self.startup_loop().await
            } => {
                res
            }
            res = async {
                match app_handle.await {
                    Ok(Ok(_)) => Ok(()),
                    Err(_) => Ok(()),
                    _ => Err("fail".into())
                }
            } => {
                res
            }
        };

        match res {
            Ok(_) => {
                info!("Application startup complete");
                Ok(())
            },
            Err(e) => {
                info!("Application startup failed. {e}");
                Err(e)
            }
        }
    }

    pub async fn handle_shutdown(&mut self) -> Result<()> {
        self.shutdown_loop().await
    }
}