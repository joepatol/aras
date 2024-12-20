use std::io;

use thiserror::Error;

use crate::ASGIMessage;

pub type Result<T> = std::result::Result<T, Error>;

// Errors the ASGI server could raise
#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Custom(String),

    #[error("{0}")]
    DisconnectedClient(String),

    #[error(transparent)]
    Hyper(#[from] hyper::Error),

    #[error(transparent)]
    HTTP(#[from] http::Error),
    
    #[error("{src} shutdown unexpectedly. {reason}")]
    UnexpectedShutdown {
        src: String,
        reason: String,
    },

    #[error(transparent)]
    IO(#[from] io::Error),

    #[error("Invalid ASGI state change. Received {received}, expected one of {expected:?}")]
    InvalidASGIStateChange {
        received: String,
        expected: Vec<String>,
    },

    #[error("Invalid ASGI message received. {msg:?}")]
    InvalidASGIMessage {
        msg: Box<dyn std::fmt::Debug + Send + Sync>,
    },
    
    #[error(transparent)]
    ChannelSendError(#[from] tokio::sync::mpsc::error::SendError<ASGIMessage>),

    #[error("Disconnect")]
    Disconnect,

    #[error(transparent)]
    SemaphoreAcquireError(#[from] tokio::sync::AcquireError),

    #[error(transparent)]
    WebsocketError(#[from] fastwebsockets::WebSocketError),
}

impl Error {
    pub fn custom(val: impl std::fmt::Display) -> Self {
        Self::Custom(val.to_string())
    }

    pub fn state_change(received: &str, expected: Vec<&str>) -> Self {
        Self::InvalidASGIStateChange { received: received.to_owned(), expected: expected.into_iter().map(|r| r.to_owned()).collect() }
    }

    pub fn invalid_asgi_message(msg: Box<dyn std::fmt::Debug + Send + Sync>) -> Self {
        Self::InvalidASGIMessage { msg }
    }

    pub fn disconnected_client() -> Self {
        Self::DisconnectedClient(String::from("Disconnected client"))
    }

    pub fn unexpected_shutdown(src: &str, reason: String) -> Self {
        Self::UnexpectedShutdown { src: src.to_string(), reason: reason }
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::custom(value.to_string())
    }
}