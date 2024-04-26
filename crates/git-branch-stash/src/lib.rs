#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod config;

pub use git::*;
pub use snapshot::*;
pub use stack::*;

mod git;
mod snapshot;
mod stack;
