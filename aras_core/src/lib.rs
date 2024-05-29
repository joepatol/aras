use std::sync::Arc;

mod asgispec;
mod error;
mod lines_codec;
mod server;
mod http1_1;
mod connection_info;
mod app_ready;
mod lifespan;
mod websocket;

pub use crate::lifespan::{LifespanScope, LifespanShutdown, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartup, LifespanStartupComplete, LifespanStartupFailed};
pub use crate::asgispec::{ASGIApplication, ReceiveFn, SendFn, Scope, ASGIMessage, ASGIScope};
pub use crate::error::{Error, Result};
pub use crate::http1_1::{HTTPResponseStartEvent, HTTPResonseBodyEvent, HTTPRequestEvent, HTTPDisconnectEvent, HTTPScope};
use crate::server::Server;

pub async fn serve(app: Arc<impl ASGIApplication + Send + Sync + 'static>, addr: [u8; 4], port: u16) -> Result<()> {
    let mut server = Server::new(addr.into(), port, app);
    server.serve().await?;
    Ok(())
}
