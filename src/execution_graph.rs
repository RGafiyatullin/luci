use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use slotmap::{new_key_type, SlotMap};

use crate::{
    messages::Messages,
    scenario::{ActorName, EventName, Msg},
};

mod build;
mod runner;

new_key_type! {
    pub struct KeySend;
    pub struct KeyRecv;
    pub struct KeyRespond;
    pub struct KeyDelay;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKey {
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
    mandatory: HashSet<EventKey>,
    names: HashMap<EventKey, EventName>,

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
struct VertexDelay(Duration);
