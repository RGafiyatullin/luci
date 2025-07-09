use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActorName(Arc<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventName(Arc<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MessageName(Arc<str>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeAlias {
    #[serde(rename = "use")]
    pub type_name: String,
    #[serde(rename = "as")]
    pub type_alias: MessageName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<TypeAlias>,
    pub cast: Vec<ActorName>,
    pub events: Vec<EventDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDef {
    pub id: EventName,

    #[serde(default)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mandatory: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<EventName>,

    #[serde(flatten)]
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Recv(EventRecv),
    Send(EventSend),
    Respond(EventRespond),
    Delay(#[serde(with = "humantime_serde")] Duration),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRespond {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ActorName>,

    pub to: EventName,
    pub data: Msg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Msg {
    Exact(Value),
    Bind(Value),
    Injected(String),
}
