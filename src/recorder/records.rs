use std::sync::Arc;

use elfo::Addr;
use serde_json::Value;

use crate::{
    execution::{runner::ReadyEventKey, EventKey, KeyBind, KeyRecv, KeyRespond, KeyScope, KeySend},
    names::ActorName,
    scenario::Msg,
};

#[derive(Debug, Clone)]
pub(crate) struct Error {
    pub(crate) reason: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessEventClass(pub ReadyEventKey);

#[derive(Debug, Clone)]
pub(crate) struct EventFired(pub EventKey);

#[derive(Debug, Clone)]
pub(crate) struct ReadyBindKeys(pub Vec<KeyBind>);

#[derive(Debug, Clone)]
pub(crate) struct ReadyRecvKeys(pub Vec<KeyRecv>);

#[derive(Debug, Clone)]
pub(crate) struct ProcessBindKey(pub KeyBind);

#[derive(Debug, Clone)]
pub(crate) struct BindSrcScope(pub KeyScope);

#[derive(Debug, Clone)]
pub(crate) struct UsingMsg(pub Msg);

#[derive(Debug, Clone)]
pub(crate) struct BindValue(pub Value);

#[derive(Debug, Clone)]
pub(crate) struct BindDstScope(pub KeyScope);

#[derive(Debug, Clone)]
pub(crate) struct BindActorName(pub ActorName, pub Addr, pub bool);

#[derive(Debug, Clone)]
pub(crate) struct ResolveActorName(pub ActorName, pub Addr);

#[derive(Debug, Clone)]
pub(crate) struct BindOutcome(pub bool);

#[derive(Debug, Clone)]
pub(crate) struct ProcessSend(pub KeySend);

#[derive(Debug, Clone)]
pub(crate) struct SendMessageType(pub Arc<str>);

#[derive(Debug, Clone)]
pub(crate) struct SendTo(pub Option<Addr>);

#[derive(Debug, Clone)]
pub(crate) struct ProcessRespond(pub KeyRespond);
