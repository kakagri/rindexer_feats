use crate::EthereumSqlTypeWrapper;
use bb8::{Pool, RunError};
use bb8_clickhouse::ClickHouseConnectionManager;
use dotenv::dotenv;
use std::env;
use tracing::info;

pub struct ClickhouseConnection {
    pub url: String,
    pub user: String,
    pub password: String,
    pub db: String,
}

pub fn clickhouse_connection() -> Result<ClickhouseConnection, env::VarError> {
    dotenv().ok();

    let connection = ClickhouseConnection {
        url: env::var("CLICKHOUSE_URL")?,
        user: env::var("CLICKHOUSE_USER")?,
        password: env::var("CLICKHOUSE_PASSWORD")?,
        db: env::var("CLICKHOUSE_DB")?,
    };

    Ok(connection)
}

#[derive(thiserror::Error, Debug)]
pub enum ClickhouseConnectionError {
    #[error("The clickhouse env vars are wrong please check your environment: {0}")]
    ClickhouseConnectionConfigWrong(#[from] env::VarError),

    #[error("Could not connect to clickhouse database: {0}")]
    ClickhouseNetworkError(#[from] clickhouse::error::Error),

    #[error("Connection pool error: {0}")]
    ConnectionPoolRuntimeError(#[from] RunError<clickhouse::error::Error>),
}

#[derive(thiserror::Error, Debug)]
pub enum ClickhouseError {
    #[error("ClickhouseError: {0}")]
    ClickhouseError(#[from] clickhouse::error::Error),

    #[error("Connection pool error: {0}")]
    ConnectionPoolError(#[from] RunError<clickhouse::error::Error>),
}

pub struct ClickhouseClient {
    pool: Pool<ClickHouseConnectionManager>,
    pub(crate) database_name: String,
}

impl ClickhouseClient {
    /// Creates a new ClickHouse client with connection pooling using environment variables.
    ///
    /// Expects the following environment variables:
    /// - CLICKHOUSE_URL: The ClickHouse server URL
    /// - CLICKHOUSE_USER: Database user
    /// - CLICKHOUSE_PASSWORD: Database password
    /// - CLICKHOUSE_DB: Database name
    pub async fn new() -> Result<Self, ClickhouseConnectionError> {
        let connection = clickhouse_connection()?;
        let database_name = connection.db.clone();

        let manager = ClickHouseConnectionManager::new(connection.url)
            .with_database(connection.db)
            .with_user(connection.user)
            .with_password(connection.password);

        let pool = Pool::builder().build(manager).await?;

        // Test the connection
        {
            let client = pool.get().await?;
            client.query("SELECT 1").execute().await?;
        }
        info!("Clickhouse client connected successfully!");

        Ok(ClickhouseClient { pool, database_name })
    }

    /// Creates a ClickHouse client from an existing connection pool.
    ///
    /// This allows for custom pool configuration by building the pool externally.
    pub async fn from_connection(
        pool: Pool<ClickHouseConnectionManager>,
        database_name: String,
    ) -> Result<Self, ClickhouseConnectionError> {
        Ok(Self { pool, database_name })
    }

    /// Returns the name of the database this client is connected to.
    pub fn get_database_name(&self) -> &str {
        &self.database_name
    }

    /// Executes a SQL query without returning any results.
    ///
    /// Useful for DDL statements (CREATE, DROP, ALTER) or DML statements where
    /// you don't need to retrieve results.
    pub async fn execute(&self, sql: &str) -> Result<(), ClickhouseError> {
        let client = self.pool.get().await?;
        client.query(sql).execute().await?;

        Ok(())
    }

    /// Executes multiple SQL statements separated by semicolons.
    ///
    /// Each statement is executed sequentially. Empty statements are skipped.
    pub async fn execute_batch(&self, sql: &str) -> Result<(), ClickhouseError> {
        let statements: Vec<&str> =
            sql.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();

        for statement in statements {
            self.execute(statement).await?;
        }

        Ok(())
    }

    /// Internal method for bulk inserting data using VALUES clause.
    ///
    /// Made pub(crate) to allow crate-internal access while keeping insert_bulk as the primary API.
    pub(crate) async fn bulk_insert_via_query(
        &self,
        table_name: &str,
        column_names: &[String],
        bulk_data: &[Vec<EthereumSqlTypeWrapper>],
    ) -> Result<u64, ClickhouseError> {
        let values = bulk_data
            .iter()
            .map(|row| row.iter().map(|v| v.to_clickhouse_value()).collect::<Vec<_>>().join(", "))
            .map(|row| format!("({})", row))
            .collect::<Vec<_>>()
            .join(", ");

        self.execute(&format!(
            "INSERT INTO {} ({}) VALUES {}",
            table_name,
            column_names.join(", "),
            values
        ))
        .await?;

        Ok(bulk_data.len() as u64)
    }

    /// Performs a bulk insert of data into the specified table.
    ///
    /// This method constructs and executes an INSERT statement with multiple VALUE rows.
    /// For optimal performance with large datasets, consider batching your inserts.
    pub async fn insert_bulk(
        &self,
        table_name: &str,
        column_names: &[String],
        bulk_data: &[Vec<EthereumSqlTypeWrapper>],
    ) -> Result<u64, ClickhouseError> {
        self.bulk_insert_via_query(table_name, column_names, bulk_data).await
    }

    /// Returns a raw pooled connection to the ClickHouse database.
    ///
    /// This provides direct access to the underlying clickhouse::Client for operations
    /// that require more control or are not exposed through the ClickhouseClient API.
    pub async fn raw_connection(&self) -> Result<bb8::PooledConnection<'_, ClickHouseConnectionManager>, ClickhouseError> {
        let client = self.pool.get().await?;
        Ok(client)
    }
}
