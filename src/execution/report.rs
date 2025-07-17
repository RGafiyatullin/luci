use std::{collections::HashMap, io};

use slotmap::SlotMap;

use crate::{
    execution::{Executable, SourceCode},
    names::EventName,
    recorder::{KeyRecord, Record, RecordLog},
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
            records: &SlotMap<KeyRecord, Record>,
            this_key: KeyRecord,
        ) -> Result<(), io::Error> {
            write!(io, "{:1$}", "", depth)?;
            writeln!(io, "{:?}", this_key)?;

            let this = &records[this_key];
            for child_key in this.children.iter().copied() {
                dump(io, depth + 1, records, child_key)?;
            }

            Ok(())
        }

        for root_key in self.record_log.roots.iter().copied() {
            writeln!(io, "ROOT: {:?}", root_key)?;
            dump(&mut io, 0, &self.record_log.records, root_key)?;
        }

        Ok(())
    }
}
