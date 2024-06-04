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
