#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod config;

pub use git::GitRepo;
pub use snapshot::Branch;
pub use snapshot::Snapshot;
pub use stack::Stack;

mod git;
mod snapshot;
mod stack;
