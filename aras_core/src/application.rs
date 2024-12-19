use std::future::Future;
use std::sync::Arc;

use derive_more::derive::Constructor;
use log::debug;
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::{ASGICallable, ASGIMessage, ReceiveFn, Scope, SendFn};
use crate::error::Result;
use crate::Error;

#[derive(Constructor)]
pub struct Application<T: ASGICallable> {
    asgi_callable: Arc<T>,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: mpsc::Sender<ASGIMessage>,
    receive_queue: Option<Arc<Mutex<mpsc::Receiver<ASGIMessage>>>>,
}

impl<T: ASGICallable> Clone for Application<T> {
    fn clone(&self) -> Self {
        Self {
            asgi_callable: self.asgi_callable.clone(),
            send: self.send.clone(),
            receive: self.receive.clone(),
            send_queue: self.send_queue.clone(),
            receive_queue: self.receive_queue.clone(),
        }
    }
}

impl<T: ASGICallable> Application<T> {
    // ASGI spec requires calls to `send` to raise an error once disconnected
    pub fn set_send_is_error(&mut self) {
        self.send = error_on_send();
    }

    // Call the application with the given scope
    pub async fn call(&self, scope: Scope) -> Result<()> {
        let send_clone = self.send.clone();
        let receive_clone = self.receive.clone();
        self.asgi_callable.call(scope, receive_clone, send_clone).await
    }

    // Send a message to the application
    pub async fn send_to(&self, message: ASGIMessage) -> Result<()> {
        debug!("Message put on queue: {message}");
        self.send_queue.send(message).await?;
        Ok(())
    }

    // Receive a message from the application
    pub async fn receive_from(&mut self) -> Result<Option<ASGIMessage>> {
        match &mut self.receive_queue {
            Some(queue) => Ok(queue.lock().await.recv().await),
            None => Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "channel closed"))?,
        }
    }
}

#[derive(Clone)]
pub struct ApplicationFactory<T: ASGICallable> {
    asgi_callable: Arc<T>,
}

impl<T: ASGICallable> ApplicationFactory<T> {
    pub fn new(asgi_callable: T) -> Self {
        Self {
            asgi_callable: Arc::new(asgi_callable),
        }
    }

    pub fn build(&self) -> Application<T> {
        let (app_tx, server_rx_) = mpsc::channel(32);
        let (server_tx, app_rx_) = mpsc::channel(32);

        // Make receivers Send and Sync, as we need to be able to send them between threads
        // TODO: is this really required? Potentially this can be avoided...
        let app_rx = Arc::new(Mutex::new(app_rx_));
        let server_rx = Arc::new(Mutex::new(server_rx_));

        let receive_closure = move || -> Box<dyn Future<Output = Result<ASGIMessage>> + Sync + Send + Unpin> {
            let rxc = app_rx.clone();
            Box::new(Box::pin(async move {
                let data = rxc.lock().await.recv().await;
                Ok(data.ok_or(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Received empty message",
                ))?)
            }))
        };

        let send_closure = move |message: ASGIMessage| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
            let txc = app_tx.clone();
            Box::new(Box::pin(async move {
                txc.send(message).await?;
                Ok(())
            }))
        };

        Application::new(
            self.asgi_callable.clone(),
            Arc::new(send_closure),
            Arc::new(receive_closure),
            server_tx,
            Some(server_rx),
        )
    }
}

fn error_on_send() -> SendFn {
    let func = move |_: ASGIMessage| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
        Box::new(Box::pin(async move {
            Err(Error::disconnected_client())
        }))
    };
    Arc::new(func)
}
