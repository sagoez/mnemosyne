use super::{Event, Init};
use crate::{
    algebra::Command,
    domain::{Enqueue, Error, GetState},
    storage::Adapter,
    Unit,
};
use actix::{Addr, Supervisor};
use rdkafka::ClientConfig;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

pub struct Engine<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + 'static + DeserializeOwned + Default,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
{
    addr: Addr<Init<State, Store, Cmd, Evt>>,
}

impl<State, Store, Cmd, Evt> Engine<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + 'static + DeserializeOwned + Default,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    pub async fn enqueue(&mut self, command: Cmd) -> Result<Unit, Error> {
        self.addr
            .send(Enqueue::from_command(command))
            .await
            .map_err(Error::Actix)?
    }

    /// Return the current state of the domain. This state is always guaranteed to be the latest
    /// state of the domain. Even if the actor has just been created, or restarted.
    pub async fn state(&mut self, entity_id: &str) -> Result<State, Error> {
        self.addr
            .send(GetState::new(entity_id))
            .await
            .map_err(Error::Actix)?
    }

    pub async fn start(
        configuration: ClientConfig,
        store: Store,
    ) -> Result<Engine<State, Store, Cmd, Evt>, Error> {
        let addr = Init::empty(configuration, store).await?;
        let supervisor = Supervisor::start(|_| addr);

        Ok(Self { addr: supervisor })
    }
}
