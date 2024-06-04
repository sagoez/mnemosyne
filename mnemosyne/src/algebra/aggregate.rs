use super::{Command, Event, Inner, Record};
use crate::domain::{
    Dequeue, Error, Process, CHUNK_BACKPRESSURE, CHUNK_SIZE, COMMAND_TOPIC, GROUP_ID,
};
use crate::storage::Adapter;
use crate::Unit;
use actix::prelude::*;
use futures::lock::Mutex;
use futures::StreamExt;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::error::KafkaError;
use rdkafka::message::BorrowedMessage;
use rdkafka::{ClientConfig, Message};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

type AddrMap<State, Store, Evt> = HashMap<String, Addr<Inner<State, Store, Evt>>>;

#[derive(Clone)]
pub struct Aggregate<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static,
    Evt: Event<State> + DeserializeOwned + Serialize + Unpin + Debug + 'static,
{
    addr: Arc<Mutex<AddrMap<State, Store, Evt>>>,
    store: Store,
    consumer: Arc<StreamConsumer>,
    _marker: std::marker::PhantomData<Cmd>,
}

impl<State, Store, Cmd, Evt> Aggregate<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static,
    Evt: Event<State> + DeserializeOwned + Serialize + Unpin + Debug + 'static,
{
    pub fn new(configuration: ClientConfig, store: Store) -> Result<Self, Error> {
        Ok(Self {
            addr: Default::default(),
            store,
            _marker: std::marker::PhantomData,
            consumer: {
                let mut configuration = configuration;

                Arc::new(
                    configuration
                        .set("group.id", GROUP_ID)
                        .set("enable.auto.commit", "false")
                        .set("auto.offset.reset", "earliest")
                        .create::<StreamConsumer>()
                        .map_err(Error::Kafka)?,
                )
            },
        })
    }
}

impl<State, Store, Cmd, Evt> Actor for Aggregate<State, Store, Cmd, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static + Default + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Command<State> + Serialize,
    Evt: Event<State> + 'static + DeserializeOwned + Serialize + Unpin + Debug,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.notify(Dequeue);
    }
}

impl<State, Store, Cmd, Evt> Supervised for Aggregate<State, Store, Cmd, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static + Default + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Command<State> + Serialize,
    Evt: Event<State> + 'static + DeserializeOwned + Serialize + Unpin + Debug,
{
    // TODO: Add state recovery
    fn restarting(&mut self, _ctx: &mut Self::Context) {}
}

// TODO: Add logging
impl<State, Store, Cmd, Evt> Handler<Dequeue> for Aggregate<State, Store, Cmd, Evt>
where
    State: Debug + Clone + Send + Sync + Unpin + 'static + Default + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Command<State> + Serialize,
    Evt: Event<State> + 'static + DeserializeOwned + Serialize + Unpin + Debug,
{
    type Result = ResponseActFuture<Self, Result<Unit, Error>>;

    // TODO: Add logging
    fn handle(&mut self, _: Dequeue, _: &mut Self::Context) -> Self::Result {
        let store = self.store.clone();
        let consumer = self.consumer.clone();
        let actors = self.addr.clone();

        Box::pin(
            async move {
                consumer.subscribe(&[COMMAND_TOPIC]).map_err(Error::Kafka)?;

                let mut chunks = consumer.stream().ready_chunks(CHUNK_SIZE as usize);

                if let Some(messages) = chunks.next().await {
                    if messages.is_empty() {
                        return Ok(());
                    }

                    if messages.len() <= 2 {
                        // sleep for a bit to allow for more messages to come in
                        tokio::time::sleep(tokio::time::Duration::from_secs(CHUNK_BACKPRESSURE))
                            .await;
                    }

                    let mut result = Vec::with_capacity(messages.len());
                    for msg in messages.iter() {
                        let actors = actors.clone();
                        let store = store.clone();
                        let msg = msg.as_ref().map_err(|e| Error::Kafka(e.to_owned()))?;

                        let key = msg.key().ok_or(Error::InvalidKey(format!(
                            "Could not find key in message {:?}",
                            msg
                        )))?;

                        let key = String::from_utf8(key.to_vec()).map_err(|e| {
                            Error::InvalidKey(format!("Could not decode key: {}", e))
                        })?;

                        if !actors.lock().await.contains_key(&key) {
                            let inner = Inner::<State, Store, Evt>::new(&key, store.clone());
                            let supervised = Supervisor::start(|_| inner);
                            actors.lock().await.insert(key.clone(), supervised.clone());
                            result.push(process::<State, Store, Cmd, Evt>(msg, supervised).await)
                        } else {
                            result.push(
                                process::<State, Store, Cmd, Evt>(
                                    msg,
                                    actors.lock().await[&key].clone(),
                                )
                                .await,
                            )
                        }
                    }

                    let is_allowed = result.iter().filter(|r| r.is_err()).all(|r| match r {
                        Err(error) => !matches!(
                            error,
                            Error::StorageError(_)
                                | Error::ConnectionError(_)
                                | Error::ConnectionRetrievalError(_)
                        ),
                        _ => true,
                    });

                    if is_allowed {
                        if let Some(Ok(msg)) = messages.last() {
                            consumer
                                .commit_message(msg, CommitMode::Async)
                                .map_err(Error::Kafka)?;
                        }
                    }
                }

                Ok(())
            }
            .into_actor(self)
            .map(|_: Result<Unit, KafkaError>, _, ctx| {
                // TODO: Figure out what to do with errors
                ctx.notify(Dequeue);
                Ok(())
            }),
        )
    }
}

async fn process<'a, State, Store, Cmd, Evt>(
    msg: &'a BorrowedMessage<'a>,
    addr: Addr<Inner<State, Store, Evt>>,
) -> Result<Unit, Error>
where
    State: Clone + Send + Sync + Unpin + 'static + Default + Debug + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + Debug + DeserializeOwned + Command<State> + Serialize,
    Evt: Event<State> + DeserializeOwned + Serialize + Unpin + Debug + 'static,
{
    match msg.payload() {
        Some(payload) => {
            let payload = serde_json::from_slice::<Record<Cmd>>(payload)
                .map_err(|e| Error::InvalidCommand(format!("Could not decode command: {}", e)))?;
            Ok(addr
                .send(Process::<Cmd>::new(payload))
                .await
                .map_err(|e| Error::InvalidCommand(format!("Could not send command: {}", e)))??)
        }
        None => Ok(()),
    }
}
