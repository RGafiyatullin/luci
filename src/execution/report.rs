use std::{collections::HashMap, io};

use crate::{
    execution::{Executable, SourceCode},
    names::EventName,
    recorder::{KeyRecord, RecordKind, RecordLog},
    scenario::RequiredToBe,
};

#[derive(Debug, Clone)]
pub struct Report {
    pub reached: HashMap<EventName, RequiredToBe>,
    pub unreached: HashMap<EventName, RequiredToBe>,
    pub record_log: RecordLog,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.reached
            .iter()
            .all(|(_, r)| matches!(r, RequiredToBe::Reached))
            && self
                .unreached
                .iter()
                .all(|(_, r)| matches!(r, RequiredToBe::Unreached))
    }
    pub fn message(&self) -> String {
        let r_r = self
            .reached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Reached))
            .count();
        let r_u = self
            .reached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Unreached))
            .count();
        let u_r = self
            .unreached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Reached))
            .count();
        let u_u = self
            .unreached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Unreached))
            .count();

        let mut out = format!(
            r#"
Reached:
    Ok:  {r_r}
    Err: {r_u}
Unreached:
    Ok:  {u_u}
    Err: {u_r}
"#
        );

        for (e, _) in self
            .unreached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Reached))
        {
            out.push_str(format!("! unreached {}\n", { e }).as_str());
        }
        for (e, _) in self
            .reached
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Unreached))
        {
            out.push_str(format!("! reached   {}\n", { e }).as_str());
        }

        out
    }

    pub fn dump_record_log(
        &self,
        mut io: impl std::io::Write,
        _sources: &SourceCode,
        executable: &Executable,
    ) -> Result<(), io::Error> {
        use std::io::Write;

        fn dump<'a>(
            io: &mut impl Write,
            depth: usize,
            last_kind: &mut Option<&'a RecordKind>,
            log: &'a RecordLog,
            this_key: KeyRecord,
            executable: &Executable,
        ) -> Result<(), io::Error> {
            let record = &log.records[this_key];

            if last_kind.is_some_and(|k| k == &record.kind) && record.children.is_empty() {
                return Ok(());
            }
            *last_kind = Some(&record.kind);

            write!(io, "{:1$}", "", depth)?;

            writeln!(
                io,
                "{}",
                display::DisplayRecord {
                    record,
                    log,
                    executable,
                }
            )?;

            for child_key in record.children.iter().copied() {
                dump(io, depth + 1, last_kind, log, child_key, executable)?;
            }

            Ok(())
        }

        let mut last_kind = None;
        for root_key in self.record_log.roots.iter().copied() {
            writeln!(io, "ROOT: {:?}", root_key)?;
            dump(
                &mut io,
                0,
                &mut last_kind,
                &self.record_log,
                root_key,
                executable,
            )?;
        }

        Ok(())
    }
}

mod display {
    use std::fmt;

    use crate::execution::runner::ReadyEventKey;
    use crate::execution::Executable;
    use crate::recorder::records as r;
    use crate::recorder::Record;
    use crate::recorder::RecordKind;
    use crate::recorder::RecordLog;
    use crate::scenario::SrcMsg;

