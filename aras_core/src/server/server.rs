use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::TryFutureExt;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{error, info};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use super::config::ServerConfig;
use super::connection_info::ConnectionInfo;
use super::service::ASGIService;
use crate::application::ApplicationFactory;
use crate::asgispec::{ASGICallable, State};
use crate::error::{Error, Result};
use crate::lifespan::LifespanHandler;
use crate::middleware_services::{ConcurrencyLimit, Logger};

pub struct Server<S: State, T: ASGICallable<S>> {
    app_factory: ApplicationFactory<S, T>,
    state: S,
}

impl<S: State, T: ASGICallable<S>> Server<S, T> {
    pub fn new(asgi_callable: T, state: S) -> Self {
        Self {
            app_factory: ApplicationFactory::new(asgi_callable),
            state,
        }
    }
}

impl<S: State + 'static, T: ASGICallable<S> + 'static> Server<S, T> {
    pub async fn serve(&mut self, config: ServerConfig) -> Result<()> {
        let lifespan_handler = LifespanHandler::new(self.app_factory.build())
            .startup(self.state.clone())
            .await?;

        // Wait for an exit signal or the server loop
        // send shutdown event when exit signal is received.
        tokio::select! {
            _ = tokio::signal::ctrl_c() => lifespan_handler.shutdown().await,
            out = self.run_server(config).map_err(|e| Error::unexpected_shutdown("server", e.to_string())) => out,
        }
    }

    async fn run_server(&mut self, config: ServerConfig) -> Result<()> {
        let socket_addr = SocketAddr::new(config.addr, config.port);
        let listener = TcpListener::bind(socket_addr).await?;
        let semaphore = Arc::new(Semaphore::new(config.limit_concurrency));
        let state = self.state.clone();
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
            let iter_state = state.clone();
            let factory_clone = self.app_factory.clone();
            let iter_semaphore = semaphore.clone();
            let conn_info = ConnectionInfo::new(client, socket_addr);
            info!("Connecting new client {client}");

            tokio::task::spawn(async move {
                let svc = tower::ServiceBuilder::new()
                    .layer_fn(Logger::new)
                    .layer_fn(ConcurrencyLimit::new(iter_semaphore).layer())
                    .service(ASGIService::new(factory_clone, conn_info, iter_state));

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
