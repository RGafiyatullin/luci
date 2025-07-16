use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use serde_json::Value;
use slotmap::{new_key_type, SlotMap};

use crate::{
    names::{ActorName, EventName},
    scenario::{Msg, RequiredToBe},
};

mod build;
mod graphics;
mod report;
mod runner;

pub use build::BuildError;
pub use report::Report;
pub use runner::RunError;
pub use runner::Runner;

new_key_type! {
    pub struct KeyBind;
    pub struct KeySend;
    pub struct KeyRecv;
    pub struct KeyRespond;
    pub struct KeyDelay;
}

/// A key corresponding to some event during test execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKey {
    Bind(KeyBind),
    Send(KeySend),
    Recv(KeyRecv),
    Respond(KeyRespond),
    Delay(KeyDelay),
}

#[derive(Debug)]
pub struct Executable {
    events: Events,
}

#[derive(Debug, Default)]
struct Events {
    priority: HashMap<EventKey, usize>,
    required: HashMap<EventKey, RequiredToBe>,
    names: HashMap<EventKey, EventName>,

    bind: SlotMap<KeyBind, EventBind>,
    send: SlotMap<KeySend, EventSend>,
    recv: SlotMap<KeyRecv, EventRecv>,
    respond: SlotMap<KeyRespond, EventRespond>,
    delay: SlotMap<KeyDelay, EventDelay>,

    entry_points: BTreeSet<EventKey>,

    key_unblocks_values: HashMap<EventKey, BTreeSet<EventKey>>,
}

#[derive(Debug)]
struct EventSend {
    from: ActorName,
    to: Option<ActorName>,
    fqn: Arc<str>,
    payload: Msg,
}

#[derive(Debug)]
struct EventRecv {
    from: Option<ActorName>,
    to: Option<ActorName>,
    fqn: Arc<str>,
    payload: Msg,
}

#[derive(Debug)]
struct EventRespond {
    respond_to: KeyRecv,
    request_type: Arc<str>,
    respond_from: Option<ActorName>,
    payload: Msg,
}

#[derive(Debug)]
struct EventBind {
    dst: Value,
    src: Msg,
}

#[derive(Debug)]
struct EventDelay {
    delay_for: Duration,
    delay_step: Duration,
}
