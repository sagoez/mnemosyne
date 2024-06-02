use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait Meta {
    /// Returns the entity id where the record belongs to
    fn entity_id(&self) -> &str;
    /// Returns the sequence number of the record, applicable only to events
    fn seq_nr(&self) -> i64;
    /// Returns the timestamp of the record
    fn timestamp(&self) -> DateTime<Utc>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record<T> {
    entity_id: String,
    seq_nr: i64,
    timestamp: DateTime<Utc>,
    message: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
}

impl<T> Record<T> {
    pub fn event(entity_id: String, seq_nr: i64, message: T, timestamp: DateTime<Utc>) -> Self {
        Self {
            entity_id,
            seq_nr,
            message,
            timestamp,
            r#type: None,
        }
    }

    // TODO: Restrict this to commands only
    pub fn command(
        entity_id: &str,
        message: T,
        timestamp: DateTime<Utc>,
        command: String,
        seq_nr: i64,
    ) -> Self
    where
        T: Serialize,
    {
        Self {
            entity_id: entity_id.to_owned(),
            seq_nr,
            message,
            timestamp,
            r#type: Some(command),
        }
    }

    pub fn message(&self) -> &T {
        &self.message
    }

    pub fn into_message(self) -> T {
        self.message
    }

    pub fn r#type(&self) -> Option<&str> {
        self.r#type.as_deref()
    }
}

impl<T> Meta for Record<T>
where
    T: Serialize + DeserializeOwned,
{
    fn entity_id(&self) -> &str {
        &self.entity_id
    }

    fn seq_nr(&self) -> i64 {
        self.seq_nr
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}
