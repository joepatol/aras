use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_more::derive::Constructor;
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::{ASGICallable, State, ASGIMessage, ReceiveFn, Scope, SendFn};
use crate::error::Result;
use crate::Error;

#[derive(Constructor, Clone)]
pub struct Application<S: State, T: ASGICallable<S>> {
    asgi_callable: T,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: mpsc::Sender<ASGIMessage>,
    receive_queue: Arc<Mutex<mpsc::Receiver<ASGIMessage>>>,
    phantom_data: PhantomData<S>,
}

impl<S: State, T: ASGICallable<S>> Application<S, T> {
    // ASGI spec requires calls to `send` to raise an error once disconnected
    pub fn set_send_is_error(&mut self) {
        self.send = create_send_error_fn();
    }

    // Call the application with the given scope
    pub async fn call(&self, scope: Scope<S>) -> Result<()> {
        let send_clone = self.send.clone();
        let receive_clone = self.receive.clone();
        if let Err(e) = self.asgi_callable.call(scope, receive_clone, send_clone).await {
            // If the application returns an error, we need to send a message
            // So any pending `receive_from` calls can return
            (self.send)(ASGIMessage::new_error()).await?;
            return Err(e);
        };
        Ok(())
    }

    // Send a message to the application
    pub async fn send_to(&self, message: ASGIMessage) -> Result<()> {
        self.send_queue.send(message).await?;
        Ok(())
    }

    // Receive a message from the application
    pub async fn receive_from(&mut self) -> Result<Option<ASGIMessage>> {
        Ok(self.receive_queue.lock().await.recv().await)
    }
}

#[derive(Clone, Constructor)]
pub struct ApplicationFactory<S: State, T: ASGICallable<S>> {
    asgi_callable: T,
    phantom_data: PhantomData<S>,
}

impl<S: State, T: ASGICallable<S>> ApplicationFactory<S, T> {
    pub fn build(&self) -> Application<S, T> {
        let (app_tx, server_rx_) = mpsc::channel(32);
        let (server_tx, app_rx_) = mpsc::channel(32);

        // Make receivers Send and Sync, as we need to be able to send them between threads
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
            server_rx,
            PhantomData,
        )
    }
}

fn create_send_error_fn() -> SendFn {
    let func = move |_: ASGIMessage| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
        Box::new(Box::pin(async move {
            Err(Error::disconnected_client())
        }))
    };
    Arc::new(func)
}
