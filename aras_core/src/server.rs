use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use derive_more::Constructor;
use tokio::net::TcpListener;

use crate::app_ready::prepare_application;
use crate::asgispec::ASGIApplication;
use crate::connection_info::ConnectionInfo;
use crate::error::Result;
use crate::http1_1::HTTPHandler;
use crate::lifespan::LifespanHandler;
use crate::lines_codec::LinesCodec;

#[derive(Constructor)]
pub struct Server<T: ASGIApplication + Send + Sync + 'static> {
    addr: IpAddr,
    port: u16,
    application: Arc<T>,
}

impl<T: ASGIApplication + Send + Sync + 'static> Server<T> {
    pub async fn serve(&mut self) -> Result<()> {
        println!("Application starting...");
        let app_clone = self.application.clone();

        let mut handler = LifespanHandler::new(prepare_application(app_clone));
        if let Err(e) = handler.handle_startup().await {
            eprint!("Error in application startup: {e:?}");
        } else {
            println!("Application startup complete");
        }

        // Wait for an exit signal or the infinite loop
        // send shutdown event when exit signal is received.
        // If for some reason the server finishes first, it's an error
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                if let Err(e) = handler.handle_shutdown().await {
                    eprint!("Error shutting down application: {e:?}");
                };
                Ok(())
            }
            _ = self.run_server() => {
                Err("Server quit unexpectedly".into())
            }
        }
    }

    async fn run_server(&mut self) -> Result<()> {
        let socket_addr = SocketAddr::new(self.addr, self.port);
        let listener = TcpListener::bind(socket_addr).await?;
        println!("Listening on: {}", socket_addr);

        loop {
            match listener.accept().await {
                Ok((socket, client)) => {
                    println!("Received connection {}", &client);
                    let app_clone = self.application.clone();
                    tokio::spawn(async move {
                        let message_broker = LinesCodec::new(socket);
                        let connection = ConnectionInfo::new(client, socket_addr);
                        let prepped_app = prepare_application(app_clone);

                        let mut handler = HTTPHandler::new(message_broker, connection, prepped_app);
                        if let Err(e) = handler.handle().await {
                            eprintln!("Error while handling connection: {e:?}");
                        };
                    });
                }
                Err(e) => {
                    eprintln!("Failed to connect to client: {e:?}")
                }
            };
        }
    }
}
