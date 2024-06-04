use super::{Aggregate, Event};
use crate::{
    algebra::{Command, Record},
    domain::{Enqueue, Error, GetState, BATCH_BACKPRESSURE, COMMAND_TOPIC},
    storage::Adapter,
    Unit,
};
use actix::{
    Actor, AsyncContext, Context, Handler, ResponseFuture, Supervised, Supervisor, WrapFuture,
};
use futures::{lock::Mutex, StreamExt};
use rdkafka::{
    producer::{DeliveryFuture, FutureProducer, FutureRecord},
    ClientConfig,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, sync::Arc, time::Duration};

pub struct Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State>,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    store: Store,
    producer: Arc<FutureProducer>,
    batch: Arc<Mutex<Vec<DeliveryFuture>>>,
    seq_nr: Arc<Mutex<i64>>,
    _marker: std::marker::PhantomData<(State, Cmd, Evt)>,
}

impl<State, Store, Cmd, Evt> Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    pub(crate) async fn empty(
        configuration: ClientConfig,
        store: Store,
    ) -> Result<Init<State, Store, Cmd, Evt>, Error> {
        let producer: FutureProducer = configuration.create().map_err(Error::Kafka)?;

        let aggregate =
            Aggregate::<State, Store, Cmd, Evt>::new(configuration.clone(), store.clone())?;
        Supervisor::start(|_| aggregate);

        Ok(Self {
            store: store.clone(),
            producer: Arc::new(producer),
            batch: Arc::new(Mutex::new(Vec::new())),
            seq_nr: Arc::new(Mutex::new(0)),
            _marker: std::marker::PhantomData,
        })
    }
}

// Ensure that Kafka is running, that the topic exists and that we can produce to it.
// async fn ensure(producer: &FutureProducer) -> Result<Unit, Error> {
//     let record = FutureRecord::to(COMMAND_TOPIC)
//         .payload("droppable")
//         .key(&[0]);
//
//     let res = producer
//         .send(record, Duration::from_secs(1))
//         .await
//         .map_err(|(e, _)| Error::Kafka(e));
//
//     Ok(())
// }

impl<State, Store, Cmd, Evt> Actor for Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_secs(BATCH_BACKPRESSURE), |act, ctx| {
            let batch = act.batch.clone();
            let future = async move {
                for record in batch.lock().await.drain(..) {
                    match record.await {
                        Ok(result) => match result {
                            Ok((_partition, _offset)) => {}
                            Err((_e, _)) => {}
                        },
                        Err(_e) => {}
                    }
                }

                batch.lock().await.clear();
            };

            ctx.spawn(future.into_actor(act));
        });
    }
}

impl<State, Store, Cmd, Evt> Supervised for Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    fn restarting(&mut self, _: &mut Self::Context) {
        // TODO: fetch state from somewhere and restore it
    }
}

impl<State, Store, Evt, Cmd> Handler<Enqueue<Cmd, Evt, State>> for Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    type Result = ResponseFuture<Result<Unit, Error>>;

    // TODO: Add logging  + Save seq_nr to store
    fn handle(&mut self, msg: Enqueue<Cmd, Evt, State>, _ctx: &mut Self::Context) -> Self::Result {
        let producer = self.producer.clone();
        let batch = self.batch.clone();
        let seq_nr = self.seq_nr.clone();
        Box::pin(async move {
            let command = msg.command().ok_or_else(|| {
                Error::InvalidCommand("Could not extract command from enqueue message".to_string())
            })?;
            let key = command.entity_id();
            let timestamp = chrono::Utc::now();
            let name = command.name();
            let mut seq_nr = seq_nr.lock().await;
            let record = serde_json::to_vec(&Record::command(
                &key,
                msg.command(),
                timestamp,
                name,
                *seq_nr,
            ))
            .map_err(|e| Error::InvalidCommand(format!("Could not serialize command: {}", e)))?;

            let record = FutureRecord::to(COMMAND_TOPIC)
                .payload(&record)
                .key(&key)
                .timestamp(timestamp.timestamp_millis());

            let record = producer
                .send_result(record)
                .map_err(|(e, _)| Error::Kafka(e));

            *seq_nr += 1;

            match record {
                Ok(record) => {
                    batch.lock().await.push(record);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        })
    }
}

const BUFFER_SIZE: u64 = 100;

impl<State, Store, Cmd, Evt> Handler<GetState<State>> for Init<State, Store, Cmd, Evt>
where
    State: Debug + Send + Sync + Unpin + Clone + Default + 'static + DeserializeOwned,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
    Cmd: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Command<State> + Serialize,
    Evt: Send + Sync + Unpin + 'static + DeserializeOwned + Debug + Event<State> + Serialize,
{
    type Result = ResponseFuture<Result<State, Error>>;

    fn handle(&mut self, msg: GetState<State>, _ctx: &mut Self::Context) -> Self::Result {
        let store = self.store.clone();
        let entity_id = msg.entity_id().to_owned();
        Box::pin(async move {
            let highest_seq_nr = store.read_highest_sequence_number(&entity_id).await?;

            match highest_seq_nr {
                Some(highest_seq_nr) => {
                    let state = store
                        .replay::<Evt>(&entity_id, 0, highest_seq_nr, highest_seq_nr + BUFFER_SIZE)
                        .await?
                        .fold(State::default(), |mut state, record| {
                            let event = record.into_message();
                            let new_state = event.apply(&state).unwrap();
                            state = new_state;
                            async move { state }
                        })
                        .await;

                    Ok(state)
                }
                None => Err(Error::InvalidCommand(format!(
                    "Could not find entity with id {}",
                    entity_id
                ))),
            }
        })
    }
}
