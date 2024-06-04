use crate::domain::Error;
use actix::prelude::*;
use std::fmt::Debug;

#[derive(Message)]
#[rtype(result = "Result<State, Error>")]
pub struct GetState<State>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static,
{
    entity_id: String,
    _phantom: std::marker::PhantomData<State>,
}

impl<State> GetState<State>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static,
{
    pub fn new(entity_id: &str) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            entity_id: entity_id.into(),
        }
    }

    pub fn entity_id(&self) -> &str {
        &self.entity_id
    }
}
