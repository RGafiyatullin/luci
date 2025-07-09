use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod no_extra;
use no_extra::NoExtra;

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
    pub types: Vec<TypeAlias>,
    pub cast: Vec<ActorName>,
    pub events: Vec<EventDef>,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDef {
    pub id: EventName,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<RequiredToBe>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<EventName>,

    #[serde(flatten)]
    pub kind: EventKind,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Bind(EventBind),
    Recv(EventRecv),
    Send(EventSend),
    Respond(EventRespond),
    Delay(#[serde(with = "humantime_serde")] Duration),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBind {
    pub dst: Value,
    pub src: Msg,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecv {
    #[serde(rename = "type")]
    pub message_type: MessageName,
    #[serde(rename = "data")]
    pub message_data: Msg,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ActorName>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<ActorName>,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSend {
    pub from: ActorName,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<ActorName>,

    #[serde(rename = "type")]
    pub message_type: MessageName,
    #[serde(rename = "data")]
    pub message_data: Msg,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRespond {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ActorName>,

    pub to: EventName,
    pub data: Msg,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Msg {
    Exact(Value),
    Bind(Value),
    Injected(String),
}
