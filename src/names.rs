use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("A:{_0}")]
pub struct ActorName(Arc<str>);

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("E:{_0}")]
pub struct EventName(Arc<str>);

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("M:{_0}")]
pub struct MessageName(Arc<str>);

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("S:{_0}")]
pub struct SubroutineName(Arc<str>);

impl EventName {
    pub fn with_suffix(&self, suffix: &str) -> Self {
        Self(format!("{}{}", self.0, suffix).into())
    }
}
