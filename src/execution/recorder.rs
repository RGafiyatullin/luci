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

new_key_type! {
    pub struct KeyRecord;
}

#[derive(Debug)]
pub struct RecordLog {
    t_zero: (StdInstant, RtInstant),
    records: SlotMap<KeyRecord, Record>,
}

#[derive(Debug)]
pub struct Recorder<'a> {
    log: &'a mut RecordLog,
    parent: Option<KeyRecord>,
    last: Option<KeyRecord>,
}

#[derive(derive_more::Debug)]
pub struct Record {
    pub at: (StdInstant, RtInstant),
    pub parent: Option<KeyRecord>,
    pub previous: Option<KeyRecord>,
    pub kind: RecordKind,

    #[debug(skip)]
    _no_pub_constructor: NoPubConstructor,
}

#[derive(Debug)]
pub enum RecordKind {
    CallFireEvent(),
}

impl RecordLog {
    pub fn new() -> Self {
        let t_zero = (StdInstant::now(), RtInstant::now());
        Self {
            t_zero,
            records: Default::default(),
        }
    }

    pub fn recorder(&mut self) -> Recorder {
        Recorder {
            log: self,
            parent: None,
            last: None,
        }
    }
}

impl<'a> Recorder<'a> {
    pub fn write(&'a mut self, entry: impl Into<RecordKind>) -> Self {
        let at = (StdInstant::now(), RtInstant::now());
        let kind = entry.into();
        let record = Record {
            at,
            parent: self.parent,
            previous: self.last,
            kind,

            _no_pub_constructor: NoPubConstructor,
        };
        let key = self.log.records.insert(record);
        self.last = Some(key);
        Self {
            log: self.log,
            parent: Some(key),
            last: None,
        }
    }
}

struct NoPubConstructor;
