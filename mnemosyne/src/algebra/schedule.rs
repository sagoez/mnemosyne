use crate::domain::Error;
use crate::domain::Inner;
use crate::storage::Adapter;
use crate::Unit;
use actix::prelude::*;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Message)]
#[rtype(result = "Result<Unit, Error>")]
pub struct Schedule<F>
where
    F: FnMut() -> Unit + Send + Sync + 'static,
{
    factory: F,
    duration: Duration,
}

impl<F> Schedule<F>
where
    F: FnMut() -> Unit + Send + Sync + 'static,
{
    pub fn new(factory: F, duration: std::time::Duration) -> Self {
        Self { factory, duration }
    }
}

impl<F, State, Store> Handler<Schedule<F>> for Inner<State, Store>
where
    F: FnMut() -> Unit + Send + Sync + 'static,
    State: Debug + Clone + Send + Sync + Unpin + 'static,
    Store: Adapter + Clone + Send + Sync + 'static + Unpin,
{
    type Result = Result<Unit, Error>;

    fn handle(&mut self, msg: Schedule<F>, ctx: &mut Context<Self>) -> Self::Result {
        let mut factory = msg.factory;
        let duration = msg.duration;

        ctx.run_later(duration, move |_, _| {
            factory();
        });

        Ok(())
    }
}
