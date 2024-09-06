mod asgispec;
mod error;
mod http;
mod lifespan;
mod server;
mod websocket;
mod application;

pub use crate::asgispec::{ASGICallable, ASGIMessage, ASGIScope, ReceiveFn, Scope, SendFn};
pub use crate::error::{Error, Result};
pub use crate::http::{
    HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope,
};
pub use crate::lifespan::{
    LifespanScope, LifespanShutdown, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartup,
    LifespanStartupComplete, LifespanStartupFailed,
};
use crate::server::Server;
pub use crate::server::ServerConfig;

pub async fn serve<T: ASGICallable + 'static>(app: T, config: Option<ServerConfig>) -> Result<()> {
    let mut server = Server::new(app);
    server.serve(config.unwrap_or_default()).await?;
    Ok(())
}
