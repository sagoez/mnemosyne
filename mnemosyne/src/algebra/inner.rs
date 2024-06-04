use super::{Event, Record};
use crate::{
    algebra::Command,
    domain::{Error, GetState, Process},
    storage::Adapter,
    Unit,
};
use actix::prelude::*;
use futures::lock::Mutex;
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, sync::Arc};

// The actor is essentially single threaded. So we can use a simple struct
// without any mutexes or other synchronization primitives.
#[derive(Debug, Clone)]
pub(crate) struct Inner<State, Store, Evt>
where
    State: Debug + Send + Sync + 'static + Clone,
    Store: Adapter + Clone + Send + Sync + 'static,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize + 'static,
{
    pub(crate) state: Arc<Mutex<State>>,
    pub(crate) seq_nr: Arc<Mutex<i64>>,
    pub(crate) entity_id: String,
    pub(crate) store: Store,
    _marker: std::marker::PhantomData<Evt>,
}

impl<State, Store, Evt> Inner<State, Store, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize,
{
    pub fn new(entity_id: &str, store: Store) -> Self {
        Self {
            state: Default::default(),
            seq_nr: Default::default(),
            entity_id: entity_id.to_string(),
            store,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<State, Store, Evt> Actor for Inner<State, Store, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize + 'static,
{
    type Context = Context<Self>;

    // TODO: Add logging
    fn started(&mut self, _ctx: &mut Self::Context) {}
}

impl<State, Store, Evt> Supervised for Inner<State, Store, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize + 'static,
{
}

impl<State, Store, Cmd, Evt> Handler<Process<Cmd>> for Inner<State, Store, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static + DeserializeOwned + Default,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Debug + DeserializeOwned + Command<State> + Unpin + Serialize,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize + 'static,
{
    type Result = ResponseFuture<Result<Unit, Error>>;

    fn handle(&mut self, msg: Process<Cmd>, _: &mut Context<Self>) -> Self::Result {
        let state = self.state.clone();
        let seq_nr = self.seq_nr.clone();
        let id = self.entity_id.clone();
        let store = self.store.clone();

        Box::pin(async move {
            let cmd = msg.command();
            let mut state = state.lock().await;
            let mut seq_nr = seq_nr.lock().await;

            // 1. Validate command
            cmd.validate(&state).map_err(|e| {
                Error::Validation(format!(
                    "Command {:?} is not valid for state {:?}: {}",
                    cmd, state, e
                ))
            })?;

            // 2. If valid, yield events
            let events = cmd.directive(&state)?;

            let mut new_state = state.clone();
            // // 3. Apply events to state and emit effects
            for event in events.iter() {
                new_state = event.apply(&new_state)?;
                event.effects(&state, &new_state);
            }

            *state = new_state;

            let records = events
                .into_iter()
                .map(|event| {
                    *seq_nr += 1;
                    Record::event(id.clone(), *seq_nr, event, chrono::Utc::now())
                })
                .collect::<Vec<_>>();

            // 4. Save events to storage, if this fails it is non-recoverable, so we panic
            store.write(records).await
            // 5. Publish events to Kafka (this should be done in a separate actor)
        })
    }
}

impl<State, Store, Evt> Handler<GetState<State>> for Inner<State, Store, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Evt: Debug + DeserializeOwned + Event<State> + Unpin + Serialize + 'static,
{
    type Result = ResponseFuture<Result<State, Error>>;

    fn handle(&mut self, _: GetState<State>, _: &mut Context<Self>) -> Self::Result {
        let state = self.state.clone();

        Box::pin(async move { Ok(state.lock().await.clone()) })
    }
}
