use deadpool::managed::PoolError;
use deadpool_postgres::BuildError;
use rdkafka::error::KafkaError;
use std::fmt::Debug;
use std::{error::Error as StdError, writeln};
use tokio_postgres::Error as PostgresError;

#[derive(thiserror::Error)]
pub enum Error {
    #[error("Actix error: {0}")]
    Actix(#[from] actix::MailboxError),
    #[error("Unable to connect to database.")]
    ConnectionError(#[source] BuildError),
    #[error("Unable to retrieve database connection.")]
    ConnectionRetrievalError(#[source] PoolError<PostgresError>),
    #[error("Decoding error: {0}")]
    Decoding(String),
    #[error("{0}")]
    Error(String),
    #[error("Invalid entity id: {0}")]
    InvalidEntityId(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    #[error("Invalid event: {0}")]
    InvalidEvent(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Kafka error: {0}")]
    Kafka(#[from] KafkaError),
    #[error("System error: {0}")]
    System(#[from] Box<dyn StdError + Send + Sync>),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Command validation error: {0}")]
    Validation(String),
}

impl Error {
    pub fn new(message: &str) -> Self {
        Error::Error(message.to_string())
    }
}

impl From<Error> for KafkaError {
    fn from(error: Error) -> Self {
        match error {
            Error::Kafka(error) => error,
            _ => KafkaError::Subscription(error.to_string()),
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}\n", &self)?;

        let mut current = self.source();

        while let Some(cause) = current {
            writeln!(f, "Caused by:\n\t{}", cause)?;
            current = cause.source();
        }

        Ok(())
    }
}
