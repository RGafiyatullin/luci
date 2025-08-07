use std::fmt;

use slotmap::SlotMap;

use crate::execution::build::{BuildError, BuildErrorReason};
use crate::execution::runner::ReadyEventKey;
use crate::execution::sources::SingleScenarioSource;
use crate::execution::{Executable, KeyScenario, KeyScope, ScopeInfo, SourceCode};
use crate::recorder::{records as r, Record, RecordKind, RecordLog};
use crate::scenario::SrcMsg;

pub(super) struct DisplayRecord<'a> {
    pub(super) record:      &'a Record,
    pub(super) log:         &'a RecordLog,
    pub(super) executable:  &'a Executable,
    pub(super) source_code: &'a SourceCode,
}

impl<'a> fmt::Display for DisplayRecord<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            record,
            log,
            executable,
            source_code,
        } = self;
        let (t0_wall, t0_rt) = log.t_zero;
        let (t_wall, t_rt) = record.at;
        let kind = &record.kind;

        let dt_wall = t_wall.duration_since(t0_wall);
        let dt_rt = t_rt.duration_since(t0_rt);
        write!(
            f,
            "[wall: {:>8?}; rt: {:>8?}] {}",
            dt_wall,
            dt_rt,
            DisplayRecordKind {
                kind,
                executable,
                source_code,
            }
        )
    }
}

impl<'a> fmt::Display for BuildError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BuildErrorReason::*;

        let Self {
            reason,
            scopes,
            sources,
        } = self;

        let scope = *match reason {
            UnknownEvent(_, k) => k,
            NotARequest(_, k) => k,
            UnknownActor(_, k) => k,
            UnknownDummy(_, k) => k,
            UnknownSubroutine(_, k) => k,
            UnknownFqn(_, k) => k,
            UnknownAlias(_, k) => k,
            DuplicateAlias(_, k) => k,
            DuplicateEventName(_, k) => k,
            DuplicateActorName(_, k) => k,
            DuplicateDummyName(_, k) => k,
        };

        write!(f, "{} (", reason)?;
        fmt_scope_recursively(f, scope, scopes, sources)?;
        write!(f, ")")
    }
}

impl<'a> fmt::Debug for BuildError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

pub(super) struct DisplayRecordKind<'a> {
    kind:        &'a RecordKind,
    executable:  &'a Executable,
    source_code: &'a SourceCode,
}

struct DisplayScope<'a> {
    scope:       KeyScope,
    executable:  &'a Executable,
    source_code: &'a SourceCode,
}

impl<'a> DisplayRecordKind<'a> {
    fn scope(&self, scope: KeyScope) -> DisplayScope<'a> {
        DisplayScope {
            scope,
            executable: self.executable,
            source_code: self.source_code,
        }
    }
}

impl<'a> fmt::Display for DisplayScope<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_scope_recursively(
            f,
            self.scope,
            &self.executable.scopes,
            &self.source_code.sources,
        )
    }
}

