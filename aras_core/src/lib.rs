use std::sync::Arc;

mod app_ready;
mod asgispec;
mod connection_info;
mod error;
mod http1_1;
mod lifespan;
mod lines_codec;
mod server;

pub use crate::asgispec::{ASGIApplication, ASGIMessage, ASGIScope, ReceiveFn, Scope, SendFn};
pub use crate::error::{Error, Result};
pub use crate::http1_1::{
    HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope,
};
pub use crate::lifespan::{
    LifespanScope, LifespanShutdown, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartup,
    LifespanStartupComplete, LifespanStartupFailed,
};
use crate::server::Server;

pub async fn serve(app: Arc<impl ASGIApplication + Send + Sync + 'static>, addr: [u8; 4], port: u16) -> Result<()> {
    let mut server = Server::new(addr.into(), port, app);
    server.serve().await?;
    Ok(())
}
