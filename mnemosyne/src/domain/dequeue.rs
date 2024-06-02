use actix::prelude::*;
use std::fmt::Debug;

use crate::{domain::Error, Unit};

#[derive(Message, Debug)]
#[rtype(result = "Result<Unit, Error>")]
pub struct Dequeue;

impl Dequeue {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Dequeue {
    fn default() -> Self {
        Self::new()
    }
}
