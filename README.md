<p align="center">
  <img src="./icon.svg" alt="ORMIFY logo" width="128" height="128">
</p>

<h1 align="center">ORMIFY_POSTGRES</h1>

<p align="center">
  <a href="https://crates.io/crates/ormify_postgres"><img src="https://img.shields.io/crates/v/ormify_postgres?style=flat&logo=rust" alt="crates.io"></a>
  <a href="https://docs.rs/ormify_postgres"><img src="https://img.shields.io/docsrs/ormify_postgres?style=flat&logo=docs.rs" alt="docs.rs"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat&logo=github" alt="license"></a>
  <a href="https://github.com/Fasterino/ormify_postgres">
    <img src="https://img.shields.io/github/stars/Fasterino/ormify_postgres?style=social" alt="Star on GitHub">
  </a>
</p>

PostgreSQL dialect and drivers for [`ormify`](https://github.com/Fasterino/ormify), a backend-agnostic Rust ORM.

## Features

- PostgreSQL SQL generation with quoted identifiers and `$n` parameter placeholders.
- PostgreSQL schemas are selected through `#[db(group = "...")]`.
- Entity initialization creates schemas, tables, constraints, and indexes.
- Foreign keys default to `ON DELETE CASCADE ON UPDATE CASCADE`.
- Optional `tokio-postgres` and SQLx drivers.
- Engine-owned recursive joins and transactional writes are provided by `ormify`.

## Installation

Choose one driver/runtime combination:

```toml
[dependencies]
ormify = { version = "1.0.0", features = ["full"] }
ormify_postgres = { version = "1.0.1", features = ["tokio_postgres_engine"] }
```

Or select a SQLx runtime:

```toml
ormify = { version = "1.0.0", features = ["full"] }
ormify_postgres = { version = "1.0.1", features = ["tokio_sqlx_engine"] }
# alternatively use `async_std_sqlx_engine` with an async-std SQLx runtime
```

### Feature flags

| Feature                 | Meaning                                                                                           |
| ----------------------- | ------------------------------------------------------------------------------------------------- |
| `tokio_postgres_engine` | Enables `TokioEngine`; caller must run a Tokio runtime.                                           |
| `tokio_sqlx_engine`     | Enables `SqlxEngine` using SQLx's Tokio runtime.                                                  |
| `async_std_sqlx_engine` | Enables `SqlxEngine` using SQLx's async-std runtime.                                              |
| `connect_args`          | Exposes `ConnectArgs` and `PostgresConnectArgs`. This feature is included in all engine features. |

The crate has no default driver feature; enable the one you need. Avoid enabling multiple SQLx runtime features in the same build.

## Quick start: Tokio + tokio-postgres

```rust
use ormify::{Entity, Observer, Ormify, Transform as _, UnixTimeStamp, Uuid, VarChar};
use ormify_postgres::{PostgresConnectArgs, PostgresSql, TokioEngine};

#[derive(Entity, Debug)]
#[db(group = "app")]
struct User {
    #[db(pk)]
    id: Uuid,
    #[db(unique)]
    nickname: VarChar<32>,
    created_at: UnixTimeStamp,
}

#[tokio::main]
async fn main() -> ormify::Result<()> {
    let mut db = Ormify::<PostgresSql, TokioEngine>::connect(PostgresConnectArgs::new(
        "localhost", // host
        5432,        // port
        "postgres",  // user
        "postgres",  // password
        "postgres",  // database
    ))
    .await?;

    db.execute(vec![User::init()]).await?;

    let user = User::builder().id(Uuid::new()).nickname("ada").build();
    db.execute(vec![user.insert()]).await?;

    let res = db.query(User::select()).await?;
    let users = Vec::<Observer<User>>::transform(res)?;
    println!("{users:?}");
    Ok(())
}
```

`TokioEngine` creates a single `tokio-postgres` client connection and spawns its connection driver on the current Tokio runtime. It does not create or own a runtime.

Instead of `PostgresConnectArgs`, both engines also accept a driver-specific connection string, for example, for TokioEngine, this is a string like `host=localhost user=postgres password=postgres dbname=postgres`

## SQLx

Use `SqlxEngine` with `tokio_sqlx_engine` or `async_std_sqlx_engine`. The selected runtime must be compatible with the executor used by the application. You can connect with a PostgreSQL URL, for example `postgres://user:password@localhost/database`.

## Schema initialization and foreign keys

A scheme group is a PostgreSQL schema. Initialization emits `CREATE SCHEMA IF NOT EXISTS`, then creates the qualified table, followed by separate index statements. Foreign-key constraints reference the target entity's qualified schema/table and currently use cascading update/delete behavior.

Submit related `Entity::init()` executions together; `Ormify` resolves the included foreign-key dependencies and initializes referenced entities first. For cycles, use a migration workflow that creates tables before adding foreign keys.

Initialization is DDL execution, not a versioned migration system. It does not alter existing tables to match changed entity definitions.

## PostgreSQL mappings and limits

Core mappings include booleans, signed integers, floats, UUID, timestamps, strings, and byte arrays. PostgreSQL types not recognized by the selected driver return a database error. `Option<T>` maps to nullable database values; `NULL` is bound without assuming a concrete PostgreSQL type.

The SQL language uses bound parameters for condition and write values. `Condition::Raw` is an escape hatch and should only contain trusted SQL.

## License

[MIT.](./LICENSE)
