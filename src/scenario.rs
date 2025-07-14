use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod names;
pub use names::*;

mod call_sub;
pub use call_sub::*;

mod no_extra;
use no_extra::NoExtra;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAlias {
    #[serde(rename = "use")]
    pub type_name: String,
    #[serde(rename = "as")]
    pub type_alias: MessageName,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
)]
#[serde(rename_all = "snake_case")]
pub enum RequiredToBe {
    Reached,
    Unreached,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) types: Vec<TypeAlias>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) sub: Vec<SubDef>,

    pub(crate) cast: Vec<ActorName>,
    pub(crate) events: Vec<EventDef>,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubDef {
    #[serde(rename = "load")]
    pub(crate) def_path: PathBuf,
    #[serde(rename = "as")]
    pub(crate) sub_name: SubName,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDef {
    pub(crate) id: EventName,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) require: Option<RequiredToBe>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) after: Vec<EventName>,

    #[serde(flatten)]
    pub(crate) kind: EventKind,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EventKind {
    Bind(EventBind),
    Recv(EventRecv),
    Send(EventSend),
    Respond(EventRespond),
    Call(EventCallSub),
    Delay(EventDelay),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EventBind {
    pub(crate) dst: Value,
    pub(crate) src: Msg,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EventRecv {
    #[serde(rename = "type")]
    pub(crate) message_type: MessageName,
    #[serde(rename = "data")]
    pub(crate) message_data: Msg,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from: Option<ActorName>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to: Option<ActorName>,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSend {
    pub(crate) from: ActorName,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to: Option<ActorName>,

    #[serde(rename = "type")]
    pub(crate) message_type: MessageName,
    #[serde(rename = "data")]
    pub(crate) message_data: Msg,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRespond {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from: Option<ActorName>,

    pub(crate) to: EventName,
    pub(crate) data: Msg,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDelay {
    #[serde(with = "humantime_serde")]
    #[serde(rename = "for")]
    pub(crate) delay_for: Duration,

    #[serde(with = "humantime_serde")]
    #[serde(rename = "step")]
    #[serde(default = "defaults::default_delay_step")]
    pub(crate) delay_step: Duration,

    #[serde(flatten)]
    pub(crate) no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Msg {
    #[serde(alias = "exact")]
    Literal(Value),
    Bind(Value),
    #[serde(alias = "injected")]
    Inject(String),
}

mod defaults {
    use std::time::Duration;

    pub(crate) fn default_delay_step() -> Duration {
        Duration::from_millis(25)
    }
}
