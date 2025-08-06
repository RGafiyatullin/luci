use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use slotmap::{SecondaryMap, SlotMap};

use crate::marshalling::MarshallingRegistry;
use crate::names::{ActorName, DummyName, EventName, SubroutineName};
use crate::scenario::{DstPattern, RequiredToBe, SrcMsg};

mod keys;
pub use keys::*;

mod build;
mod display;
mod names;
mod report;
pub(crate) mod runner;
mod sources;

pub use build::BuildError;
pub use report::Report;
pub use runner::{RunError, Runner};
pub use sources::{SourceCode, SourceCodeLoader};

/// A key corresponding to some event during test execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::From)]
pub enum EventKey {
    Bind(KeyBind),
    Send(KeySend),
    Recv(KeyRecv),
    Respond(KeyRespond),
    Delay(KeyDelay),
}

#[derive(Debug)]
pub struct Executable {
    marshalling:        MarshallingRegistry,
    pub(crate) actors:  SlotMap<KeyActor, ActorInfo>,
    pub(crate) dummies: SlotMap<KeyDummy, DummyInfo>,
    events:             Events,

    root_scope_key:    KeyScope,
    pub(crate) scopes: SlotMap<KeyScope, ScopeInfo>,
}

#[derive(Debug)]
// the fields of this structure can be used to build a sort of stack-trace, which might be useful
#[allow(dead_code)]
pub(crate) struct ScopeInfo {
    pub(crate) source_key: KeyScenario,
    pub(crate) invoked_as: Option<(KeyScope, EventName, SubroutineName)>,
}

#[derive(Debug)]
pub(crate) struct ActorInfo {
    pub(crate) known_as: SecondaryMap<KeyScope, ActorName>,
}

#[derive(Debug)]
pub(crate) struct DummyInfo {
    pub(crate) known_as: SecondaryMap<KeyScope, DummyName>,
}

#[derive(Debug, Default)]
struct Events {
    priority: HashMap<EventKey, usize>,
    required: HashMap<EventKey, RequiredToBe>,
    names:    HashMap<EventKey, (KeyScope, EventName)>,

    bind:    SlotMap<KeyBind, EventBind>,
    send:    SlotMap<KeySend, EventSend>,
    recv:    SlotMap<KeyRecv, EventRecv>,
    respond: SlotMap<KeyRespond, EventRespond>,
    delay:   SlotMap<KeyDelay, EventDelay>,

    entry_points: BTreeSet<EventKey>,

    key_unblocks_values: HashMap<EventKey, BTreeSet<EventKey>>,
}

#[derive(Debug)]
struct EventSend {
    scope_key: KeyScope,

    from:    KeyDummy,
    to:      Option<KeyActor>,
    fqn:     Arc<str>,
    payload: SrcMsg,
}

#[derive(Debug)]
struct EventRecv {
    scope_key: KeyScope,

    from:             Option<KeyActor>,
    to:               Option<KeyDummy>,
    fqn:              Arc<str>,
    timeout:          Option<Duration>,
    payload_matchers: Vec<DstPattern>,
}

#[derive(Debug)]
struct EventRespond {
    scope_key: KeyScope,

    respond_to:   KeyRecv,
    request_type: Arc<str>,
    respond_from: Option<KeyDummy>,
    payload:      SrcMsg,
}

#[derive(Debug)]
struct EventDelay {
    delay_for:  Duration,
    delay_step: Duration,
}

#[derive(Debug)]
struct EventBind {
    dst: DstPattern,
    src: SrcMsg,

    scope: BindScope,
}

#[derive(Debug)]
enum BindScope {
    Same(KeyScope),
    Two { src: KeyScope, dst: KeyScope },
}
