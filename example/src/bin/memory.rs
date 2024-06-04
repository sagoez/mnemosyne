use mnemosyne::{
    algebra::{Command, Engine, Event},
    domain::{Error, NonEmptyVec},
    prelude::{Command as MCommand, Event as MEvent},
    rdkafka::ClientConfig,
    storage::MemoryAdapter,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, time::Duration};

#[derive(Default, Debug, Clone, Deserialize)]
pub struct State {
    count: u64,
}

#[derive(Debug, Clone, Serialize, MCommand, Deserialize)]
#[command(state = "State", directive = "UserEvent")]
#[serde(tag = "type")]
pub enum UserCommand {
    Increment(Increment),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Increment;

const ENTITY_ID: &str = "user::entity::id";

impl Command<State> for Increment {
    type T = UserEvent;

    fn validate(&self, _state: &State) -> Result<mnemosyne::Unit, Error> {
        Ok(())
    }

    fn directive(&self, _state: &State) -> Result<NonEmptyVec<Box<Self::T>>, Error> {
        let res: Result<NonEmptyVec<Box<UserEvent>>, Error> =
            NonEmptyVec::new(vec![Box::new(UserEvent::Incremented(Incremented))]);

        res
    }

    fn entity_id(&self) -> String {
        ENTITY_ID.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MEvent)]
#[event(state = "State")]
#[serde(tag = "type")]
pub enum UserEvent {
    Incremented(Incremented),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incremented;

impl Event<State> for Incremented {
    fn apply(&self, state: &State) -> Result<State, Error> {
        Ok(State {
            count: state.count + 1,
        })
    }

    fn effects(&self, before: &State, after: &State) -> mnemosyne::Unit {
        println!("Effects: {:?} -> {:?}", before, after);
    }
}

#[actix::main]
async fn main() {
    let mut configuration = ClientConfig::new();
    let configuration = configuration.set("bootstrap.servers", "localhost:9092");
    println!("Configuration created");

    let engine: Engine<State, MemoryAdapter, UserCommand, Incremented> =
        Engine::start(configuration.to_owned(), MemoryAdapter::default())
            .await
            .expect("Could not create engine");

    println!("Engine created");

    for _ in 0..10 {
        let command = UserCommand::Increment(Increment);
        println!("Command: {:?}", command);

        engine
            .enqueue(command.clone())
            .await
            .expect("Could not enqueue command");

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    tokio::time::sleep(Duration::from_secs(3)).await;

    let state = engine.state(ENTITY_ID).await.expect("Could not get state");

    println!("State: {:?}", state); // State { count: 10 }
}
