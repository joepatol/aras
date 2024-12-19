use crate::asgispec::ASGIScope;

#[derive(Debug, Clone)]
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