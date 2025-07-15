use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("ACT:{_0}")]
pub struct ActorName(Arc<str>);

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("EVT:{_0}")]
pub struct EventName(Arc<str>);

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, derive_more::Display,
)]
#[display("MSG:{_0}")]
pub struct MessageName(Arc<str>);
