use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use bimap::BiHashMap;
use slotmap::SlotMap;

use crate::{
    marshalling::MarshallingRegistry,
    names::{ActorName, EventName, SubroutineName},
    scenario::{DstPattern, RequiredToBe, SrcMsg},
};

mod keys;
pub use keys::*;

mod build;
mod names;
mod report;
pub(crate) mod runner;
mod sources;

pub use build::BuildError;
pub use report::Report;
pub use runner::RunError;
pub use runner::Runner;
pub use sources::SourceCode;
pub use sources::SourceCodeLoader;

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
    marshalling: MarshallingRegistry,
    events: Events,

    root_scope_key: KeyScope,
    scopes: SlotMap<KeyScope, ScopeInfo>,
}

#[derive(Debug)]
// the fields of this structure can be used to build a sort of stack-trace, which might be useful
#[allow(dead_code)]
struct ScopeInfo {
    source_key: KeyScenario,
    invoked_as: Option<(KeyScope, EventName, SubroutineName)>,
}

#[derive(Debug, Default)]
struct Events {
    priority: HashMap<EventKey, usize>,
    required: HashMap<EventKey, RequiredToBe>,
    names: HashMap<EventKey, (KeyScope, EventName)>,

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
    scope_key: KeyScope,

    from: ActorName,
    to: Option<ActorName>,
    fqn: Arc<str>,
    payload: SrcMsg,
}

#[derive(Debug)]
struct EventRecv {
    scope_key: KeyScope,

    from: Option<ActorName>,
    to: Option<ActorName>,
    fqn: Arc<str>,
    timeout: Option<Duration>,
    payload_matchers: Vec<DstPattern>,
}

#[derive(Debug)]
struct EventRespond {
    scope_key: KeyScope,

    respond_to: KeyRecv,
    request_type: Arc<str>,
    respond_from: Option<ActorName>,
    payload: SrcMsg,
}

#[derive(Debug)]
struct EventDelay {
    delay_for: Duration,
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
    Two {
        src: KeyScope,
        dst: KeyScope,

        // left: src-scope
        // right: dst-scope
        actors: BiHashMap<ActorName, ActorName>,
    },
}
