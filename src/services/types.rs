use failure::Error as FailureError;
use futures::Future;

/// Service layer Future
pub type ServiceFuture<T> = Box<Future<Item = T, Error = FailureError>>;
