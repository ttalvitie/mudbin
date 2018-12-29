use crate::errors::{Error, ErrorKind, ResultExt};

use std::error;

use futures::Future;

pub type BoxFuture<T> = Box<futures::Future<Item = T, Error = Error> + Send>;

pub trait FutureChainErrExt<T> {
    fn chain_err<C, E>(self, callback: C) -> BoxFuture<T>
    where
        C: FnOnce() -> E + Send + 'static,
        E: Into<ErrorKind>;
}

impl<F> FutureChainErrExt<F::Item> for F
where
    F: Future + Send + 'static,
    F::Item: Send,
    F::Error: error::Error + Send + 'static,
{
    fn chain_err<C, E>(self, callback: C) -> BoxFuture<F::Item>
    where
        C: FnOnce() -> E + Send + 'static,
        E: Into<ErrorKind>,
    {
        Box::new(self.then(|r| r.chain_err(callback)))
    }
}

pub trait FutureIntoBoxExt<T> {
    fn into_box(self) -> BoxFuture<T>;
}

impl<F> FutureIntoBoxExt<F::Item> for F
where
    F: Future<Error = Error> + Send + 'static,
    F::Item: Send,
{
    fn into_box(self) -> BoxFuture<F::Item> {
        Box::new(self)
    }
}
