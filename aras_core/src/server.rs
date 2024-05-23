use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use derive_more::Constructor;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::ASGIApplication;
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::http1_1::HTTPHandler;
use crate::connection_info::ConnectionInfo;
use crate::app_ready::ReadyApplication;

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
                    println!("Received connection");
                    let app_clone = self.application.clone();
                    tokio::spawn(async move {
                        let message_broker = LinesCodec::new(socket);
                        let connection = ConnectionInfo::new(client, socket_addr);
                        let prepped_app = prepare_application(app_clone);
                        
                        let mut handler = HTTPHandler::new(message_broker, connection, prepped_app);
                        if let Err(e) = handler.handle().await {
                            eprint!("Error while handling connection: {}", e);
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

fn prepare_application<T: ASGIApplication + Send + Sync + 'static>(application: Arc<T>) -> ReadyApplication<T> {
    let (app_tx, server_rx) = mpsc::channel(32);
    let (server_tx, app_rx_) = mpsc::channel(32);
    let app_rx  = Arc::new(Mutex::new(app_rx_));

    let receive_closure = move || -> Box<dyn Future<Output = Result<Option<Vec<u8>>>> + Sync + Send + Unpin> {
        let rxc = app_rx.clone();
        Box::new(Box::pin(async move {
            let data = rxc.lock().await.recv().await;
            Ok(data)
        }))
    };

    let send_closure = move |message: Vec<u8>| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
        let txc = app_tx.clone();
        Box::new(Box::pin(async move {
            txc.send(message)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }))
    };

    ReadyApplication::new(
        application, 
        Arc::new(send_closure), 
        Arc::new(receive_closure),
        server_tx,
        server_rx,
    )
}