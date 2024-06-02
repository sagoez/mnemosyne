# Mnemosyne

Greek goddess of memory, mother of the Muses. Mnemosyne is a tool for storing and retrieving memories, i.e. event sourced data.

This is a work in progress, and is not yet ready for production use. The API is likely to change.

The API is predominantly constructed around a single type, Engine, which is a convenient wrapper around an Actor. 
The Actor is the core of the system, and is the responsible for storing, validating and retrieving data.

### Engine

The Engine is a convenient wrapper around an `Actix` actor.  In order to use the engine, you must provide three things:

1. A type that implements the `Command` trait.  This is the type that will be used to send commands to the engine.
2. A `State` that implements the `Default` trait. This will be used to initialize the engine and recover its state.
3. A `Configuration` that sets the `Kafka` properties.  This is used to configure the Kafka consumer and producer.

```rust
let mut configuration = ClientConfig::new();
let configuration = configuration.set("bootstrap.servers", "localhost:9092");

let mut engine: Engine<State, MemoryAdapter, UserCommand> =
    Engine::empty(configuration.to_owned(), Some(MemoryAdapter::default()))
        .await
        .expect("Could not create engine");
```

### Command

The `Command` trait is used to send commands to the engine.  The command will then be sent to the engine's actor, which will validate the command,
and if it is valid, it will then yield a list of events.

The `Variant` is just an internal type that ensures that the command is an `Enum` of `Command` types. This means that the engine *does not* receive
a single command, but rather a list of commands.

```rust
pub trait Command<State>: Send + Sync
where
    State: Debug + Clone + Send + Sync + 'static,
{
    /// Validate the command. This function will be called before the `directive`
    /// function. If the command is invalid, an error should be returned.
    fn validate(&self, state: &State) -> Result<Unit, Error>;

    /// Yield a directive. Essentially, it should return an event or a list of events.
    /// Event order is ensured and enforced by the engine.
    fn directive(&self, state: &State) -> Result<NonEmptyVec<Box<dyn Variant<State>>>, Error>;

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
```

### Event

The `Event` trait is used to apply events to the engine's state.  The engine will apply the events to the state, and then return the new state.
After the state has been updated, the engine will then publish the events to Kafka and save the `Event` on the storage adapter.

```rust
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
    fn effects(&self, before: &State, after: &State) -> Unit;
}
```

### Storage

The `Adapter` trait is used to store and retrieve events.  The engine will use the adapter to store events, and to retrieve events when recovering the state,
replaying events and when storing snapshots. The default adapter is the `MemoryAdapter`, which stores events in memory.  This adapter is not suitable for production use.

If you want to use a different adapter, you can implement the `Adapter` trait and pass it to the engine when creating it.

```rust
pub trait Adapter {
    /// Creates a new connection to the database and returns a handle to it.
    async fn connect(&self) -> Self;

    /// Read the highest sequence number for a given entity id from the database
    async fn read_highest_sequence_number(&self, entity_id: &str) -> Result<Option<u64>, Error>;
    /// Write a batch of messages atomically to the database
    async fn write<T>(&self, batch: Vec<Record<T>>) -> Result<Unit, Error>
    where
        T: Serialize + Send + DeserializeOwned;
    /// Replay messages from the database for a given entity id and sequence number
    /// range.
    async fn replay<T>(
        &self,
        entity_id: &str,
        from_sequence_number: u64,
        to_sequence_number: u64,
        max: u64,
    ) -> Result<BoxStream<'static, Record<T>>, Error>
    where
        T: DeserializeOwned + Send + Debug + 'static + Serialize;
}
```

## Summary

```
In summary, the engine does six (6) things:

* Consumer gets message (command) from Kafka
* Consumer deserializes message
* Consumer sends message to actor
* Actor processes message:
    1. Validate command
    2. If valid, yield events
    3. Apply events to state
    4. Save events to storage
    5. Publish events to Kafka
* Actor sends response to consumer
* Consumer commits message offset to Kafka

This is to ensure no message is lost and that the system is resilient to failure.
```
