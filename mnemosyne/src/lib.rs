pub mod algebra;
pub mod domain;
pub mod storage;
pub use futures;
pub use rdkafka;

pub type Unit = ();

pub mod prelude {
    pub use crate::algebra::*;
    pub use crate::domain::*;
    pub use crate::storage::*;
    #[cfg(feature = "derive")]
    pub use mnemosyne_derive::*;
}

use ractor::{cast, Actor, ActorProcessingErr, ActorRef, BytesConvertable};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Message {
    Ping,
    Pong,
}

pub struct PingPong;

impl BytesConvertable for Message {
    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn from_bytes(bytes: Vec<u8>) -> Self {
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl Message {
    // retrieve the next message in the sequence
    fn next(&self) -> Self {
        match self {
            Self::Ping => Self::Pong,
            Self::Pong => Self::Ping,
        }
    }
    // print out this message
    fn print(&self) {
        match self {
            Self::Ping => print!("ping.."),
            Self::Pong => print!("pong.."),
        }
    }
}

// the implementation of our actor's "logic"
impl Actor for PingPong {
    // An actor has a message type
    type Msg = Message;
    // and (optionally) internal state
    type State = u8;
    // Startup initialization args
    type Arguments = ();

    // Initially we need to create our state, and potentially
    // start some internal processing (by posting a message for
    // example)
    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        // startup the event processing
        cast!(myself, Message::Ping)?;
        // create the initial state
        Ok(0u8)
    }

    // This is our main message handler
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if *state < 10u8 {
            message.print();
            cast!(myself, message.next())?;
            *state += 1;
        } else {
            println!();
            myself.stop(None);
            // don't send another message, rather stop the agent after 10 iterations
        }
        Ok(())
    }
}
