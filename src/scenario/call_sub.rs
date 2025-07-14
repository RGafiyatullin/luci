use serde::{Deserialize, Serialize};

use crate::scenario::{no_extra::NoExtra, SubName};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCallSub {
    pub sub: SubName,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}
