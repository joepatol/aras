use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use log::{info, error, debug};
use derive_more::Constructor;
use tokio::net::TcpListener;
use object_pool::{Pool, Reusable};

use crate::app_ready::prepare_application;
use crate::asgispec::ASGIApplication;
use crate::connection_info::ConnectionInfo;
use crate::error::{Result, Error};
use crate::http1_1::HTTP11Handler;
use crate::lifespan::LifespanHandler;
use crate::lines_codec::LinesCodec;

pub struct ServerConfig {
    pub t_keep_alive: usize,
    pub buf_pool_size: usize,
    pub limit_concurrency: Option<usize>,
    pub buffer_capacity: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            t_keep_alive: 5,
            buf_pool_size: 100,
            limit_concurrency: None,  // TODO: implement usage
            buffer_capacity: 2056,
        }
    }
}

#[derive(Constructor)]
pub struct Server<T: ASGIApplication + Send + Sync + 'static> {
    addr: IpAddr,
    port: u16,
    application: Arc<T>,
}

impl<T: ASGIApplication + Send + Sync + Clone + 'static> Server<T> {
    pub async fn serve(&mut self, config: ServerConfig) -> Result<()> {
        let app_clone = self.application.clone();

        let mut lifespan_handler = LifespanHandler::new(prepare_application(app_clone));
        if let Err(e) = lifespan_handler.handle_startup().await {
            return Err(e)
        };

        // Wait for an exit signal or the infinite loop
        // send shutdown event when exit signal is received.
        // If for some reason the server finishes first, it's an error
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting...");
                if let Err(e) = lifespan_handler.handle_shutdown().await {
                    error!("Error shutting down application: {e}");
                };
                Ok(())
            }
            r = self.run_server(config) => {
                match r {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::UnexpectedShutdown { src: "server".into(), reason: e.to_string() })
                }
            }
        }
    }

    async fn run_server(&mut self, config: ServerConfig) -> Result<()> {
        let socket_addr = SocketAddr::new(self.addr, self.port);
        let listener = TcpListener::bind(socket_addr).await?;
        info!("Listening on: {}", socket_addr);

        let buffer_pool: Arc<Pool<Vec<u8>>> = Arc::new(Pool::new(config.buf_pool_size, || vec![0; config.buffer_capacity]));

        loop {
            match listener.accept().await {
                Ok((socket, client)) => {
                    debug!("Received connection {}", &client);
                    let app_clone = self.application.clone();
                    let buf_pool_clone = buffer_pool.clone();
                    tokio::spawn(async move {
                        let message_broker = LinesCodec::new(socket);
                        let connection = ConnectionInfo::new(client, socket_addr);
                        let prepped_app = prepare_application(app_clone);
                        let buffer = buf_pool_clone
                            .try_pull()
                            .unwrap_or(Reusable::new(&buf_pool_clone, vec![0; config.buffer_capacity]));

                        let handler = HTTP11Handler::new( 
                            &connection, 
                            config.t_keep_alive,
                        );
                        if let Err(e) = handler.connect(message_broker, buffer, prepped_app).await {
                            error!("Error while handling connection: {e}");
                        };
                    });
                }
                Err(e) => {
                    error!("Failed to connect to client: {e}")
                }
            };
        }
    }
}