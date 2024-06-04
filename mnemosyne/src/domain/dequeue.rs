use crate::{domain::Error, Unit};
use actix::prelude::*;
use std::fmt::Debug;

#[derive(Message, Debug, Default)]
#[rtype(result = "Result<Unit, Error>")]
pub struct Dequeue;
