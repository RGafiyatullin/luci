use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use serde_json::Value;
use slotmap::{new_key_type, SlotMap};

use crate::{
    messages::Messages,
    scenario::{ActorName, EventName, Msg, RequiredToBe},
};

mod build;
mod runner;

new_key_type! {
    pub struct KeyBind;
    pub struct KeySend;
    pub struct KeyRecv;
    pub struct KeyRespond;
    pub struct KeyDelay;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKey {
    Bind(KeyBind),
    Send(KeySend),
    Recv(KeyRecv),
    Respond(KeyRespond),
    Delay(KeyDelay),
}

#[derive(Debug)]
pub struct ExecutionGraph {
    messages: Arc<Messages>,
    vertices: Vertices,
}

#[derive(Debug, Default)]
struct Vertices {
    priority: HashMap<EventKey, usize>,
    required: HashMap<EventKey, RequiredToBe>,
    names: HashMap<EventKey, EventName>,

    bind: SlotMap<KeyBind, VertexBind>,
    send: SlotMap<KeySend, VertexSend>,
    recv: SlotMap<KeyRecv, VertexRecv>,
    respond: SlotMap<KeyRespond, VertexRespond>,
    delay: SlotMap<KeyDelay, VertexDelay>,

    entry_points: BTreeSet<EventKey>,

    key_unblocks_values: HashMap<EventKey, BTreeSet<EventKey>>,
}

#[derive(Debug)]
struct VertexSend {
    send_from: ActorName,
    send_to: Option<ActorName>,
    message_type: Arc<str>,
    message_data: Msg,
}

#[derive(Debug)]
struct VertexRecv {
    match_type: Arc<str>,
    match_from: Option<ActorName>,
    match_to: Option<ActorName>,
    match_message: Msg,
}

#[derive(Debug)]
struct VertexRespond {
    respond_to: KeyRecv,
    request_fqn: Arc<str>,
    respond_from: Option<ActorName>,
    message_data: Msg,
}

#[derive(Debug)]
struct VertexBind {
    dst: Value,
    src: Msg,
}

#[derive(Debug)]
struct VertexDelay {
    delay_for: Duration,
    delay_step: Duration,
}
