use std::{collections::HashSet, fmt::Display};

use dot_writer::{Attributes, DotWriter, Scope};

use crate::scenario::DefEventKind;

use super::{DefEvent, Scenario};

pub(crate) fn draw(scenario: &Scenario) -> String {
    let mut output_bytes = Vec::new();

    let mut writer = DotWriter::from(&mut output_bytes);
    writer.set_pretty_print(true);

    let mut digraph = writer.digraph();
    digraph.set_rank_direction(dot_writer::RankDirection::LeftRight);

    let mut seen_ids = HashSet::new();
    for event in scenario
        .events
        .iter()
        .filter(|event| seen_ids.insert(event.id.clone()))
        .collect::<Vec<&DefEvent>>()
    {
        draw_node(&mut digraph, &event);
    }

    for event in &scenario.events {
        for subnode_name in &event.prerequisites {
            digraph.edge(quote(subnode_name), quote(&event.id));
        }
    }

    drop(digraph);

    String::from_utf8(output_bytes).unwrap()
}

fn draw_node(digraph: &mut Scope, event: &DefEvent) {
    let mut node = digraph.node_named(quote(&event.id));

    let (kind, data) = match &event.kind {
        DefEventKind::Bind(bind) => ("BIND", serde_yaml::to_string(&bind).unwrap()),
        DefEventKind::Recv(recv) => ("RECV", serde_yaml::to_string(&recv).unwrap()),
        DefEventKind::Send(send) => ("SEND", serde_yaml::to_string(&send).unwrap()),
        DefEventKind::Respond(respond) => ("RESPOND", serde_yaml::to_string(&respond).unwrap()),
        DefEventKind::Delay(delay) => ("DELAY", serde_yaml::to_string(&delay).unwrap()),
    };

    let label = format!(r#"{}\nid={}\n\n{}"#, kind, event.id, data);
    node.set_label(&label);
}

fn quote(str: &impl Display) -> String {
    format!("\"{}\"", str)
}
