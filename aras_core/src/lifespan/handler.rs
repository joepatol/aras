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
                eprintln!("{}", &event.message);
                Err("startup failed".into())
            },
            _ => {
                println!("Lifespan protocol appears unsupported");
                self.in_use = false;
                Ok(())
            }
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
                println!("Application startup complete");
                Ok(())
            },
            Err(e) => {
                println!("Application startup failed. {:?}", e);
                Err(e)
            }
        }
    }

    pub async fn handle_shutdown(&mut self) -> Result<()> {
        self.shutdown_loop().await
    }
}