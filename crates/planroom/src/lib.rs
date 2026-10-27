//! The `planroom` binary's library: the CLI, the live loop and the app services, the composition
//! root of every other crate.

pub mod app;
pub mod cli;
pub mod clock;
pub mod config;
pub mod live;
pub mod packs;
pub mod server;
mod text;
