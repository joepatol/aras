use std::sync::Arc;

mod asgispec;
mod error;
mod lines_codec;
mod server;
mod http1_1;
mod connection_info;
mod app_ready;

pub use crate::asgispec::{ASGIApplication, ReceiveFn, SendFn, Scope, ASGIMessage};
pub use crate::error::{Error, Result};
use crate::server::Server;

pub async fn serve(app: Arc<impl ASGIApplication + Send + Sync + 'static>) -> Result<()> {
    let server = Server::new([127, 0, 0, 1].into(), 80, app);
    server.serve().await?;
    Ok(())
}
