use std::{collections::HashMap, io};

use crate::{
    execution::{Executable, SourceCode},
    names::EventName,
    recorder::{KeyRecord, RecordLog},
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
        _executable: &Executable,
    ) -> Result<(), io::Error> {
        use std::io::Write;

        fn dump(
            io: &mut impl Write,
            depth: usize,
            log: &RecordLog,
            this_key: KeyRecord,
        ) -> Result<(), io::Error> {
            write!(io, "{:1$}", "", depth)?;

            let record = &log.records[this_key];
            writeln!(io, "{}", display::DisplayRecord { record, log })?;

            for child_key in record.children.iter().copied() {
                dump(io, depth + 1, log, child_key)?;
            }

            Ok(())
        }

        for root_key in self.record_log.roots.iter().copied() {
            writeln!(io, "ROOT: {:?}", root_key)?;
            dump(&mut io, 0, &self.record_log, root_key)?;
        }

        Ok(())
    }
}

mod display {
    use std::fmt;

    use crate::execution::runner::ReadyEventKey;
    use crate::recorder::records as r;
    use crate::recorder::Record;
    use crate::recorder::RecordKind;
    use crate::recorder::RecordLog;
    use crate::scenario::Msg;

    pub(super) struct DisplayRecord<'a> {
        pub(super) record: &'a Record,
        pub(super) log: &'a RecordLog,
    }

    impl<'a> fmt::Display for DisplayRecord<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let Self { record, log } = self;
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
                DisplayRecordKind { kind }
            )
        }
    }

    pub(super) struct DisplayRecordKind<'a> {
        kind: &'a RecordKind,
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
                    write!(f, "requested SEND:{:?}", k)
                }
                ProcessEventClass(r::ProcessEventClass(ReadyEventKey::Respond(k))) => {
                    write!(f, "requested RESP:{:?}", k)
                }

                ReadyBindKeys(r::ReadyBindKeys(ks)) => write!(f, "ready binds {:?}", ks),
                ReadyRecvKeys(r::ReadyRecvKeys(ks)) => write!(f, "ready recvs {:?}", ks),

                ProcessBindKey(r::ProcessBindKey(k)) => write!(f, "process bind {:?}", k),
                ProcessSend(r::ProcessSend(k)) => write!(f, "process send {:?}", k),
                ProcessRespond(r::ProcessRespond(k)) => write!(f, "process resp {:?}", k),

                BindSrcScope(r::BindSrcScope(k)) => write!(f, "src scope {:?}", k),
                BindDstScope(r::BindDstScope(k)) => write!(f, "dst scope {:?}", k),

                UsingMsg(r::UsingMsg(Msg::Inject(name))) => write!(f, "msg.inj {:?}", name),
                UsingMsg(r::UsingMsg(Msg::Literal(json))) => {
                    write!(f, "msg.lit: {}", serde_json::to_string(&json).unwrap())
                }
                UsingMsg(r::UsingMsg(Msg::Bind(bind))) => {
                    write!(f, "msg.bind: {}", serde_json::to_string(&bind).unwrap())
                }

                BindValue(r::BindValue(json)) => {
                    write!(f, "value: {}", serde_json::to_string(json).unwrap())
                }

                EventFired(r::EventFired(k)) => write!(f, "fired {:?}", k),

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

                Root => write!(f, "ROOT"),
                Error(r::Error { reason }) => write!(f, "{}", reason),
            }
        }
    }
}
