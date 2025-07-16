use std::collections::HashSet;

use crate::{
    graphics::{OutputFormat, RenderGraph},
    scenario::DefEventKind,
};

use super::{DefEvent, Scenario};

impl RenderGraph for Scenario {
    const OUTPUT: OutputFormat = OutputFormat::Graphviz;

    fn render(&self) -> String {
        let mut acc = String::new();
        acc.push_str("digraph test { rankdir=LR layout=dot\n");

        let mut seen_ids = HashSet::new();
        self.events
            .iter()
            .filter(|event| seen_ids.insert(event.id.clone()))
            .cloned()
            .collect::<Vec<DefEvent>>()
            .into_iter()
            .for_each(|event| {
                acc.push_str(&format!("  {}", render_node(&event)));
                acc.push('\n');
            });

        for event in &self.events {
            for subnode_name in &event.prerequisites {
                acc.push_str(&format!("  \"{}\" -> \"{}\"\n", subnode_name, event.id));
            }
        }

        acc.push_str("\n}\n");
        acc
    }
}

fn render_node(event: &DefEvent) -> String {
    let (kind, data) = match &event.kind {
        DefEventKind::Bind(bind) => ("BIND", serde_yaml::to_string(&bind).unwrap()),
        DefEventKind::Recv(recv) => ("RECV", serde_yaml::to_string(&recv).unwrap()),
        DefEventKind::Send(send) => ("SEND", serde_yaml::to_string(&send).unwrap()),
        DefEventKind::Respond(respond) => ("RESPOND", serde_yaml::to_string(&respond).unwrap()),
        DefEventKind::Delay(delay) => ("DELAY", serde_yaml::to_string(&delay).unwrap()),
    };

    format!(
        r#""{}" [label="{}\nid={}\n\n{}"]"#,
        event.id, kind, event.id, data,
    )
}
