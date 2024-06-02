use actix::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

use crate::{
    algebra::{Command, Event},
    domain::Error,
    Unit,
};

#[derive(Debug, Clone)]
pub enum EnqueueType<Cmd, Evt, State>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State>,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    Command(Cmd),
    Event(Evt),
    State(State),
}

#[derive(Message, Debug)]
#[rtype(result = "Result<Unit, Error>")]
pub struct Enqueue<Cmd, Evt, State>
where
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State>,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
{
    element: EnqueueType<Cmd, Evt, State>,
    _marker: std::marker::PhantomData<State>,
}

impl<Cmd, Evt, State> Enqueue<Cmd, Evt, State>
where
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State>,
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
{
    pub fn from_command(command: Cmd) -> Self {
        Self {
            element: EnqueueType::Command(command),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn command(&self) -> Option<&Cmd> {
        match &self.element {
            EnqueueType::Command(command) => Some(command),
            _ => None,
        }
    }
}
