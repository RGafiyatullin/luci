use std::collections::{HashMap, HashSet};
use std::{fmt, io};

use crate::execution::{display, EventKey, Executable, SourceCode};
use crate::recorder::{KeyRecord, RecordKind, RecordLog};
use crate::scenario::RequiredToBe;

#[derive(Debug, Clone)]
pub struct Report {
    pub reached_events:  HashSet<EventKey>,
    pub required_events: HashMap<EventKey, RequiredToBe>,
    pub record_log:      RecordLog,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        let reached_necessary = self
            .required_events
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Reached))
            .all(|(e, _)| self.reached_events.contains(e));
        let avoided_restricted = self
            .required_events
            .iter()
            .filter(|(_, r)| matches!(r, RequiredToBe::Unreached))
            .all(|(e, _)| !self.reached_events.contains(e));

        reached_necessary && avoided_restricted
    }

    pub fn message<'a>(
        &'a self,
        executable: &'a Executable,
        source_code: &'a SourceCode,
    ) -> impl fmt::Display + 'a {
        display::DisplayReport {
            report: self,
            executable,
            source_code,
        }
    }

    pub fn dump_record_log(
        &self,
        mut io: impl std::io::Write,
        source_code: &SourceCode,
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
            source_code: &SourceCode,
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
                    source_code,
                }
            )?;

            for child_key in record.children.iter().copied() {
                dump(
                    io,
                    depth + 1,
                    last_kind,
                    log,
                    child_key,
                    executable,
                    source_code,
                )?;
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
                source_code,
            )?;
        }

        Ok(())
    }
}
