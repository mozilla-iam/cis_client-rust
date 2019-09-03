#[macro_use]
extern crate failure_derive;

mod auth;
pub mod batch;
mod client;
pub mod error;
pub mod getby;
mod secrets;
pub mod settings;
pub mod sync;

pub use client::AsyncCisClientTrait;
pub use client::CisClient;
