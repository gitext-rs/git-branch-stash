#![allow(clippy::collapsible_else_if)]

pub mod config;

pub use git::*;
pub use snapshot::*;
pub use stack::*;

mod git;
mod snapshot;
mod stack;
