use super::Adapter;
use crate::{algebra::Record, domain::Error, Unit};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use deadpool_postgres::{Manager, Pool};
use futures::{stream::BoxStream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use tokio_postgres::Config;

#[derive(Debug, Clone)]
pub struct PostgresAdapter {
    pool: Pool,
}

impl PostgresAdapter {
    #[allow(dead_code)]
    pub async fn connect(connect: PostgresAdapterBuilder) -> Self {
        let mut config = Config::new();
        config.host(&connect.host);
        config.user(&connect.user);
        config.dbname(&connect.database);
        config.port(connect.port);
        config.connect_timeout(std::time::Duration::from_secs(connect.timeout));
        config.password(&connect.password);
        config.ssl_mode(connect.ssl.to_ssl());

        let manager = Manager::new(config, tokio_postgres::NoTls);
        let pool = Pool::builder(manager) // This is already an Arc, so no need to wrap it
            .build()
            .map_err(Error::ConnectionError);

        if let Err(e) = pool {
            panic!("Failed to connect to database: {}", e);
        }

        // test connection
        let pool = pool.unwrap();
        let connection = pool.get().await.map_err(Error::ConnectionRetrievalError);

        if let Err(e) = connection {
            panic!("Failed to connect to database: {}", e);
        }

        Self { pool }
    }
}

pub struct PostgresAdapterBuilder {
    host: String,
    user: String,
    port: u16,
    password: String,
    database: String,
    timeout: u64,
    ssl: SslMode,
}

impl PostgresAdapterBuilder {
    pub fn new(
        host: &str,
        user: &str,
        port: u16,
        password: &str,
        database: &str,
        timeout: u64,
        ssl: SslMode,
    ) -> Self {
        Self {
            host: host.into(),
            user: user.into(),
            password: password.into(),
            port,
            database: database.into(),
            timeout,
            ssl,
        }
    }
}

pub struct SslMode(bool);

impl SslMode {
    pub fn new(ssl: bool) -> Self {
        Self(ssl)
    }

    /// Returns the SSL mode for the database.
    pub fn to_ssl(&self) -> tokio_postgres::config::SslMode {
        match self.0 {
            true => tokio_postgres::config::SslMode::Require,
            false => tokio_postgres::config::SslMode::Disable,
        }
    }
}

impl Adapter for PostgresAdapter {
    async fn read_highest_sequence_number(&self, entity_id: &str) -> Result<Option<u64>, Error> {
        let connection = self
            .pool
            .get()
            .await
            .map_err(Error::ConnectionRetrievalError)?;

        let number = connection
            .query_opt(
                "SELECT MAX(seq_nr) FROM events WHERE entity_id = $1",
                &[&entity_id],
            )
            .await
            .map_err(|e| Error::StorageError(e.to_string()))
            .map(|row| row.map(|row| row.try_get::<_, i64>("seq_nr").unwrap() as u64))?;

        Ok(number)
    }

    async fn write<T>(&self, batch: Vec<Record<&T>>) -> Result<Unit, Error>
    where
        T: Serialize + Send + DeserializeOwned + Sync,
    {
        let mut connection = self
            .pool
            .get()
            .await
            .map_err(Error::ConnectionRetrievalError)?;

        let transaction = connection
            .transaction()
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        for record in batch {
            let payload = serde_json::to_value(record.message()).unwrap();
            let timestamp = record.timestamp();
            let entity_id = record.entity_id();
            let seq_nr = record.seq_nr();
            let uuid = uuid::Uuid::new_v4();

            let stmt = transaction
                .prepare(
                    "INSERT INTO events (id, entity_id, seq_nr, timestamp, payload) VALUES ($1, $2, $3, $4, $5)",
                )
                .await
                .map_err(|e| Error::StorageError(e.to_string()))?;

            transaction
                .execute(&stmt, &[&uuid, &entity_id, &seq_nr, &timestamp, &payload])
                .await
                .map_err(|e| Error::StorageError(e.to_string()))?;
        }

        transaction
            .commit()
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        Ok(())
    }

    async fn replay<T>(
        &self,
        entity_id: &str,
        from_sequence_number: u64,
        to_sequence_number: u64,
        max: u64,
    ) -> Result<BoxStream<'static, Record<T>>, Error>
    where
        T: Send + DeserializeOwned + Debug + 'static + Serialize + Sync,
    {
        let connection = self
            .pool
            .get()
            .await
            .map_err(Error::ConnectionRetrievalError)?;

        let from_sequence_number = from_sequence_number.to_string();
        let to_sequence_number = to_sequence_number.to_string();
        let max = max.to_string();

        let row_stream = connection
            .query_raw(
                "SELECT payload FROM events WHERE entity_id = $1 AND seq_nr >= $2 AND seq_nr <= $3 ORDER BY seq_nr ASC LIMIT $4",
                &[&entity_id, &from_sequence_number.as_str(), &to_sequence_number.as_str(), &max.as_str()],
            )
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        let stream = row_stream
            .map(|row| match row {
                Ok(row) => {
                    let entity_id = row
                        .try_get::<_, String>("entity_id")
                        .map_err(|e| Error::StorageError(e.to_string()))?;
                    let payload = row.try_get::<_, Value>("payload").map_err(|e| {
                        Error::StorageError(format!("Failed to get payload: {}", e))
                    })?;
                    let payload = serde_json::from_value::<T>(payload).map_err(|e| {
                        Error::StorageError(format!("Failed to deserialize: {}", e))
                    })?;
                    let timestamp = row.try_get::<_, DateTime<Utc>>("timestamp").map_err(|e| {
                        Error::StorageError(format!("Failed to get timestamp: {}", e))
                    })?;
                    let seq_nr = row
                        .try_get::<_, i64>("seq_nr")
                        .map_err(|e| Error::StorageError(format!("Failed to get seq_nr: {}", e)))?
                        as u64;

                    Ok(Record::event(
                        entity_id.to_string(),
                        seq_nr as i64,
                        payload,
                        timestamp,
                    ))
                }
                Err(e) => {
                    println!("Error: {}", e);
                    Err(Error::StorageError(e.to_string()))
                }
            })
            .filter_map(|row| async move { row.ok() })
            .boxed();

        Ok(stream)
    }
}
