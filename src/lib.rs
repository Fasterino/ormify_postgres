//! # ormify_postgres
//!
//! PostgreSQL language and driver implementations for [`ormify`](https://docs.rs/ormify).
//!
//! [`PostgresSql`] compiles ORM schemes, queries, and executions into
//! PostgreSQL SQL with quoted identifiers and `$n` parameters. A scheme's
//! `group` is used as the PostgreSQL schema name.
//!
//! Enable one driver feature for database access:
//!
//! - `tokio_postgres_engine` exposes [`TokioEngine`] and requires a Tokio
//!   runtime supplied by the application.
//! - `tokio_sqlx_engine` exposes [`SqlxEngine`] with SQLx's Tokio runtime.
//! - `async_std_sqlx_engine` exposes [`SqlxEngine`] with SQLx's async-std
//!   runtime.
//!
//! Initialization emits schema/table/constraint/index commands. Foreign keys
//! use cascading update and delete behavior. This crate does not provide a
//! versioned migration system; changed schemas should be managed by an
//! application migration workflow.

#[cfg(feature = "connect_args")]
mod connect_args;
mod lang;
#[cfg(feature = "_sqlx_engine")]
mod sqlx_engine;
#[cfg(feature = "tokio_postgres_engine")]
mod tokio_engine;

#[cfg(feature = "connect_args")]
pub use connect_args::*;
pub use lang::*;
#[cfg(feature = "_sqlx_engine")]
pub use sqlx_engine::*;
#[cfg(feature = "tokio_postgres_engine")]
pub use tokio_engine::*;

#[cfg(test)]
mod tests;
