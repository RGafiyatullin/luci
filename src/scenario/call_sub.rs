use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scenario::{no_extra::NoExtra, ActorName, Msg, SubName};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EventCallSub {
    pub(crate) sub: SubName,

    pub(crate) cast: Vec<CallSubBindActor>,

    pub(crate) input: CallSubBindValue,
    pub(crate) output: CallSubBindValue,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CallSubBindActor {
    #[serde(rename = "use")]
    pub(crate) outer_name: ActorName,
    #[serde(rename = "as")]
    pub(crate) inner_name: ActorName,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CallSubBindValue {
    pub(crate) dst: Value,
    pub(crate) src: Msg,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}
