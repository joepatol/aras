use std::net::SocketAddr;

use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{info, error};
use tokio::net::TcpListener;

use super::connection_info::ConnectionInfo;
use super::config::ServerConfig;
use crate::http::HTTP11Handler;
use crate::application::ApplicationFactory;
use crate::asgispec::ASGICallable;
use crate::error::{Result, Error};
use crate::lifespan::LifespanHandler;
use crate::middleware_services::Logger;

pub struct Server<T: ASGICallable> {
    asgi_factory: ApplicationFactory<T>,
}

impl<T: ASGICallable> Server<T> {
    pub fn new(asgi_callable: T) -> Self {
        Self { asgi_factory: ApplicationFactory::new(asgi_callable) }
    }
}

impl<T: ASGICallable + 'static> Server<T> {
    pub async fn serve(&mut self, config: ServerConfig) -> Result<()> {
        let mut lifespan_handler = LifespanHandler::new(self.asgi_factory.build());
        if let Err(e) = lifespan_handler.handle_startup().await {
            return Err(e)
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
            let asgi_app = self.asgi_factory.build();
            let conn_info = ConnectionInfo::new(client, socket_addr);

            tokio::task::spawn(async move {
                let svc = tower::ServiceBuilder::new()
                    .layer_fn(Logger::new)
                    .service(HTTP11Handler::new(asgi_app, conn_info));
                
                if let Err(err) = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .keep_alive(config.keep_alive)
                    .serve_connection(io, svc)
                    .await
                {
                    error!("Error serving connection: {:?}", err);
                }
            });
        };
    }
}