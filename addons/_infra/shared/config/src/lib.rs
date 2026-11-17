//! Configuration primitives shared by the host and independent integrations.
pub mod ini;
mod storage;
pub use storage::{write_atomic, write_atomic_bytes, DirectoryLock};

#[cfg(test)]
mod tests;

pub mod layout;
pub mod names;
pub mod ownership;
pub mod pci;
pub mod process;
