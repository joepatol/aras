mod asgispec;
mod types;
mod error;
mod http;
mod lifespan;
mod server;
mod websocket;
mod application;
mod middleware_services;

pub use crate::asgispec::{ASGICallable, ASGIMessage, ASGIScope, ReceiveFn, Scope, SendFn, State};
pub use crate::error::{Error, Result};
pub use crate::http::{
    HTTPDisconnectEvent, HTTPRequestEvent, HTTPResonseBodyEvent, HTTPResponseStartEvent, HTTPScope,
};
pub use crate::lifespan::{
    LifespanScope, LifespanShutdown, LifespanShutdownComplete, LifespanShutdownFailed, LifespanStartup,
    LifespanStartupComplete, LifespanStartupFailed,
};
pub use crate::websocket::{
    WebsocketAcceptEvent, WebsocketCloseEvent, WebsocketConnectEvent, WebsocketDisconnectEvent, WebsocketReceiveEvent,
    WebsocketScope, WebsocketSendEvent,
};
use crate::server::Server;
pub use crate::server::ServerConfig;

pub async fn serve<S: State + 'static, T: ASGICallable<S> + 'static>(app: T, state: S, config: Option<ServerConfig>) -> Result<()> {
    let mut server = Server::new(app, state);
    server.serve(config.unwrap_or_default()).await?;
    Ok(())
}
