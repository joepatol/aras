use std::process::abort;

use crate::error::Result;
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
                eprintln!("Application startup failed; {}", &event.message);
                abort(); // TODO graceful shutdown
            },
            _ => Err(format!("Received invalid lifespan event").into()),
        }
    }

    async fn shutdown_loop(&mut self) -> Result<()> {
        if self.in_use == true {
            self.application.send_to(ASGIMessage::Shutdown(LifespanShutdown::new())).await?;
            match self.application.receive_from().await? {
                Some(ASGIMessage::ShutdownComplete(_)) => Ok(()),
                Some(ASGIMessage::ShutdownFailed(event)) => {
                    eprintln!("Application shutdown failed; {}", &event.message);
                    Ok(())
                },
                _ => Err(format!("Received invalid lifespan event").into()),
            }
        } else {
            Ok(())
        }
    }

    pub async fn handle_startup(&mut self) -> Result<()> {
        let app_handle = self.application.call(Scope::Lifespan(LifespanScope::new())).await;
        tokio::select! {
            res = async {
                self.startup_loop().await
            } => {
                res
            }
            _ = app_handle => {
                self.in_use = false;
                Ok(())
            }
        }
    }

    pub async fn handle_shutdown(&mut self) -> Result<()> {
        self.shutdown_loop().await
    }
}