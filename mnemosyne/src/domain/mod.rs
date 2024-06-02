mod dequeue;
mod enqueue;
mod error;
mod process;
mod state;

use std::{slice::Iter, vec::IntoIter};

pub(crate) use dequeue::*;
pub(crate) use enqueue::*;
pub use error::*;
pub(crate) use process::*;
pub(crate) use state::*;

use serde::{Deserialize, Serialize};

// Make all this configurable
pub const STATE_TOPIC: &str = "state";
pub const EVENT_TOPIC: &str = "events";
pub const COMMAND_TOPIC: &str = "commands";

pub const BATCH_BACKPRESSURE: u64 = 2;
pub const CHUNK_BACKPRESSURE: u64 = 2;

pub const CHUNK_SIZE: u64 = 100;
pub const GROUP_ID: &str = "mnemosyne";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    /// Create a new NonEmptyVec. If the vector is empty, an error is returned.
    pub fn new(vec: Vec<T>) -> Result<Self, Error> {
        if vec.is_empty() {
            Err(Error::InvalidCommand("Empty vector".to_string()))
        } else {
            Ok(Self(vec))
        }
    }

    /// Create a new NonEmptyVec with one element.
    pub fn one(value: T) -> Self {
        Self(vec![value])
    }

    /// Return the underlying vector.
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Returns an iterator over the vector.
    ///
    /// The iterator yields all items from start to end.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mnemosyne::prelude::NonEmptyVec;
    ///
    /// let x = NonEmptyVec::new(vec![1, 2, 4]).unwrap();
    /// let mut iterator = x.iter();
    ///
    /// assert_eq!(iterator.next(), Some(&1));
    /// assert_eq!(iterator.next(), Some(&2));
    /// assert_eq!(iterator.next(), Some(&4));
    /// assert_eq!(iterator.next(), None);
    /// ```
    pub fn iter(&self) -> Iter<T> {
        self.0.iter()
    }

    pub fn into_iter(self) -> IntoIter<T> {
        self.0.into_iter()
    }
}
