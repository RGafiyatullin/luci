use std::{
    collections::{BTreeSet, HashMap, HashSet},
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

impl Vertices {
    pub fn draw_graphviz(&self) -> String {
        let mut acc = String::new();
        acc.push_str("digraph test {  rankdir=LR layout=dot\n");

        let tree = self.key_unblocks_values.values().into_iter().flatten();

        self.entry_points
            .iter()
            .chain(tree.clone())
            .cloned()
            .collect::<HashSet<EventKey>>()
            .iter()
            .for_each(|key| {
                self.draw_node(&mut acc, &key);
            });

        for (parent, children) in &self.key_unblocks_values {
            for child in children {
                acc.push_str(&format!("  \"{:?}\" -> \"{:?}\"\n", parent, child));
            }
        }

        acc.push_str("}\n");
        acc
    }

    fn draw_node(&self, acc: &mut String, key: &EventKey) {
        match key {
            EventKey::Delay(key_delay) => {
                let delay = self.delay.get(*key_delay).unwrap();
                acc.push_str(&format!(
                    "  \"{:?}\" [label=\"delay {:?} by {:?}\"]\n",
                    key, delay.delay_for, delay.delay_step
                ));
            }
            EventKey::Bind(key_bind) => {
                let bind = self.bind.get(*key_bind).unwrap();
                let src = serde_yaml::to_string(&bind.src).unwrap();
                let dst = serde_yaml::to_string(&bind.dst).unwrap();
                acc.push_str(&format!(
                    "  \"{:?}\" [label=\"bind\nsrc: \n{}\ndst: \n{}\"]\n",
                    key, src, dst
                ));
            }
            EventKey::Recv(key_recv) => {
                let VertexRecv {
                    match_type,
                    match_from,
                    match_to,
                    match_message,
                } = self.recv.get(*key_recv).unwrap();
                let data = serde_yaml::to_string(match_message).unwrap();
                acc.push_str(&format!(
                    "  \"{:?}\" [label=\"recv '{}'\nfrom: {}\nto: {}\\ndata: {}\"]\n",
                    key,
                    match_type,
                    match_from
                        .clone()
                        .map(|actor| actor.to_string())
                        .unwrap_or_default(),
                    match_to
                        .clone()
                        .map(|actor| actor.to_string())
                        .unwrap_or_default(),
                    data
                ));
            }
            EventKey::Send(key_send) => {
                let VertexSend {
                    send_from,
                    send_to,
                    message_type,
                    message_data,
                } = self.send.get(*key_send).unwrap();
                let data = serde_yaml::to_string(message_data).unwrap();
                acc.push_str(&format!(
                    "  \"{:?}\" [label=\"send '{}'\nfrom: {}\nto: {}\\ndata: {}\"]\n",
                    key,
                    message_type,
                    send_from,
                    send_to
                        .clone()
                        .map(|actor| actor.to_string())
                        .unwrap_or_default(),
                    data
                ));
            }
            EventKey::Respond(key_respond) => {
                let VertexRespond {
                    request_fqn,
                    respond_from,
                    ..
                } = self.respond.get(*key_respond).unwrap();
                acc.push_str(&format!(
                    "  \"{:?}\" [label=\"respond '{}'\\nfrom: {}\"]\n",
                    key,
                    request_fqn,
                    respond_from
                        .clone()
                        .map(|actor| actor.to_string())
                        .unwrap_or_default(),
                ));
            }
        }
    }
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
