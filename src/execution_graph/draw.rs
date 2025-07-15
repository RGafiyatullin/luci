use std::collections::HashSet;

use super::{EventKey, VertexBind, VertexDelay, VertexRecv, VertexRespond, VertexSend, Vertices};

pub trait DrawDot {
    fn draw(&self, key: EventKey) -> String;
}

impl DrawDot for VertexDelay {
    fn draw(&self, key: EventKey) -> String {
        format!(
            r#""{:?}" [label="delay {:?} by {:?}"]"#,
            key, self.delay_for, self.delay_step
        )
    }
}

impl DrawDot for VertexBind {
    fn draw(&self, key: EventKey) -> String {
        let src = serde_yaml::to_string(&self.src).unwrap();
        let dst = serde_yaml::to_string(&self.dst).unwrap();
        format!(
            r#""{:?}" [label="bind\nsrc: \n{}\ndst: \n{}"]"#,
            key, src, dst
        )
    }
}

impl DrawDot for VertexRecv {
    fn draw(&self, key: EventKey) -> String {
        let data = serde_yaml::to_string(&self.match_message).unwrap();
        format!(
            r#""{:?}" [label="recv '{}'\nfrom: {}\nto: {}\ndata: {}"]"#,
            key,
            self.match_type,
            self.match_from
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            self.match_to
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            data
        )
    }
}

impl DrawDot for VertexSend {
    fn draw(&self, key: EventKey) -> String {
        let data = serde_yaml::to_string(&self.message_data).unwrap();
        format!(
            r#""{:?}" [label="send '{}'\nfrom: {}\nto: {}\ndata: {}"]"#,
            key,
            self.message_type,
            self.send_from,
            self.send_to
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
            data
        )
    }
}

impl DrawDot for VertexRespond {
    fn draw(&self, key: EventKey) -> String {
        format!(
            r#""{:?}" [label="respond '{}'\nfrom: {}"]"#,
            key,
            self.request_fqn,
            self.respond_from
                .clone()
                .map(|actor| actor.to_string())
                .unwrap_or_default(),
        )
    }
}

impl Vertices {
    pub fn draw_graphviz(&self) -> String {
        let mut acc = String::new();
        acc.push_str("digraph test { rankdir=LR layout=dot\n");

        self.entry_points
            .iter()
            .chain(self.key_unblocks_values.values().flatten())
            .cloned()
            .collect::<HashSet<EventKey>>() // deduplicate
            .iter()
            .for_each(|key| {
                self.draw_node(&mut acc, &key);
            });

        for (parent, children) in &self.key_unblocks_values {
            for child in children {
                acc.push_str(&format!(r#"  "{:?}" -> "{:?}""#, parent, child));
            }
        }

        acc.push_str("}\n");
        acc
    }

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
