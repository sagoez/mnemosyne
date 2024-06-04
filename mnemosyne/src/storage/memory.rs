use super::{Adapter, Record};
use crate::{algebra::Meta, domain::Error, Unit};
use futures::stream::BoxStream;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub struct MemoryAdapter {
    storage: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryAdapter {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MemoryAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Adapter for MemoryAdapter {
    async fn read_highest_sequence_number(&self, entity_id: &str) -> Result<Option<u64>, Error> {
        let entity_id_in_bytes = entity_id.as_bytes();
        // A max key is created by appending 8 bytes of u8::MAX to the entity id in bytes
        let mut max_key = Vec::with_capacity(entity_id_in_bytes.len() + 8);

        max_key.extend_from_slice(entity_id_in_bytes);
        max_key.extend_from_slice(&[u8::MAX; 8]);

        let locked = self
            .storage
            .lock()
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to read storage: {}", e)))?;

        Ok(locked
            .iter()
            .filter_map(|(k, _)| {
                if k.len() == entity_id_in_bytes.len() + 8 || k.starts_with(entity_id_in_bytes) {
                    // For the keys that matched the entity id, we extract the sequence number
                    // assuming that the sequence number is stored in the last 8 bytes of the key
                    // in a big endian format
                    let sequence_nr_bytes = &k[k.len() - 8..];
                    sequence_nr_bytes.try_into().ok().map(u64::from_be_bytes)
                } else {
                    None
                }
            })
            .max())
    }

    async fn write<T>(&self, batch: Vec<Record<T>>) -> Result<Unit, Error>
    where
        T: Serialize + Send + DeserializeOwned + Sync,
    {
        fn mk_key(entity_id: &str, sequence_nr: i64) -> Vec<u8> {
            let mut key = Vec::with_capacity(entity_id.len() + 8);
            key.extend_from_slice(entity_id.as_bytes());
            key.extend_from_slice(&sequence_nr.to_be_bytes());
            key
        }

        let mut locked = self
            .storage
            .lock()
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to write storage: {}", e)))?;

        batch.into_iter().try_for_each(|value| {
            let entity_id = value.entity_id();
            let sequence_nr = value.seq_nr();
            let key = mk_key(entity_id, sequence_nr);
            // TODO: Retry on failure and if the error persists, then save the batch somewhere else
            // such that the data is not lost
            let serialized = bincode::serialize(&value).map_err(|e| {
                Error::InvalidConfiguration(format!("Failed to serialize value: {}", e))
            })?;
            locked.insert(key, serialized);
            Ok(())
        })
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
        fn seq_nr_from_key(key: &[u8]) -> Option<i64> {
            let length = key.len();
            let seq_nr_part: [u8; 8] = key[length - 8..].try_into().ok()?;
            Some(i64::from_be_bytes(seq_nr_part))
        }

        let locked = self
            .storage
            .lock()
            .map_err(|e| Error::InvalidConfiguration(format!("Failed to read storage: {}", e)))?;

        let entity_id_in_bytes = entity_id.as_bytes();

        let from_key = {
            let mut key = Vec::with_capacity(entity_id_in_bytes.len() + 8);
            key.extend_from_slice(entity_id_in_bytes);
            key.extend_from_slice(&from_sequence_number.to_be_bytes());
            key
        };

        let to_key = {
            let mut key = Vec::with_capacity(entity_id_in_bytes.len() + 8);
            key.extend_from_slice(entity_id_in_bytes);
            key.extend_from_slice(&to_sequence_number.to_be_bytes());
            key
        };

        let events: Vec<Record<T>> = locked
            .iter()
            .filter_map(|(k, v)| {
                if k.starts_with(entity_id_in_bytes)
                    && k.as_slice() >= from_key.as_slice()
                    && k.as_slice() <= to_key.as_slice()
                {
                    let timestamp = chrono::Utc::now();
                    seq_nr_from_key(k).and_then(|seq_nr| {
                        bincode::deserialize::<T>(v)
                            .ok()
                            .map(|msg| Record::event(entity_id.to_string(), seq_nr, msg, timestamp))
                    })
                } else {
                    None
                }
            })
            .take(max as usize)
            .collect();

        Ok(Box::pin(futures::stream::iter(events)))
    }
}
