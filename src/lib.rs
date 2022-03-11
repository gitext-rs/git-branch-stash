#![allow(clippy::collapsible_else_if)]

pub mod config;
pub mod git;

pub use snapshot::*;
pub use stack::*;

mod snapshot;
mod stack;
