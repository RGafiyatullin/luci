use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{names::EventName, scenario::RequiredToBe};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub reached: HashMap<EventName, RequiredToBe>,
    pub unreached: HashMap<EventName, RequiredToBe>,
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
}
