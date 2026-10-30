use tokio_postgres::{
    Client, NoTls, Row,
    types::{FromSql, IsNull, ToSql, private::BytesMut, to_sql_checked},
};

use ormify::{
    DefaultLanguageEnum, Driver, Entry, Error, FieldType, LanguageQuery, Supported, UnixTimeStamp,
    Uuid, Value, VarChar,
};

use crate::{ConnectArgs, PostgresSql};

/// A PostgreSQL driver backed by `tokio-postgres`.
///
/// The caller supplies and drives the Tokio runtime. This driver does not
/// create or own a runtime; it owns one PostgreSQL client connection.
/// `tokio-postgres` driver for [`ormify`].
///
/// The application must provide and run a Tokio runtime. The driver owns one
/// PostgreSQL client connection and executes each ORM write transaction
/// atomically. PostgreSQL `NULL` values are bound without a concrete type so
/// they can be used in any nullable column.
pub struct TokioEngine {
    client: Client,
}

impl Supported<PostgresSql> for TokioEngine {
    fn get_language() -> Self::LanguageEnum {
        Default::default()
    }
}

#[derive(Debug)]
struct Null;

impl ToSql for Null {
    fn to_sql(
        &self,
        _ty: &tokio_postgres::types::Type,
        _out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>>
    where
        Self: Sized,
    {
        Ok(IsNull::Yes)
    }

    fn accepts(_ty: &tokio_postgres::types::Type) -> bool
    where
        Self: Sized,
    {
        true
    }

    to_sql_checked!();
}

fn bind(value: Value) -> Box<dyn ToSql + Sync + Send> {
    match value {
        Value::Null => Box::new(Null),
        Value::Bool(value) => Box::new(value),
        Value::I16(value) => Box::new(value),
        Value::I32(value) => Box::new(value),
        Value::I64(value) => Box::new(value),
        Value::F32(value) => Box::new(value),
        Value::F64(value) => Box::new(value),
        Value::Uuid(value) => Box::new(*value),
        Value::UnixTimeStamp(value) => Box::new(*value),
        Value::VarChar(value) => Box::new(value),
        Value::Blob(value) => Box::new(value),
    }
}

fn params(values: Vec<Value>) -> Vec<Box<dyn ToSql + Sync + Send>> {
    values.into_iter().map(bind).collect()
}

fn try_get<'a, T: FromSql<'a>>(row: &'a Row, index: usize) -> ormify::Result<T> {
    row.try_get(index)
        .map_err(|e| Error::Database(format!("PostgreSQL decode failed: {e}")))
}

impl Driver for TokioEngine {
    type LanguageEnum = DefaultLanguageEnum;
    type ConnectArgs = ConnectArgs;

    fn connect(
        args: Self::ConnectArgs,
        _language: Self::LanguageEnum,
    ) -> impl std::future::Future<Output = ormify::Result<Self>> {
        async move {
            let (client, connection) = tokio_postgres::connect(&args.0, NoTls)
                .await
                .map_err(|e| Error::Database(format!("PostgreSQL connection failed: {e}")))?;

            tokio::spawn(async move {
                if let Err(error) = connection.await {
                    eprintln!("PostgreSQL connection error: {error}");
                }
            });

            Ok(Self { client })
        }
    }

    fn query(
        &mut self,
        query: LanguageQuery,
    ) -> impl std::future::Future<Output = ormify::Result<Vec<Entry>>> {
        async move {
            let statement = self
                .client
                .prepare(&query.query)
                .await
                .map_err(|e| Error::Database(format!("PostgreSQL prepare failed: {e}")))?;
            let values = params(query.args);
            let references: Vec<&(dyn ToSql + Sync)> = values
                .iter()
                .map(|value| value.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = self
                .client
                .query(&statement, &references)
                .await
                .map_err(|e| Error::Database(format!("PostgreSQL query failed: {e}")))?;

            rows.into_iter()
                .map(|row| {
                    let mut entry = Entry::new();
                    for (index, column) in row.columns().iter().enumerate() {
                        let type_info = column.type_();

                        let value = match *type_info {
                            tokio_postgres::types::Type::BOOL => {
                                <Option<bool> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::INT2 => {
                                <Option<i16> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::INT4 => {
                                <Option<i32> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::INT8 => {
                                <Option<i64> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::FLOAT4 => {
                                <Option<f32> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::FLOAT8 => {
                                <Option<f64> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            tokio_postgres::types::Type::UUID => {
                                let value: Option<_> = try_get(&row, index)?;
                                FieldType::to_value(value.map(Uuid))
                            }
                            tokio_postgres::types::Type::TIMESTAMP
                            | tokio_postgres::types::Type::TIMESTAMPTZ => {
                                let value: Option<_> = try_get(&row, index)?;
                                FieldType::to_value(value.map(UnixTimeStamp))
                            }
                            tokio_postgres::types::Type::VARCHAR => {
                                let value: Option<_> = try_get(&row, index)?;
                                FieldType::to_value(value.map(VarChar::new_unsized))
                            }
                            tokio_postgres::types::Type::BYTEA => {
                                <Option<Vec<u8>> as FieldType>::to_value(try_get(&row, index)?)
                            }
                            _ => {
                                return Err(Error::Database(format!(
                                    "Unsupported PostgreSQL type: {}",
                                    type_info.name()
                                )));
                            }
                        };
                        entry.insert(column.name().to_owned(), value);
                    }
                    Ok(entry)
                })
                .collect()
        }
    }

    fn execute(
        &mut self,
        commands: Vec<LanguageQuery>,
    ) -> impl std::future::Future<Output = ormify::Result<ormify::ExecutionResponse>> {
        async move {
            let transaction = self.client.transaction().await.map_err(|e| {
                Error::Database(format!("PostgreSQL transaction begin failed: {e}"))
            })?;
            let mut affected = 0;

            for query in commands {
                let statement = transaction
                    .prepare(&query.query)
                    .await
                    .map_err(|e| Error::Database(format!("PostgreSQL prepare failed: {e}")))?;

                let values = params(query.args);
                let references: Vec<&(dyn ToSql + Sync)> = values
                    .iter()
                    .map(|value| value.as_ref() as &(dyn ToSql + Sync))
                    .collect();
                affected += transaction
                    .execute(&statement, &references)
                    .await
                    .map_err(|e| Error::Database(format!("PostgreSQL execute failed: {e}")))?;
            }

            transaction.commit().await.map_err(|e| {
                Error::Database(format!("PostgreSQL transaction commit failed: {e}"))
            })?;
            Ok(affected)
        }
    }
}
