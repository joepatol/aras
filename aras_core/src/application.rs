use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_more::derive::Constructor;
use tokio::sync::{mpsc, Mutex};

use crate::asgispec::{ASGICallable, ASGIReceiveEvent, ASGISendEvent, ReceiveFn, Scope, SendFn, State};
use crate::error::Result;
use crate::Error;

#[derive(Constructor, Clone)]
pub struct Application<S: State, T: ASGICallable<S>> {
    asgi_callable: T,
    send: SendFn,
    receive: ReceiveFn,
    send_queue: mpsc::Sender<ASGIReceiveEvent>,
    receive_queue: Arc<Mutex<mpsc::Receiver<ASGISendEvent>>>,
    phantom_data: PhantomData<S>,
}

impl<S: State, T: ASGICallable<S>> Application<S, T> {
    // ASGI spec requires calls to `send` to raise an error once the client disconnected
    pub async fn disconnect_client(&mut self) {
        self.receive_queue.lock().await.close();
    }

    // Call the application with the given scope
    pub async fn call(&self, scope: Scope<S>) -> Result<()> {
        let send_clone = self.send.clone();
        let receive_clone = self.receive.clone();
        // If the application returns, it could be before the server expects it to.
        // Either because it raised on error or worse, it just returned.
        // To avoid waiting indefinitely on the next message (i.e. `receive_from` was called)
        // an internal message is send once the application quits.
        if let Err(e) = self.asgi_callable.call(scope, receive_clone, send_clone).await {
            (self.send)(ASGISendEvent::new_error(e.to_string())).await?;
            Err(e)
        } else if !self.receive_queue.lock().await.is_closed() {
            (self.send)(ASGISendEvent::new_app_stopped()).await?;
            Ok(())
        } else {
            Ok(())
        }
    }

    // Send a message to the application
    pub async fn send_to(&self, message: ASGIReceiveEvent) -> Result<()> {
        self.send_queue.send(message).await?;
        Ok(())
    }

    // Receive a message from the application
    pub async fn receive_from(&mut self) -> Result<Option<ASGISendEvent>> {
        Ok(self.receive_queue.lock().await.recv().await)
    }
}

#[derive(Clone)]
pub struct ApplicationFactory<S: State, T: ASGICallable<S>> {
    asgi_callable: T,
    phantom_data: PhantomData<S>,
}

impl<S: State, T: ASGICallable<S>> ApplicationFactory<S, T> {
    pub fn new(asgi_callable: T) -> Self {
        Self {
            asgi_callable,
            phantom_data: PhantomData,
        }
    }

    pub fn build(&self) -> Application<S, T> {
        let (app_tx, server_rx_) = mpsc::channel(64);
        let (server_tx, app_rx_) = mpsc::channel(64);

        // Make receivers Send and Sync, as we need to be able to send them between threads
        let app_rx = Arc::new(Mutex::new(app_rx_));
        let server_rx = Arc::new(Mutex::new(server_rx_));

        let receive_closure = move || -> Box<dyn Future<Output = Result<ASGIReceiveEvent>> + Sync + Send + Unpin> {
            let rxc = app_rx.clone();
            Box::new(Box::pin(async move {
                let data = rxc.lock().await.recv().await;
                Ok(data.ok_or(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Received empty message",
                ))?)
            }))
        };

        let send_closure = move |message: ASGISendEvent| -> Box<dyn Future<Output = Result<()>> + Sync + Send + Unpin> {
            let txc = app_tx.clone();
            Box::new(Box::pin(async move {
                if let Err(_) = txc.send(message).await {
                    return Err(Error::disconnected_client());
                }
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
