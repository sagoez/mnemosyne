mod memory;
mod mongo;
mod postgres;

pub use memory::*;
#[cfg(feature = "mongo")]
pub use mongo::*;
#[cfg(feature = "postgres")]
pub use postgres::*;

use crate::Unit;
use crate::{algebra::Record, domain::Error};
use futures::stream::BoxStream;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

#[async_trait::async_trait]
pub trait Adapter {
    /// Read the highest sequence number for a given entity id from the database
    ///
    /// # Arguments
    /// * `entity_id` - The entity id to read the highest sequence number for from the database
    ///
    /// # Examples
    /// ```rust,ignore
    /// let db = MemoryAdapter::new();
    /// let highest_sequence_nr = db.read_highest_sequence_nr("entity_id");
    /// ```
    /// # Returns
    /// The highest sequence number for the given entity id or None if no sequence number was found
    /// for the given entity id.
    async fn read_highest_sequence_number(&self, entity_id: &str) -> Result<Option<u64>, Error>;
    /// Write a batch of messages atomically to the database
    ///
    /// # Arguments
    /// * `batch` - The atomic batch to write to the database
    ///
    /// # Returns
    /// A Result with Ok(()) if the message was written successfully or `Error` if the message
    async fn write<T>(&self, batch: Vec<Record<T>>) -> Result<Unit, Error>
    where
        T: Serialize + Send + DeserializeOwned + Sync;
    /// Replay messages from the database for a given entity id and sequence number
    /// range.
    ///
    /// # Arguments
    /// * `entity_id` - The entity id to replay messages for
    /// * `from_sequence_number` - The sequence number to start replaying messages from
    /// * `to_sequence_number` - The sequence number to stop replaying messages at
    /// * `max` - The maximum number of messages to replay
    ///
    /// # Returns
    /// A stream of messages replayed from the database for the given entity id and sequence number range.
    async fn replay<T>(
        &self,
        entity_id: &str,
        from_sequence_number: u64,
        to_sequence_number: u64,
        max: u64,
    ) -> Result<BoxStream<'static, Record<T>>, Error>
    where
        T: DeserializeOwned + Send + Debug + 'static + Serialize + Sync;
}
