use futures::{stream::BoxStream, StreamExt};
use mongodb::{bson::doc, Client, ClientSession, Database};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::{fmt::Debug, sync::Arc};

use crate::{
    algebra::{Meta, Record},
    domain::Error,
    Unit,
};

use super::Adapter;

pub const EVENT_COLLECTION: &str = "events";

pub struct MongoAdapter {
    database: Arc<Database>,
    client: Arc<Client>,
}

impl MongoAdapter {
    #[allow(dead_code)]
    pub async fn connect(connect: MongoAdapterBuilder) -> Self {
        let client = Client::with_uri_str(connect.uri.as_str()).await;

        if let Err(e) = client {
            panic!("Failed to connect to database: {}", e);
        }
        let client = client.unwrap();
        let database = client.database(connect.database.as_str());

        // test connection
        let connection = database
            .run_command(doc! {"ping": 1}, None)
            .await
            .map_err(|e| Error::StorageError(e.to_string()));

        if let Err(e) = connection {
            panic!("Failed to connect to database: {}", e);
        }

        Self {
            database: Arc::new(database),
            client: Arc::new(client),
        }
    }
}

pub struct MongoAdapterBuilder {
    uri: String,
    database: String,
}

#[async_trait::async_trait]
impl Adapter for MongoAdapter {
    async fn read_highest_sequence_number(&self, entity_id: &str) -> Result<Option<u64>, Error> {
        let collection = self.database.collection::<Record<Value>>(EVENT_COLLECTION);

        let filter = doc! {
            "entity_id": entity_id
        };

        let options = mongodb::options::FindOneOptions::builder()
            .sort(doc! {
                "seq_nr": -1
            })
            .build();

        let result = collection
            .find_one(filter, options)
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        match result {
            Some(document) => {
                let sequence_number = document.seq_nr();

                Ok(Some(sequence_number as u64))
            }
            None => Ok(None),
        }
    }

    async fn write<T>(&self, batch: Vec<Record<T>>) -> Result<Unit, Error>
    where
        T: Serialize + Send + DeserializeOwned + Sync,
    {
        let collection = self.database.collection::<Record<T>>(EVENT_COLLECTION);
        let mut transaction: ClientSession = self
            .client
            .start_session(None)
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        let result = collection
            .insert_many_with_session(&batch, None, &mut transaction)
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        if result.inserted_ids.len() == batch.len() {
            transaction
                .commit_transaction()
                .await
                .map_err(|e| Error::StorageError(e.to_string()))?;
            Ok(())
        } else {
            transaction
                .abort_transaction()
                .await
                .map_err(|e| Error::StorageError(e.to_string()))?;
            Err(Error::StorageError(format!(
                "Failed to write all records to database. Expected {} records to be written, but only {} were written",
                batch.len(),
                result.inserted_ids.len()
            )))
        }
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
        let collection = self.database.collection::<Record<T>>(EVENT_COLLECTION);

        let filter = doc! {
            "entity_id": entity_id,
            "seq_nr": {
                "$gte": from_sequence_number as i64,
                "$lte": to_sequence_number as i64
            }
        };

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! {
                "seq_nr": 1
            })
            .limit(max as i64)
            .build();

        let cursor = collection
            .find(filter, options)
            .await
            .map_err(|e| Error::StorageError(e.to_string()))?;

        let records = cursor
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .map(|r| {
                let record = r.map_err(|e| Error::StorageError(e.to_string()))?;
                Ok(record)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Box::pin(futures::stream::iter(records)))
    }
}
