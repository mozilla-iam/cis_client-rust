// DEBT: we slow down the compiler in favour of maintaining downstream
// compatibility.
//
// Addresses:
//  the `Err`-variant returned from this function is very large
// https://rust-lang.github.io/rust-clippy/rust-1.93.0/index.html#result_large_err
#![allow(clippy::result_large_err)]

mod auth;
mod client;
mod encoding;
pub mod error;
pub mod getby;
mod secrets;
pub mod settings;
#[cfg(feature = "sync")]
pub mod sync;

pub use client::AsyncCisClientTrait;
pub use client::CisClient;
pub use client::CisFut;
