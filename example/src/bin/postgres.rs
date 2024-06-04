use mnemosyne::{
    algebra::{Command, Engine, Event},
    domain::{Error, NonEmptyVec},
    prelude::{Command as MCommand, Event as MEvent},
    rdkafka::ClientConfig,
    storage::{PostgresAdapter, PostgresAdapterBuilder, SslMode},
    Unit,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, time::Duration};

pub const ENTITY_ID: &str = "tictactoe::player::1";

#[derive(Default, Debug, Clone, Deserialize)]
/// Tic Tac Toe
pub struct State {
    pub board: Board,
    pub current: Player,
    pub winner: Option<Player>,
    pub draw: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub inner: Vec<Vec<Option<Player>>>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            inner: vec![
                vec![None, None, None],
                vec![None, None, None],
                vec![None, None, None],
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum Player {
    #[default]
    X,
    O,
}

impl Event<State> for Player {
    fn apply(&self, _state: &State) -> Result<State, Error> {
        Ok(State::default())
    }

    fn effects(&self, _before: &State, _after: &State) -> Unit {
        println!("Player won: {:?}", self);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    pub player: Player,
    pub x: usize,
    pub y: usize,
}

impl Move {
    pub fn turn(&self) -> Player {
        match self.player {
            Player::X => Player::O,
            Player::O => Player::X,
        }
    }
}

impl Command<State> for Move {
    type T = PlayerEvent;

    fn validate(&self, state: &State) -> Result<Unit, Error> {
        if self.x > 2 || self.y > 2 {
            return Err(Error::new("Move is out of bounds"));
        }

        if state
            .board
            .inner
            .get(self.x)
            .and_then(|row| row.get(self.y))
            .cloned()
            .flatten()
            .is_some()
        {
            return Err(Error::new("Cell is already occupied"));
        }

        if self.player != state.current {
            return Err(Error::new("It is not your turn"));
        }

        Ok(())
    }

    fn directive(&self, state: &State) -> Result<NonEmptyVec<Box<Self::T>>, Error> {
        if state.winner.is_some() {
            Ok(NonEmptyVec::new(vec![Box::new(PlayerEvent::GameWon(
                state.winner.clone().unwrap(),
            ))])?)
        } else if state.draw {
            Ok(NonEmptyVec::new(vec![Box::new(PlayerEvent::GameDraw(
                GameDraw,
            ))])?)
        } else {
            Ok(NonEmptyVec::new(vec![Box::new(PlayerEvent::MoveMade(
                self.clone(),
            ))])?)
        }
    }

    fn entity_id(&self) -> String {
        ENTITY_ID.to_owned()
    }
}

impl Event<State> for Move {
    fn apply(&self, state: &State) -> Result<State, Error> {
        let mut board = state.board.clone();
        if let Some(cell) = board
            .inner
            .get_mut(self.x)
            .and_then(|row| row.get_mut(self.y))
        {
            *cell = Some(self.player.clone());
        }

        let mut winner = None;

        // Check rows
        for row in board.inner.iter() {
            if row.iter().all(|cell| cell.is_some()) {
                if row.iter().all(|cell| cell == &Some(Player::X)) {
                    winner = Some(Player::X);
                } else if row.iter().all(|cell| cell == &Some(Player::O)) {
                    winner = Some(Player::O);
                }
            }
        }

        // Check columns
        for i in 0..3 {
            if board.inner.iter().all(|row| row[i].is_some()) {
                if board.inner.iter().all(|row| row[i] == Some(Player::X)) {
                    winner = Some(Player::X);
                    break;
                } else if board.inner.iter().all(|row| row[i] == Some(Player::O)) {
                    winner = Some(Player::O);
                    break;
                }
            }
        }

        // Check diagonals
        if board.inner.iter().all(|row| row[0].is_some())
            && board.inner.iter().all(|row| row[0] == row[1])
            && board.inner.iter().all(|row| row[0] == row[2])
        {
            winner = board
                .inner
                .first()
                .and_then(|row| row.first())
                .cloned()
                .flatten();
        }

        if board.inner.iter().all(|row| row[2].is_some())
            && board.inner.iter().all(|row| row[2] == row[1])
            && board.inner.iter().all(|row| row[2] == row[0])
        {
            winner = board
                .inner
                .first()
                .and_then(|row| row.get(2))
                .cloned()
                .flatten();
        }

        // Check draw
        let mut draw = true;
        for row in board.inner.iter() {
            for cell in row.iter() {
                if cell.is_none() {
                    draw = false;
                    break;
                }
            }
        }

        match winner {
            Some(_) => Ok(State {
                board,
                current: self.turn(),
                winner,
                draw: false,
            }),
            None => Ok(State {
                board,
                current: self.turn(),
                winner: None,
                draw,
            }),
        }
    }

    fn effects(&self, before: &State, after: &State) -> mnemosyne::Unit {
        let previous = &before.board.inner[self.x][self.y];
        let current = &after.board.inner[self.x][self.y];

        println!("Previous: {:?}", previous);
        println!("Current: {:?}", current);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MCommand)]
#[command(state = "State", directive = "PlayerEvent")]
#[serde(tag = "type")]
pub enum PlayerCommand {
    MakeMove(Move),
}

#[derive(Debug, Clone, Serialize, Deserialize, MEvent)]
#[event(state = "State")]
pub enum PlayerEvent {
    MoveMade(Move),
    GameWon(Player),
    GameDraw(GameDraw),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDraw;

impl Event<State> for GameDraw {
    fn apply(&self, _state: &State) -> Result<State, Error> {
        Ok(State::default())
    }

    fn effects(&self, _before: &State, _after: &State) -> Unit {
        println!("Game ended in a draw");
    }
}

#[actix::main]
async fn main() {
    let mut configuration = ClientConfig::new();
    let configuration = configuration.set("bootstrap.servers", "localhost:9092");
    let storage = PostgresAdapter::connect(PostgresAdapterBuilder::new(
        "localhost",
        "postgres",
        5432,
        "postgres",
        "mnemosyne",
        10,
        SslMode::new(false),
    ))
    .await;

    let engine: Engine<State, PostgresAdapter, PlayerCommand, PlayerEvent> =
        Engine::start(configuration.to_owned(), storage)
            .await
            .expect("Could not create engine");

    let move_1 = Move {
        player: Player::X,
        x: 0,
        y: 0,
    };

    let move_2 = Move {
        player: Player::O,
        x: 1,
        y: 0,
    };

    let move_3 = Move {
        player: Player::X,
        x: 0,
        y: 1,
    };

    let move_4 = Move {
        player: Player::O,
        x: 1,
        y: 1,
    };

    let move_5 = Move {
        player: Player::X,
        x: 0,
        y: 2,
    };

    let move_6 = Move {
        player: Player::O,
        x: 1,
        y: 2,
    };

    for m in [move_1, move_2, move_3, move_4, move_5, move_6] {
        engine
            .enqueue(PlayerCommand::MakeMove(m))
            .await
            .expect("Could not enqueue command");
    }

    tokio::time::sleep(Duration::from_secs(10)).await;
}
