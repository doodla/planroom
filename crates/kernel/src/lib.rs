//! Pure, domain-free logic: fold, roll-up, query shapes, diff, impact, reconcile and policy; no I/O.

pub mod cas;
pub mod display;
pub mod facts;
pub mod fold;
pub mod keys;
pub mod model;
pub mod redact;
pub mod redact_fields;
pub mod resolve;
pub mod rollup;
pub mod tree;

pub use fold::fold;
pub use model::{Facts, Model};
pub use resolve::{Resolved, resolve};
