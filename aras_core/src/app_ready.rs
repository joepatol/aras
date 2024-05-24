use std::future::Future;
use std::sync::Arc;

use derive_more::Constructor;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::{ASGIApplication, ASGIMessage, SendFn, ReceiveFn, Scope};
use crate::error::Result;

#[derive(Constructor)]
pub struct ReadyApplication<T: ASGIApplication + Send + Sync + 'static> {
    application: Arc<T>,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: mpsc::Sender<ASGIMessage>,
    receive_queue: mpsc::Receiver<ASGIMessage>,
}

impl<T: ASGIApplication + Send + Sync> ReadyApplication<T> {
    pub async fn call(&self, scope: Scope) -> Result<()> {
        self.application.call(scope, self.receive.clone(), self.send.clone()).await
    }

    pub async fn send_to(&self, message: ASGIMessage) -> Result<()> {
        self.send_queue.send(message).await.map_err(|e| e.into())
    }

    pub fn try_receive_from(&mut self) -> Result<ASGIMessage> {
        self.receive_queue.try_recv().map_err(|e| e.into())
    }

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
            let data = rxc.lock().await.recv().await
            .ok_or(Box::new(TryRecvError::Empty) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(data)
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
        server_tx,
        server_rx,
    )
}