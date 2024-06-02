use serde::{de::DeserializeOwned, Serialize};

use super::event::Event;
use crate::{
    prelude::{Error, NonEmptyVec},
    Unit,
};
use std::fmt::Debug;

pub trait Command<State>: Send + Sync
where
    State: Debug + Clone + Send + Sync + 'static,
{
    type T: Event<State> + Debug + DeserializeOwned + Serialize + 'static;

    /// Validate the command. This function will be called before the `directive`
    /// function. If the command is invalid, an error should be returned.
    fn validate(&self, state: &State) -> Result<Unit, Error>; // TODO: Check if should be async instead

    /// Yield a directive. Essentially, it should return an event or a list of events.
    /// Event order is ensured and enforced by the engine.
    fn directive(&self, state: &State) -> Result<NonEmptyVec<Box<Self::T>>, Error>; //TODO: Check if should be async instead

    /// Return the entity id of the entity.
    ///
    /// Make sure that all commands that are sent to the same entity have the same
    /// entity id.
    ///
    /// The entity id will be validated by the engine, if the entity id is invalid, the
    /// command will be rejected.
    ///
    /// The format of the entity id is up to the user, up to certain constraints:
    ///
    /// - The entity id must be unique.
    /// - The entity id must be a string.
    /// - It must be of the form: `aggregate_type:entity_id`, i.e. `user:atrg-aiuhsn-aiwp`.
    fn entity_id(&self) -> String;

    /// Return the name of the command.
    fn name(&self) -> String {
        std::any::type_name::<Self>().to_string()
    }
}
