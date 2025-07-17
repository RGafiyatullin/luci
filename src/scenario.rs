use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    names::*,
    scenario::subs::{DefCallSub, DefDeclareSub},
};

mod no_extra;
use no_extra::NoExtra;

mod subs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<DefTypeAlias>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "backward-compatibility", serde(alias = "subs"))]
    pub subroutines: Vec<DefDeclareSub>,

    pub cast: Vec<ActorName>,
    pub events: Vec<DefEvent>,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefTypeAlias {
    #[serde(rename = "use")]
    pub type_name: String,
    #[serde(rename = "as")]
    pub type_alias: MessageName,

    #[serde(flatten)]
    pub no_extra: NoExtra,
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
pub struct DefEvent {
    pub id: EventName,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<RequiredToBe>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "happens_after")]
    #[cfg_attr(feature = "backward-compatibility", serde(alias = "after"))]
    pub prerequisites: Vec<EventName>,

    #[serde(flatten)]
    pub kind: DefEventKind,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefEventKind {
    Bind(DefEventBind),
    Recv(DefEventRecv),
    Send(DefEventSend),
    Respond(DefEventRespond),
    Delay(DefEventDelay),
    Call(DefCallSub),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefEventBind {
    pub dst: Value,
    pub src: Msg,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefEventRecv {
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
pub struct DefEventSend {
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
pub struct DefEventRespond {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<ActorName>,

    #[cfg_attr(feature = "backward-compatibility", serde(alias = "to"))]
    pub to_request: EventName,
    pub data: Msg,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefEventDelay {
    #[serde(with = "humantime_serde")]
    #[serde(rename = "for")]
    pub delay_for: Duration,

    #[serde(with = "humantime_serde")]
    #[serde(rename = "step")]
    #[serde(default = "defaults::default_delay_step")]
    pub delay_step: Duration,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

/// A template for marshalling [elfo::AnyMessage].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Msg {
    /// Stores [Value] to be marshalled as [elfo::AnyMessage] as-is.
    #[cfg_attr(feature = "backward-compatibility", serde(alias = "exact"))]
    Literal(Value),
    /// Stores [Value] to be bound with values for variables in it and then
    /// marshalled as [elfo::AnyMessage].
    Bind(Value),
    /// Stores a key to find a predefined [elfo::AnyMessage] to be injected
    /// into the message flow.
    #[cfg_attr(feature = "backward-compatibility", serde(alias = "injected"))]
    Inject(String),
}

mod defaults {
    use std::time::Duration;

    pub fn default_delay_step() -> Duration {
        Duration::from_millis(25)
    }
}
