use std::fmt::Debug;
use std::sync::Arc;

use derive_more::derive::Constructor;
use hyper::service::Service;
use hyper::Request;
use hyper::body::Incoming;
use tokio::sync::Semaphore;

use crate::types::{Response, ServiceFuture};
use crate::error::Error;

#[derive(Constructor, Debug, Clone)]
pub struct ConcurrencyLimit {
    semaphore: Arc<Semaphore>,
}

impl ConcurrencyLimit {
    pub fn layer<S>(self) -> impl Fn(S) -> ConcurrencyLimitLayer<S> 
    where
        S: Service<
            Request<Incoming>,
            Response = Response,
            Error = Error,
            Future = ServiceFuture,
        > + Send + Sync + 'static,
    {   
        move |inner: S| -> ConcurrencyLimitLayer<S> {
            ConcurrencyLimitLayer::new(Arc::new(inner), self.semaphore.clone())
        }
    }
}

#[derive(Constructor, Debug, Clone)]
pub struct ConcurrencyLimitLayer<S> {
    inner: Arc<S>,
    semaphore: Arc<Semaphore>,
}

impl<S> Service<Request<Incoming>> for ConcurrencyLimitLayer<S>
where
    S: Service<
        Request<Incoming>, 
        Response = Response,
        Error = Error, 
        Future = ServiceFuture,
    > + Send + Sync + 'static,
{
    type Error = S::Error;
    type Response = S::Response;
    type Future = S::Future;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let inner_clone = self.inner.clone();
        let semaphore_clone = self.semaphore.clone();
        Box::pin(async move {
            let _permit = semaphore_clone
                .acquire()
                .await
                .expect("Semaphore in `ConcurrencyLimit` closed, this should never happen!");
            inner_clone.call(req).await
        })
    }
}