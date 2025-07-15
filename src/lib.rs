//! `luci` — an event-based testkit for [`elfo`](docs.rs/elfo).
//!
//! We define [scenarios](crate::scenario::Scenario).
//!
//! We compile scenarios into [executables](crate::execution::Executable).
//!
//! We [run executables](crate::execution::Runner) to get [reports](crate::execution::Report).

pub mod execution;
pub mod messages;
pub mod names;
pub mod scenario;
