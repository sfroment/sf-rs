// Declare the internal modules. Files must exist.
mod common;
mod counter;
mod gauge;
mod histogram;
mod implementation;

pub use implementation::InMemoryMetrics;
