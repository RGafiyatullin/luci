use std::collections::HashMap;
use std::io;

use crate::execution::{display, Executable, SourceCode};
use crate::names::EventName;
use crate::recorder::{KeyRecord, RecordKind, RecordLog};
use crate::scenario::RequiredToBe;

#[derive(Debug, Clone)]
pub struct Report {
    pub reached:    HashMap<EventName, RequiredToBe>,
    pub unreached:  HashMap<EventName, RequiredToBe>,
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
