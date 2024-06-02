use crate::{prelude::Error, Unit};
use std::fmt::Debug;

pub trait Event<State>: Sync + Send
where
    State: Debug + Clone + Send + Sync + 'static,
{
    /// Applies the event to the state and returns the updated state.
    ///
    /// This method should be a pure function, ensuring determinism and idempotence.
    fn apply(&self, state: &State) -> Result<State, Error>;

    /// Performs side effects based on the application of the event.
    ///
    /// This method is not pure and may trigger side effects but does not modify the state directly.
    fn effects(&self, before: &State, after: &State) -> Unit; //TODO: Check if should be async instead

    // /// Schedule a command to be executed after the event is applied.
    // ///
    // /// This method is not pure and may trigger side effects but does not modify the state directly.
    // fn schedule(&self, before: &State, after: &State) -> Unit; //TODO: Check if should be async instead
}
