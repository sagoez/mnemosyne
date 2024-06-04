use std::fmt::Debug;

pub trait Event<State>: Sync + Send
where
    State: Debug + Clone + Send + Sync + 'static,
{
    /// Applies the event to the state and returns the updated state.
    ///
    /// This method should be a pure function, ensuring determinism and idempotence.
    fn apply(&self, state: &State) -> Option<State>;
}
