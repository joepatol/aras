use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use derive_more::Constructor;
use tokio::net::TcpListener;

use crate::asgispec::ASGIApplication;
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::http1_1::HTTPHandler;
use crate::connection_info::ConnectionInfo;
use crate::app_ready::prepare_application;

#[derive(Clone, Constructor)]
pub struct Server<T: ASGIApplication + Send + Sync + 'static> {
    addr: IpAddr,
    port: u16,
    application: Arc<T>,
}

impl<T: ASGIApplication + Send + Sync + 'static> Server<T> {
    pub async fn serve(&self) -> Result<()> {
        println!("Application starting...");
        println!("Application startup complete");

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
                            eprint!("Error while handling connection: {e:?}");
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