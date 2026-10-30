#[cfg(not(feature = "tokio_sqlx_engine"))]
use futures::TryStreamExt;
#[cfg(feature = "tokio_sqlx_engine")]
use tokio_stream::StreamExt;

use ormify::{
    DefaultLanguageEnum, Driver, Entry, Error, FieldType, LanguageQuery, Supported, UnixTimeStamp,
    Uuid, Value, VarChar,
};
use sqlx::{Column, Row, TypeInfo};

use crate::{ConnectArgs, PostgresSql};

/// SQLx PostgreSQL driver.
///
/// Select `tokio_sqlx_engine` or `async_std_sqlx_engine` to choose the SQLx
/// runtime integration. The driver uses a PostgreSQL pool and executes ORM
/// writes in a transaction.
pub struct SqlxEngine {
    pool: sqlx::PgPool,
}

impl Supported<PostgresSql> for SqlxEngine {
    fn get_language() -> Self::LanguageEnum {
        Default::default()
    }
}

fn bind(
    req: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
    arg: Value,
) -> sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match arg {
        Value::Null => req.bind(None::<i32>),
        Value::Bool(x) => req.bind(x),
        Value::I16(x) => req.bind(x),
        Value::I32(x) => req.bind(x),
        Value::I64(x) => req.bind(x),
        Value::F32(x) => req.bind(x),
        Value::F64(x) => req.bind(x),
        Value::Uuid(x) => req.bind(*x),
        Value::UnixTimeStamp(x) => req.bind(*x),
        Value::VarChar(x) => req.bind(x),
        Value::Blob(x) => req.bind(x),
    }
}
fn try_get<'r, T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>>(
    row: &'r sqlx::postgres::PgRow,
    index: usize,
) -> ormify::Result<T> {
    row.try_get(index)
        .map_err(|e| Error::Database(format!("SQLx decode failed: {e}")))
}

impl Driver for SqlxEngine {
    type LanguageEnum = DefaultLanguageEnum;

    type ConnectArgs = ConnectArgs;

    fn connect(
        args: Self::ConnectArgs,
        _language: Self::LanguageEnum,
    ) -> impl std::future::Future<Output = ormify::Result<Self>> {
        async move {
            let pool = sqlx::Pool::connect(&args.as_str())
                .await
                .map_err(|e| Error::Database(format!("SQLx connection failed: {e}")))?;

            Ok(Self { pool })
        }
    }

    fn query(
        &mut self,
        query: LanguageQuery,
    ) -> impl std::future::Future<Output = ormify::Result<Vec<Entry>>> {
        async move {
            let mut req = sqlx::query(sqlx::AssertSqlSafe(query.query));

            for arg in query.args {
                req = bind(req, arg);
            }

            let mut rows = req.fetch(&self.pool);

            let mut entries = Vec::new();

            loop {
                let row = match rows
                    .try_next()
                    .await
                    .map_err(|e| Error::Database(format!("SQLx fetch failed: {e}")))?
                {
                    Some(x) => x,
                    None => break,
                };

                let mut entry = Entry::new();

                for (index, column) in row.columns().iter().enumerate() {
                    let type_info = column.type_info();

                    // Извлекаем PgDataType
                    let value = match TypeInfo::name(type_info) {
                        "BOOL" => <Option<bool> as FieldType>::to_value(try_get(&row, index)?),
                        "INT2" => <Option<i16> as FieldType>::to_value(try_get(&row, index)?),
                        "INT4" => <Option<i32> as FieldType>::to_value(try_get(&row, index)?),
                        "INT8" => <Option<i64> as FieldType>::to_value(try_get(&row, index)?),
                        "FLOAT4" => <Option<i32> as FieldType>::to_value(try_get(&row, index)?),
                        "FLOAT8" => <Option<i64> as FieldType>::to_value(try_get(&row, index)?),
                        "UUID" => {
                            FieldType::to_value(Option::<_>::map(try_get(&row, index)?, Uuid))
                        }
                        "TIMESTAMP" | "TIMESTAMPTZ" => FieldType::to_value(Option::<_>::map(
                            try_get(&row, index)?,
                            UnixTimeStamp,
                        )),
                        "VARCHAR" => FieldType::to_value(Option::<_>::map(
                            try_get(&row, index)?,
                            VarChar::new_unsized,
                        )),
                        "BYTEA" => <Option<Vec<u8>> as FieldType>::to_value(try_get(&row, index)?),
                        _ => {
                            return Err(Error::Database(format!(
                                "Unsupported SQLx type: {}",
                                type_info.name()
                            )));
                        }
                    };

                    entry.insert(column.name().to_owned(), value);
                }

                entries.push(entry);
            }

            Ok(entries)
        }
    }

    fn execute(
        &mut self,
        commands: Vec<LanguageQuery>,
    ) -> impl std::future::Future<Output = ormify::Result<ormify::ExecutionResponse>> {
        async move {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| Error::Database(format!("SQLx transaction begin failed: {e}")))?;

            let mut rows_affected = 0;

            for execution in commands {
                let mut req = sqlx::query(sqlx::AssertSqlSafe(execution.query));

                for arg in execution.args {
                    req = bind(req, arg);
                }

                rows_affected += req
                    .execute(tx.as_mut())
                    .await
                    .map_err(|e| Error::Database(format!("SQLx transaction commit failed: {e}")))?
                    .rows_affected();
            }

            tx.commit()
                .await
                .map_err(|e| Error::Database(format!("SQLx transaction commit failed: {e}")))?;

            Ok(rows_affected)
        }
    }
}
