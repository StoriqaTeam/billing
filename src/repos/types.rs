use diesel::pg::PgConnection;
use failure::Error as FailureError;
use futures::future::Future;
use r2d2;
use r2d2_diesel::ConnectionManager;

use repos::Error as RepoError;

/// Repos layer Future
pub type RepoFuture<T> = Box<Future<Item = T, Error = FailureError>>;
pub type RepoResult<T> = Result<T, FailureError>;
pub type RepoResultV2<T> = Result<T, RepoError>;
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;
