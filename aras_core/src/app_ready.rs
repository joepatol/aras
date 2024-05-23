use std::sync::Arc;

use derive_more::Constructor;
use tokio::sync::mpsc;

use crate::asgispec::{ASGIApplication, SendFn, ReceiveFn, Scope};
use crate::error::Result;

#[derive(Constructor)]
pub struct ReadyApplication<T: ASGIApplication + Send + Sync + 'static> {
    application: Arc<T>,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: mpsc::Sender<Vec<u8>>,
    receive_queue: mpsc::Receiver<Vec<u8>>,
}

impl<T: ASGIApplication + Send + Sync> ReadyApplication<T> {
    pub async fn call(&self, scope: Scope) -> Result<()> {
        self.application.call(scope, self.receive.clone(), self.send.clone()).await
    }

    pub async fn send_to(&self, message: Vec<u8>) -> Result<()> {
        self.send_queue.send(message).await.map_err(|e| e.into())
    }

    pub fn try_receive_from(&mut self) -> Result<Vec<u8>> {
        self.receive_queue.try_recv().map_err(|e| e.into())
    }

    pub async fn receive_from(&mut self) -> Option<Vec<u8>> {
        self.receive_queue.recv().await
    }
}