impl<'a> fmt::Display for DisplayRecordKind<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use RecordKind::*;

        match self.kind {
            ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Bind)) => {
                write!(f, "\x1b[90mrequested BIND\x1b[0m")
            },
            ProcessEventClass(r::ProcessEventClass(ReadyEventKey::RecvOrDelay)) => {
                write!(f, "\x1b[90mrequested RECV or DELAY\x1b[0m")
            },
            ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Send(k))) => {
                let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                write!(
                    f,
                    "\x1b[90mrequested SEND: {} ({})\x1b[0m",
                    event,
                    self.scope(scope)
                )
            },
            ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Respond(k))) => {
                let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                write!(
                    f,
                    "\x1b[90mrequested RESP: {} ({})\x1b[0m",
                    event,
                    self.scope(scope)
                )
            },

            ReadyBindKeys(r::ReadyBindKeys(ks)) => {
                write!(f, "\x1b[90mready binds: [")?;
                for k in ks {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, " {}({}) ", event, self.scope(scope))?;
                }
                write!(f, "]\x1b[0m")
            },
            ReadyRecvKeys(r::ReadyRecvKeys(ks)) => {
                write!(f, "\x1b[90mready recvs: [")?;
                for k in ks {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, " {}({}) ", event, self.scope(scope))?;
                }
                write!(f, "]\x1b[0m")
            },
            TimedOutRecvKey(r::TimedOutRecvKey(k)) => {
                let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                write!(
                    f,
                    "\x1b[31mtimed out RECV: {} \x1b[0m({})",
                    event,
                    self.scope(scope)
                )
            },

            ProcessBindKey(r::ProcessBindKey(k)) => {
                let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                write!(f, "process bind {} ({})", event, self.scope(scope))
            },
            ProcessSend(r::ProcessSend(k)) => write!(f, "process send {:?}", k),
            ProcessRespond(r::ProcessRespond(k)) => write!(f, "process resp {:?}", k),

            BindSrcScope(r::BindSrcScope(k)) => {
                write!(f, "\x1b[92msrc scope\x1b[0m {}", self.scope(*k))
            },
            BindDstScope(r::BindDstScope(k)) => {
                write!(f, "\x1b[92mdst scope\x1b[0m {}", self.scope(*k))
            },

            MatchActorAddress(r::MatchActorAddress(ka, ks, exp, act)) if exp == act => {
                let actor_name = &self.executable.actors[*ka].known_as[*ks];
                write!(
                    f,
                    "\x1b[32mMATCH ACTOR {} = {}\x1b[0m {}",
                    exp,
                    actor_name,
                    self.scope(*ks)
                )
            },
            MatchActorAddress(r::MatchActorAddress(ka, ks, exp, act)) => {
                let actor_name = &self.executable.actors[*ka].known_as[*ks];
                write!(
                    f,
                    "\x1b[33mMISMATCH ACTOR exp={}, act={}; {}\x1b[0m {}",
                    exp,
                    act,
                    actor_name,
                    self.scope(*ks)
                )
            },
            StoreActorAddress(r::StoreActorAddress(ka, ks, addr)) => {
                let actor_name = &self.executable.actors[*ka].known_as[*ks];
                write!(
                    f,
                    "\x1b[32mSET actor name {} = {} \x1b[0m {}",
                    addr,
                    actor_name,
                    self.scope(*ks)
                )
            },
            ResolveActorName(r::ResolveActorName(ka, ks, addr)) => {
                let actor_name = &self.executable.actors[*ka].known_as[*ks];
                write!(
                    f,
                    "resolve actor {} = {} {}",
                    addr,
                    actor_name,
                    self.scope(*ks)
                )
            },

            MatchDummyAddress(r::MatchDummyAddress(kd, ks, exp, act)) if exp == act => {
                let dummy_name = &self.executable.dummies[*kd].known_as[*ks];
                write!(
                    f,
                    "\x1b[32mMATCH DUMMY {} = {}\x1b[0m {}",
                    exp,
                    dummy_name,
                    self.scope(*ks)
                )
            },
            MatchDummyAddress(r::MatchDummyAddress(kd, ks, exp, act)) => {
                let dummy_name = &self.executable.dummies[*kd].known_as[*ks];
                write!(
                    f,
                    "\x1b[33mMISMATCH DUMMY exp={}, act={}; {}\x1b[0m {}",
                    exp,
                    act,
                    dummy_name,
                    self.scope(*ks)
                )
            },

            UsingMsg(r::UsingMsg(SrcMsg::Inject(name))) => write!(f, "msg.inj {:?}", name),
            UsingMsg(r::UsingMsg(SrcMsg::Literal(json))) => {
                write!(f, "msg.lit: {}", serde_json::to_string(&json).unwrap())
            },
            UsingMsg(r::UsingMsg(SrcMsg::Bind(bind))) => {
                write!(f, "msg.bind: {}", serde_json::to_string(&bind).unwrap())
            },

            BindToPattern(r::BindToPattern(pattern)) => {
                write!(f, "pattern: {}", serde_json::to_string(pattern).unwrap())
            },
            UsingValue(r::UsingValue(json)) => {
                write!(
                    f,
                    "\x1b[34mvalue: {}\x1b[0m",
                    serde_json::to_string(json).unwrap()
                )
            },
            NewBinding(r::NewBinding(key, value)) => {
                write!(
                    f,
                    "\x1b[32mSET {} = {}\x1b[0m",
                    key,
                    serde_json::to_string(value).unwrap()
                )
            },

            EventFired(r::EventFired(k)) => {
                let (scope, event) = self.executable.event_name(*k).unwrap();
                write!(
                    f,
                    "\x1b[1;32mcompleted {} \x1b[0m({})",
                    event,
                    self.scope(scope)
                )
            },

            SendMessageType(r::SendMessageType(fqn)) => {
                write!(f, "\x1b[36msend {}\x1b[0m", fqn)
            },
            SendTo(r::SendTo(None)) => write!(f, "\x1b[36mrouted\x1b[0m"),
            SendTo(r::SendTo(Some(addr))) => write!(f, "\x1b[36mto:{}\x1b[0m", addr),

            BindOutcome(r::BindOutcome(true)) => write!(f, "\x1b[1;32mBOUND\x1b[0m"),
            BindOutcome(r::BindOutcome(false)) => write!(f, "\x1b[33mNOT BOUND\x1b[0m"),

            EnvelopeReceived(r::EnvelopeReceived {
                message_name,
                from,
                to_opt,
            }) => {
                if let Some(to) = to_opt {
                    write!(
                        f,
                        "\x1b[35mreceived {} \x1b[1mfrom {} to {}\x1b[0m",
                        message_name, from, to
                    )
                } else {
                    write!(
                        f,
                        "\x1b[35mreceived {} \x1b[1mfrom {} routed\x1b[0m",
                        message_name, from
                    )
                }
            },

            MatchingRecv(r::MatchingRecv(k)) => {
                let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                write!(f, "matching RECV: {} ({})", event, self.scope(scope))
            },

            ExpectedDirectedGotRouted(r::ExpectedDirectedGotRouted(name)) => {
                write!(f, "expected directed to {:?}, got routed", name)
            },

            Root => write!(f, "ROOT"),
            Error(r::Error { reason }) => write!(f, "{}", reason),
            // _fix_me => write!(f, "TODO"),
        }
    }
}

pub(super) fn fmt_scope_recursively(
    f: &mut fmt::Formatter<'_>,
    this_scope_key: KeyScope,
    scopes: &SlotMap<KeyScope, ScopeInfo>,
    sources: &SlotMap<KeyScenario, SingleScenarioSource>,
) -> fmt::Result {
    let this_scope = &scopes[this_scope_key];
    let this_source = &sources[this_scope.source_key].source_file;
    write!(f, "in {:?} ", &this_source)?;

    let mut invoked_as = this_scope.invoked_as.as_ref();
    while let Some((scope, event_name, _subroutine_name)) = invoked_as.take() {
        write!(f, "< {} ", event_name)?;
        invoked_as = scopes[*scope].invoked_as.as_ref();
    }
    Ok(())
}
