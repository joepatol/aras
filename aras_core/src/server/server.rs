use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{error, info};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::service::ASGIService;
use super::config::ServerConfig;
use super::connection_info::ConnectionInfo;
use crate::application::ApplicationFactory;
use crate::asgispec::ASGICallable;
use crate::error::{Error, Result};
use crate::lifespan::LifespanHandler;
use crate::middleware_services::{ConcurrencyLimit, Logger};

pub struct Server<T: ASGICallable> {
    app_factory: ApplicationFactory<T>,
}
 
impl<T: ASGICallable + Clone> Server<T> {
    pub fn new(asgi_callable: T) -> Self {
        Self {
            app_factory: ApplicationFactory::new(asgi_callable),
        }
    }
}

impl<T: ASGICallable + Clone + 'static> Server<T> {
    pub async fn serve(&mut self, config: ServerConfig) -> Result<()> {
        let mut lifespan_handler = LifespanHandler::new(self.app_factory.build());
        if let Err(e) = lifespan_handler.handle_startup().await {
            return Err(e);
        };

        // Wait for an exit signal or the server loop
        // send shutdown event when exit signal is received.
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting...");
                if let Err(e) = lifespan_handler.handle_shutdown().await {
                    error!("Error shutting down application: {e}");
                };
                Ok(())
            }
            server_output = self.run_server(config) => {
                if let Err(e) = server_output {
                    error!("Server quit unexpectedly; {:?}", e.to_string());
                    return Err(Error::UnexpectedShutdown { src: "server".into(), reason: e.to_string() })
                };
                Ok(())
            }
        }
    }

    async fn run_server(&mut self, config: ServerConfig) -> Result<()> {
        let socket_addr = SocketAddr::new(config.addr, config.port);
        let listener = TcpListener::bind(socket_addr).await?;
        let semaphore = Arc::new(Semaphore::new(config.limit_concurrency));
        info!("Listening on http://{}", socket_addr);

        loop {
            let (tcp, client) = match listener.accept().await {
                Ok((t, c)) => (t, c),
                Err(e) => {
                    error!("Failed to connect to client: {e}");
                    continue;
                }
            };

            let io = TokioIo::new(tcp);
            let factory_clone = self.app_factory.clone();
            let iter_semaphore = semaphore.clone();
            let conn_info = ConnectionInfo::new(client, socket_addr);
            info!("Connecting new client {client}");

            tokio::task::spawn(async move {
                let svc = tower::ServiceBuilder::new()
                    .layer_fn(Logger::new)
                    .layer_fn(ConcurrencyLimit::new(iter_semaphore).layer())
                    .service(ASGIService::new(factory_clone, conn_info));

                if let Err(err) = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(60))
                    .keep_alive(config.keep_alive)
                    .serve_connection(io, svc)
                    .with_upgrades()
                    .await
                {
                    if err.is_closed() || err.is_timeout() {
                        info!("Disconnected client {client}");
                    } else {
                        error!("Error serving connection: {:?}", err);
                    };
                }
            });
        }
    }
}