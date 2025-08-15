use std::sync::Arc;
use std::time::Duration;

use elfo::Addr;
use serde_json::Value;
use tokio::time::Instant;

use crate::execution::runner::ReadyEventKey;
use crate::execution::{
    EventKey, KeyActor, KeyBind, KeyDummy, KeyRecv, KeyRespond, KeyScope, KeySend,
};
use crate::scenario::{DstPattern, SrcMsg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Error {
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessEventClass(pub ReadyEventKey);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventFired(pub EventKey);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadyBindKeys(pub Vec<KeyBind>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimedOutRecvKey(pub KeyRecv);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadyRecvKeys(pub Vec<KeyRecv>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessBindKey(pub KeyBind);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindSrcScope(pub KeyScope);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsingMsg(pub SrcMsg);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsingValue(pub Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewBinding(pub String, pub Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindDstScope(pub KeyScope);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchActorAddress(pub KeyActor, pub KeyScope, pub Addr, pub Addr);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchDummyAddress(pub KeyDummy, pub KeyScope, pub Addr, pub Addr);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreActorAddress(pub KeyActor, pub KeyScope, pub Addr);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolveActorName(pub KeyActor, pub KeyScope, pub Addr);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindOutcome(pub bool);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BindToPattern(pub DstPattern);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessSend(pub KeySend);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SendMessageType(pub Arc<str>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SendTo(pub Option<Addr>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessRespond(pub KeyRespond);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnvelopeReceived {
    pub message_name: &'static str,
    pub from:         Addr,
    pub to_opt:       Option<Addr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchingRecv(pub KeyRecv);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpectedDirectedGotRouted(pub KeyDummy);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidFrom(pub Instant);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TooEarly(pub Duration);
