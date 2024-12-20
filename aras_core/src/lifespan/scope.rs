use crate::asgispec::ASGIScope;

#[derive(Debug, Clone)]
pub struct LifespanScope<S: Clone + Send + Sync> {
    pub type_: String,
    pub asgi: ASGIScope,
    pub state: S,
}

impl<S: Clone + Send + Sync> LifespanScope<S> {
    pub fn new(state: S) -> Self {
        Self { type_: "lifespan".into(), asgi: ASGIScope::new(), state}
    }
}