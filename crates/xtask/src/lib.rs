//! The one task entry point's library: the checks and generators `cargo xtask` runs, exposed so
//! the gate tests under `crates/xtask/tests/` can run them on a temporary copy of the repo.

pub mod areas;
pub mod boundaries;
pub mod canvas;
pub mod docs;
pub mod fixture;
pub mod generate;
pub mod layout;
pub mod modgen;
pub mod plugin;
pub mod report;
pub mod rustlex;
pub mod tools;
pub mod util;
pub mod workflows;
