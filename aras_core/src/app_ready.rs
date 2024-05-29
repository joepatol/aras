use std::future::Future;
use std::sync::Arc;

use derive_more::Constructor;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::asgispec::{ASGIApplication, ASGIMessage, SendFn, ReceiveFn, Scope};
use crate::error::Result;

#[derive(Constructor)]
// ASGI Application ready to be used in a protocol handler
pub struct ReadyApplication<T: ASGIApplication + Send + Sync + 'static> {
    application: Arc<T>,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: Option<mpsc::Sender<ASGIMessage>>,
    receive_queue: mpsc::Receiver<ASGIMessage>,
}

impl<T: ASGIApplication + Send + Sync> ReadyApplication<T> {
    // Close the send queue when the server is done
    // ASGI spec requires an error to be send to the application if 
    // receive is called after http.disconnect
    pub fn server_done(&mut self) {
        self.send_queue = None;
    }
    
    // Call the application with the given scope, returns a handle to it
    // application is run in a separate task so the caller can continue doing work
    pub async fn call(&self, scope: Scope) -> JoinHandle<Result<()>> {
        let send_clone = self.send.clone();
        let receive_clone = self.receive.clone();
        let app_clone = self.application.clone();
        tokio::spawn(async move {
            app_clone.call(scope, receive_clone, send_clone).await
        })
    }

    // Send a message to the application
    pub async fn send_to(&self, message: ASGIMessage) -> Result<()> {
        match &self.send_queue {
            Some(queue) => queue.send(message).await.map_err(|e| e.into()),
            None => Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotConnected, "Send queue closed")) as Box<dyn std::error::Error + Send + Sync>)
        }
    }

    // Receive a message from the application
    pub async fn receive_from(&mut self) -> Option<ASGIMessage> {
        self.receive_queue.recv().await
    }
}

pub fn prepare_application<T: ASGIApplication + Send + Sync + 'static>(application: Arc<T>) -> ReadyApplication<T> {
    let (app_tx, server_rx) = mpsc::channel(32);
    let (server_tx, app_rx_) = mpsc::channel(32);
    let app_rx  = Arc::new(Mutex::new(app_rx_));

    let receive_closure = move || -> Box<dyn Future<Output = Result<ASGIMessage>> + Sync + Send + Unpin> {
        let rxc = app_rx.clone();
        Box::new(Box::pin(async move {
            let data = rxc.lock().await.recv().await;
            // TODO: Should be IO error
            Ok(data.ok_or(Box::new(TryRecvError::Empty) as Box<dyn std::error::Error + Send + Sync>)?)
        }))
    };

    let send_closure = move |message: ASGIMessage| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
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
        Some(server_tx),
        server_rx,
    )
}