    pub(super) struct DisplayRecord<'a> {
        pub(super) record: &'a Record,
        pub(super) log: &'a RecordLog,
        pub(super) executable: &'a Executable,
    }

    impl<'a> fmt::Display for DisplayRecord<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let Self {
                record,
                log,
                executable,
            } = self;
            let (t0_wall, t0_rt) = log.t_zero;
            let (t_wall, t_rt) = record.at;
            let kind = &record.kind;

            let dt_wall = t_wall.duration_since(t0_wall);
            let dt_rt = t_rt.duration_since(t0_rt);
            write!(
                f,
                "[wall: {:?}; rt: {:?}] {}",
                dt_wall,
                dt_rt,
                DisplayRecordKind { kind, executable }
            )
        }
    }

    pub(super) struct DisplayRecordKind<'a> {
        kind: &'a RecordKind,
        executable: &'a Executable,
    }

    impl<'a> fmt::Display for DisplayRecordKind<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            use RecordKind::*;

            match self.kind {
                ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Bind)) => {
                    write!(f, "requested BIND")
                }
                ProcessEventClass(r::ProcessEventClass(ReadyEventKey::RecvOrDelay)) => {
                    write!(f, "requested RECV or DELAY")
                }
                ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Send(k))) => {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, "requested SEND: {} (@{:?})", event, scope)
                }
                ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Respond(k))) => {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, "requested RESP: {} (@{:?})", event, scope)
                }

                ReadyBindKeys(r::ReadyBindKeys(ks)) => {
                    write!(f, "ready binds: [")?;
                    for k in ks {
                        let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                        write!(f, " {}(@{:?}) ", event, scope)?;
                    }
                    write!(f, "]")
                }
                ReadyRecvKeys(r::ReadyRecvKeys(ks)) => {
                    write!(f, "ready recvs: [")?;
                    for k in ks {
                        let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                        write!(f, " {}(@{:?}) ", event, scope)?;
                    }
                    write!(f, "]")
                }
                TimedOutRecvKey(r::TimedOutRecvKey(k)) => {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, "timed out RECV: {} (@{:?})", event, scope)
                }

                ProcessBindKey(r::ProcessBindKey(k)) => {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, "process bind {} (@{:?})", event, scope)
                }
                ProcessSend(r::ProcessSend(k)) => write!(f, "process send {:?}", k),
                ProcessRespond(r::ProcessRespond(k)) => write!(f, "process resp {:?}", k),

                BindSrcScope(r::BindSrcScope(k)) => write!(f, "src scope {:?}", k),
                BindDstScope(r::BindDstScope(k)) => write!(f, "dst scope {:?}", k),

                UsingMsg(r::UsingMsg(SrcMsg::Inject(name))) => write!(f, "msg.inj {:?}", name),
                UsingMsg(r::UsingMsg(SrcMsg::Literal(json))) => {
                    write!(f, "msg.lit: {}", serde_json::to_string(&json).unwrap())
                }
                UsingMsg(r::UsingMsg(SrcMsg::Bind(bind))) => {
                    write!(f, "msg.bind: {}", serde_json::to_string(&bind).unwrap())
                }

                BindToPattern(r::BindToPattern(pattern)) => {
                    write!(f, "pattern: {}", serde_json::to_string(pattern).unwrap())
                }
                BindValue(r::BindValue(json)) => {
                    write!(f, "value: {}", serde_json::to_string(json).unwrap())
                }

                EventFired(r::EventFired(k)) => {
                    let (scope, event) = self.executable.event_name(*k).unwrap();
                    write!(f, "completed {} (@{:?})", event, scope)
                }

                BindActorName(r::BindActorName(name, addr, true)) => {
                    write!(f, "SET {} = {}", name, addr)
                }
                BindActorName(r::BindActorName(name, addr, false)) => {
                    write!(f, "NOT SET {} = {}", name, addr)
                }
                ResolveActorName(r::ResolveActorName(name, addr)) => {
                    write!(f, "resolve {} = {}", name, addr)
                }

                SendMessageType(r::SendMessageType(fqn)) => write!(f, "send {}", fqn),
                SendTo(r::SendTo(None)) => write!(f, "routed"),
                SendTo(r::SendTo(Some(addr))) => write!(f, "to:{}", addr),

                BindOutcome(r::BindOutcome(true)) => write!(f, "BOUND"),
                BindOutcome(r::BindOutcome(false)) => write!(f, "NOT BOUND"),

                EnvelopeReceived(r::EnvelopeReceived {
                    message_name,
                    from,
                    to_opt,
                }) => {
                    if let Some(to) = to_opt {
                        write!(f, "received {} from {} to {}", message_name, from, to)
                    } else {
                        write!(f, "received {} from {} routed", message_name, from)
                    }
                }

                MatchingRecv(r::MatchingRecv(k)) => {
                    let (scope, event) = self.executable.event_name((*k).into()).unwrap();
                    write!(f, "matching RECV: {} (@{:?})", event, scope)
                }

                ExpectedDirectedGotRouted(r::ExpectedDirectedGotRouted(name)) => {
                    write!(f, "expected directed to {}, got routed", name)
                }

                Root => write!(f, "ROOT"),
                Error(r::Error { reason }) => write!(f, "{}", reason),
            }
        }
    }
}
