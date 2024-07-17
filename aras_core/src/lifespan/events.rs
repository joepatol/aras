use crate::asgispec::ASGIScope;

#[derive(Debug)]
pub struct LifespanScope {
    pub type_: String,
    pub asgi: ASGIScope,
    // State not supported for now
}

impl LifespanScope {
    pub fn new() -> Self {
        Self { type_: "lifespan".into(), asgi: ASGIScope::new()}
    }
}

#[derive(Debug)]
pub struct LifespanStartup {
    pub type_: String,
}

impl LifespanStartup {
    pub fn new() -> Self {
        Self { type_: "lifespan.startup".into() }
    }
}

#[derive(Debug)]
pub struct LifespanStartupComplete {
    pub type_: String,
}

impl LifespanStartupComplete {
    pub fn new() -> Self {
        Self { type_: "lifespan.startup.complete".into() }
    }
}

#[derive(Debug)]
pub struct LifespanStartupFailed {
    pub type_: String,
    pub message: String,
}

impl LifespanStartupFailed {
    pub fn new(message: String) -> Self {
        Self { type_: "lifespan.startup.failed".into(), message }
    }
}

#[derive(Debug)]
pub struct LifespanShutdown {
    pub type_: String,
}

impl LifespanShutdown {
    pub fn new() -> Self {
        Self { type_: "lifespan.shutdown".into() }
    }
}

#[derive(Debug)]
pub struct LifespanShutdownComplete {
    pub type_: String,
}

impl LifespanShutdownComplete {
    pub fn new() -> Self {
        Self { type_: "lifespan.shutdown.complete".into() }
    }
}

#[derive(Debug)]
pub struct LifespanShutdownFailed {
    pub type_: String,
    pub message: String,
}

impl LifespanShutdownFailed {
    pub fn new(message: String) -> Self {
        Self { type_: "lifespan.shutdown.failed".into(), message }
    }
}