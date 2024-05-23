use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::ASGIApplication;
use crate::error::Result;
use crate::lines_codec::LinesCodec;
use crate::http1_1::parse_http;

#[derive(Clone)]
pub struct Server<T: ASGIApplication + Send + Sync + 'static> {
    addr: IpAddr,
    port: u16,
    application: Arc<T>,
}

impl<T: ASGIApplication + Send + Sync + 'static> Server<T> {
    pub fn new(addr: IpAddr, port: u16, application: Arc<T>) -> Self {
        Self {
            addr,
            port,
            application,
        }
    }

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
                        let mut handler = ConnectionHandler::new(app_clone, socket_addr);
                        if let Err(e) = handler.handle(socket, client).await {
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

pub struct ConnectionHandler<T: ASGIApplication + Send + Sync + 'static> {
    application: Arc<T>,
    server: SocketAddr,
    app_receiver: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
    server_receiver: mpsc::Receiver<Vec<u8>>,
    app_sender: mpsc::Sender<Vec<u8>>,
    server_sender: mpsc::Sender<Vec<u8>>,
}

impl<T: ASGIApplication + Send + Sync + 'static> ConnectionHandler<T> {
    pub fn new(application: Arc<T>, server: SocketAddr) -> Self {
        let (app_tx, server_rx) = mpsc::channel(32);
        let (server_tx, app_rx) = mpsc::channel(32);

        Self {
            application,
            server,
            app_receiver: Arc::new(Mutex::new(app_rx)), // To be shared with application
            server_receiver: server_rx,
            app_sender: app_tx,
            server_sender: server_tx,
        }
    }

    pub async fn handle(&mut self, stream: TcpStream, client: SocketAddr) -> Result<()> {
        let mut codec = LinesCodec::new(stream);
        let (scope, body) = parse_http(&mut codec, client, self.server).await?;
        self.server_sender.send(body).await?;

        let receiver_clone = self.app_receiver.clone();
        let receive_closure = move || -> Box<dyn Future<Output = Result<Option<Vec<u8>>>> + Sync + Send + Unpin> {
            let rxc = receiver_clone.clone();
            Box::new(Box::pin(async move {
                let data = rxc.lock().await.recv().await;
                Ok(data)
            }))
        };

        let sender_clone = self.app_sender.clone();
        let send_closure = move |message: Vec<u8>| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
            let txc = sender_clone.clone();
            Box::new(Box::pin(async move {
                txc.send(message)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }))
        };

        println!("Calling application");
        self.application
            .call(scope, Arc::new(receive_closure), Arc::new(send_closure))
            .await?;

        loop {
            match self.server_receiver.try_recv() {
                Ok(msg) => codec.send_message(msg.as_slice()).await?,
                Err(_) => break,
            }
        }

        Ok(())
    }
}