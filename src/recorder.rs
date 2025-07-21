#![allow(dead_code)] //TODO: remove it once some report is rendered

//! This module is responsible for recording the events that happened during a test run.
//!
//! A pair of two "timestamps" is used here — one for the wall-clock ([`std::time::Instant`]),
//! the other — for the simulated time ([`tokio::time::Instant`]).
//!
//! [`RecordLog`] — carries the `t_0` timestamp, and the sequence of all logs
//!

use slotmap::{new_key_type, SlotMap};
use std::time::Instant as StdInstant;
use tokio::time::Instant as RtInstant;

pub(crate) mod records;

new_key_type! {
    pub struct KeyRecord;
}

#[derive(Debug, Clone)]
pub struct RecordLog {
    pub(crate) t_zero: (StdInstant, RtInstant),
    pub(crate) roots: Vec<KeyRecord>,
    pub(crate) records: SlotMap<KeyRecord, Record>,
}

#[derive(Debug)]
pub(crate) struct Recorder<'a> {
    log: &'a mut RecordLog,
    parent: Option<KeyRecord>,
    last: Option<KeyRecord>,
}

#[derive(derive_more::Debug, Clone)]
pub(crate) struct Record {
    pub(crate) at: (StdInstant, RtInstant),
    pub(crate) parent: Option<KeyRecord>,
    pub(crate) children: Vec<KeyRecord>,
    pub(crate) previous: Option<KeyRecord>,
    pub(crate) kind: RecordKind,

    #[debug(skip)]
    _no_pub_constructor: NoPubConstructor,
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::From)]
pub(crate) enum RecordKind {
    Root,
    Error(records::Error),
    ProcessEventClass(records::ProcessEventClass),
    EventFired(records::EventFired),
    ReadyBindKeys(records::ReadyBindKeys),
    ReadyRecvKeys(records::ReadyRecvKeys),
    TimedOutRecvKey(records::TimedOutRecvKey),
    ProcessBindKey(records::ProcessBindKey),
    BindSrcScope(records::BindSrcScope),
    BindValue(records::BindValue),
    BindDstScope(records::BindDstScope),
    BindOutcome(records::BindOutcome),
    ProcessSend(records::ProcessSend),
    BindActorName(records::BindActorName),
    ResolveActorName(records::ResolveActorName),
    SendMessageType(records::SendMessageType),
    UsingMsg(records::UsingMsg),
    SendTo(records::SendTo),
    ProcessRespond(records::ProcessRespond),
}

impl RecordLog {
    pub fn new() -> Self {
        let t_zero = (StdInstant::now(), RtInstant::now());
        Self {
            t_zero,
            roots: Default::default(),
            records: Default::default(),
        }
    }

    pub fn t_zero(&self) -> (StdInstant, RtInstant) {
        self.t_zero
    }

    pub(crate) fn recorder(&mut self) -> Recorder {
        let at = (StdInstant::now(), RtInstant::now());
        let kind = RecordKind::Root;
        let parent = None;
        let root_record = Record {
            at,
            parent,
            children: vec![],
            previous: None,
            kind,

            _no_pub_constructor: NoPubConstructor,
        };
        let root_key = self.records.insert(root_record);
        self.roots.push(root_key);
        Recorder {
            log: self,
            parent: Some(root_key),
            last: Some(root_key),
        }
    }
}

impl<'a> Recorder<'a> {
    pub(crate) fn write<'b>(&'b mut self, entry: impl Into<RecordKind>) -> Recorder<'b>
    where
        'a: 'b,
    {
        let at = (StdInstant::now(), RtInstant::now());
        let kind = entry.into();
        let parent = self.parent;
        let record = Record {
            at,
            parent,
            children: vec![],
            previous: self.last,
            kind,

            _no_pub_constructor: NoPubConstructor,
        };
        let key = self.log.records.insert(record);
        if let Some(parent) = parent {
            self.log.records[parent].children.push(key);
        }
        self.last = Some(key);
        Recorder {
            log: self.log,
            parent: Some(key),
            last: None,
        }
    }

    #[deprecated(note = "let's see whether we can do without it")]
    pub(crate) fn on_error<E>(&mut self) -> impl for<'e> FnOnce(&'e E) + use<'_, 'a, E>
    where
        E: ToString,
    {
        |e| {
            self.write(records::Error {
                reason: e.to_string(),
            });
        }
    }
}

#[derive(Clone, Copy)]
struct NoPubConstructor;
