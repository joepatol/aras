use std::future::Future;
use std::sync::Arc;

use crate::error::Result;

pub const ASGI_VERSION: &str = "3.0";

pub type SendFn = Arc<
    dyn Fn(Vec<u8>) -> Box<dyn Future<Output = Result<()>> + Unpin + Sync + Send> + Send + Sync,
>;
pub type ReceiveFn =
    Arc<dyn Fn() -> Box<dyn Future<Output = Result<Option<Vec<u8>>>> + Unpin + Sync + Send> + Send + Sync>;

pub trait ASGIApplication {
    fn call(
        &self,
        scope: Scope,
        receive: ReceiveFn,
        send: SendFn,
    ) -> impl Future<Output = Result<()>> + Send + Sync;
}

pub enum SupportedASGISpecVersion {
    V2_0,
}

pub enum ScopeType {
    HTTP,
    LifeSpan,
}

pub enum HTTPVersion {
    V1_1,
}

impl From<ScopeType> for String {
    fn from(value: ScopeType) -> Self {
        match value {
            ScopeType::LifeSpan => String::from("lifespan"),
            ScopeType::HTTP => String::from("http"),
        }
    }
}

struct ASGIScope {
    version: String,
    spec_version: SupportedASGISpecVersion,
}

impl ASGIScope {
    pub fn new() -> Self {
        Self { version: ASGI_VERSION.to_string(), spec_version: SupportedASGISpecVersion::V2_0 }
    }
}

pub struct HTTPScope {
    type_: String,
    asgi: ASGIScope,
    http_version: HTTPVersion,
    method: String,
    scheme: String,
    path: String,
    raw_path: Option<Vec<u8>>,
    query_string: Vec<u8>,
    root_path: String,
    headers: Vec<(Vec<u8>, Vec<u8>)>,
    client: (String, u64),
    server: (String, u64),
    // State not supported for now
}

impl HTTPScope {
    pub fn new(
        http_version: HTTPVersion,
        method: String,
        scheme: String,
        path: String,
        raw_path: Option<Vec<u8>>,
        query_string: Vec<u8>,
        root_path: String,
        headers: Vec<(Vec<u8>, Vec<u8>)>,
        client: (String, u64),
        server: (String, u64),
    ) -> Self {
        Self {
            type_: String::from("http"),
            asgi: ASGIScope::new(),
            http_version,
            method,
            scheme,
            path,
            raw_path,
            query_string,
            root_path,
            headers,
            client,
            server,
        }
    }
}

pub struct Scope {
    pub scope_type: ScopeType,
}

impl Scope {
    pub fn new(scope_type: ScopeType) -> Self {
        Self { scope_type }
    }
}
