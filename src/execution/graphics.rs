use std::collections::HashSet;

use crate::graphics::{OutputFormat, RenderGraph};

use super::{
    EventBind, EventDelay, EventKey, EventRecv, EventRespond, EventSend, Events, Executable,
};

impl RenderGraph for Executable {
    const OUTPUT: OutputFormat = OutputFormat::Graphviz;

    fn render(&self) -> String {
        let mut acc = String::new();
        acc.push_str("digraph test { rankdir=LR layout=dot\n");

        self.events
            .entry_points
            .iter()
            .chain(self.events.key_unblocks_values.values().flatten())
            .cloned()
            .collect::<HashSet<EventKey>>() // deduplicate
            .iter()
            .for_each(|key| {
                self.events.draw_node(&mut acc, &key);
            });

        for (parent, children) in &self.events.key_unblocks_values {
            for child in children {
                acc.push_str(&format!(r#"  "{:?}" -> "{:?}""#, parent, child));
            }
        }

        acc.push_str("}\n");
        acc
    }
}

impl Events {
    fn draw_node(&self, acc: &mut String, key: &EventKey) {
        let node = match key {
            EventKey::Delay(key_delay) => {
                let delay = self.delay.get(*key_delay).unwrap();
                delay.draw(*key)
            }
            EventKey::Bind(key_bind) => {
                let bind = self.bind.get(*key_bind).unwrap();
                bind.draw(*key)
            }
            EventKey::Recv(key_recv) => {
                let recv = self.recv.get(*key_recv).unwrap();
                recv.draw(*key)
            }
            EventKey::Send(key_send) => {
                let send = self.send.get(*key_send).unwrap();
                send.draw(*key)
            }
            EventKey::Respond(key_respond) => {
                let respond = self.respond.get(*key_respond).unwrap();
                respond.draw(*key)
            }
        };
        acc.push_str(&format!("{}", &node));
        acc.push('\n');
    }
}

pub trait DrawDot {
    fn draw(&self, key: EventKey) -> String;
}

impl DrawDot for EventDelay {
    fn draw(&self, key: EventKey) -> String {
        format!(
            r#""{:?}" [label="delay {:?} by {:?}"]"#,
            key, self.delay_for, self.delay_step
        )
    }
}

impl DrawDot for EventBind {
    fn draw(&self, key: EventKey) -> String {
        let src = serde_yaml::to_string(&self.src).unwrap();
        let dst = serde_yaml::to_string(&self.dst).unwrap();
        format!(
            r#""{:?}" [label="bind\nsrc: \n{}\ndst: \n{}"]"#,
            key, src, dst
        )
    }
}

impl DrawDot for EventRecv {
    fn draw(&self, key: EventKey) -> String {
        let data = serde_yaml::to_string(&self.payload).unwrap();
        format!(
            r#""{:?}" [label="recv '{}'\nfrom: {}\nto: {}\ndata: {}"]"#,
            key,
            self.fqn,
            self.from
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            self.to
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            data
        )
    }
}

impl DrawDot for EventSend {
    fn draw(&self, key: EventKey) -> String {
        let data = serde_yaml::to_string(&self.payload).unwrap();
        format!(
            r#""{:?}" [label="send '{}'\nfrom: {}\nto: {}\ndata: {}"]"#,
            key,
            self.fqn,
            self.from,
            self.to
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            data
        )
    }
}

impl DrawDot for EventRespond {
    fn draw(&self, key: EventKey) -> String {
        format!(
            r#""{:?}" [label="respond '{}'\nfrom: {}"]"#,
            key,
            self.request_type,
            self.respond_from
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
        )
    }
}
