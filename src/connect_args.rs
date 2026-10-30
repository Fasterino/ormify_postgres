use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Connection string passed to a selected PostgreSQL driver.
#[derive(Serialize, Deserialize)]
pub struct ConnectArgs(pub String);

impl Into<ConnectArgs> for String {
    fn into(self) -> ConnectArgs {
        ConnectArgs(self)
    }
}
impl Into<ConnectArgs> for &str {
    fn into(self) -> ConnectArgs {
        self.to_owned().into()
    }
}

impl Into<ConnectArgs> for PostgresConnectArgs {
    fn into(self) -> ConnectArgs {
        #[cfg(feature = "tokio_postgres_engine")]
        let args = format!(
            "user={} password='{}' host={} port={} dbname={}",
            self.user,
            self.password.replace('\'', "\\'"),
            self.host,
            self.port,
            self.database
        );
        #[cfg(not(feature = "tokio_postgres_engine"))]
        let args = format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.database
        );

        ConnectArgs(args)
    }
}

impl Deref for ConnectArgs {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Structured PostgreSQL connection parameters used to build [`ConnectArgs`].
#[derive(Serialize, Deserialize)]
pub struct PostgresConnectArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl PostgresConnectArgs {
    pub fn new<I1: Into<String>, I2: Into<String>, I3: Into<String>, I4: Into<String>>(
        host: I1,
        port: u16,
        user: I2,
        password: I3,
        database: I4,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            user: user.into(),
            password: password.into(),
            database: database.into(),
        }
    }
}
