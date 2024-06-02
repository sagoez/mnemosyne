use crate::{algebra::Record, domain::Error};
use actix::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

use crate::Unit;

#[derive(Message)]
#[rtype(result = "Result<Unit, Error>")]
pub struct Process<Cmd>
where
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Serialize,
{
    record: Box<Record<Cmd>>,
}

impl<Cmd> Process<Cmd>
where
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Serialize,
{
    pub fn new(record: Record<Cmd>) -> Self {
        Self {
            record: Box::new(record),
        }
    }

    pub fn command(&self) -> &Cmd {
        self.record.message()
    }
}
