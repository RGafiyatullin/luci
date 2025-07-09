use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tracing::{debug, trace};

use crate::{
    execution_graph::{EventKey, VertexBind},
    messages,
    scenario::{EventBind, EventRecv, EventRespond, EventSend},
};
use crate::{
    execution_graph::{
        ExecutionGraph, VertexDelay, VertexRecv, VertexRespond, VertexSend, Vertices,
    },
    messages::Messages,
    scenario::{ActorName, EventDef, EventKind, EventName, MessageName, Scenario, TypeAlias},
};

#[derive(Debug, thiserror::Error)]
pub enum BuildError<'a> {
    #[error("unknown event: {}", _0)]
    UnknownEvent(&'a EventName),

    #[error("not a request: {}", _0)]
    NotARequest(&'a EventName),

    #[error("unknown actor: {}", _0)]
    UnknownActor(&'a ActorName),

    #[error("unknown FQN: {}", _0)]
    UnknownFqn(&'a str),

    #[error("unknown alias: {}", _0)]
    UnknownAlias(&'a MessageName),

    #[error("duplicate alias: {}", _0)]
    DuplicateAlias(&'a MessageName),

    #[error("duplicate actor name: {}", _0)]
    DuplicateActorName(&'a ActorName),

    #[error("invalid data: {}", _0)]
    InvalidData(messages::AnError),
}

#[derive(Debug)]
pub struct Builder {
    messages: Messages,
}

impl ExecutionGraph {
    pub fn builder(messages: Messages) -> Builder {
        debug!("created a builder");
        Builder { messages }
    }
}

impl Builder {
    pub fn build(self, scenario: &Scenario) -> Result<ExecutionGraph, BuildError<'_>> {
        debug!("building...");
        let Self { messages } = self;
        let messages = Arc::new(messages);

        debug!("storing type-aliases...");
        let type_aliases = type_aliases(&messages, &scenario.types)?;
        for (a, fqn) in &type_aliases {
            trace!("- {:?} -> {:?}", a, fqn);
        }

        debug!("checking actor-names...");
        let actors = sanitize_cast(&scenario.cast)?;
        for a in &actors {
            trace!("- {:?}", a);
        }

        debug!("building the graph...");
        let vertices = build_graph(&scenario.events, &type_aliases, &actors, &messages)?;

        debug!("- bind-vertices:\t{}", vertices.bind.len());
        debug!("- send-vertices:\t{}", vertices.send.len());
        debug!("- recv-vertices:\t{}", vertices.recv.len());
        debug!("- respond-vertices:\t{}", vertices.respond.len());
        debug!("- delay-vertices:\t{}", vertices.delay.len());

        debug!("done!");
        Ok(ExecutionGraph { messages, vertices })
    }
}

fn type_aliases<'a>(
    messages: &Messages,
    imports: impl IntoIterator<Item = &'a TypeAlias>,
) -> Result<HashMap<MessageName, Arc<str>>, BuildError<'a>> {
    use std::collections::hash_map::Entry::Vacant;
    let mut aliases = HashMap::new();
    for import in imports {
        let Vacant(entry) = aliases.entry(import.type_alias.to_owned()) else {
            return Err(BuildError::DuplicateAlias(&import.type_alias));
        };
        let _marshaller = messages
            .resolve(&import.type_name)
            .ok_or(BuildError::UnknownFqn(&import.type_name))?;

        entry.insert(import.type_name.as_str().into());
    }

    Ok(aliases)
}

fn sanitize_cast<'a>(
    cast: impl IntoIterator<Item = &'a ActorName>,
) -> Result<HashSet<ActorName>, BuildError<'a>> {
    let mut out = HashSet::new();

    for a in cast {
        if !out.insert(a.clone()) {
            return Err(BuildError::DuplicateActorName(a));
        }
    }

    Ok(out)
}

fn build_graph<'a>(
    event_defs: impl IntoIterator<Item = &'a EventDef>,
    type_aliases: &HashMap<MessageName, Arc<str>>,
    actors: &HashSet<ActorName>,
    _messages: &Messages,
) -> Result<Vertices, BuildError<'a>> {
    let mut vertices: Vertices = Default::default();

    let mut idx_keys = HashMap::new();

    let mut priority = vec![];

    for event in event_defs {
        debug!(" processing event[{:?}]...", event.id);

        let this_name = &event.id;
        let after = resolve_event_ids(&idx_keys, &event.after).collect::<Result<Vec<_>, _>>()?;

        let this_key = match &event.kind {
            EventKind::Delay(duration) => {
                let key = vertices.delay.insert(VertexDelay(*duration));
                EventKey::Delay(key)
            }

            EventKind::Bind(def_bind) => {
                let EventBind {
                    dst,
                    src,
                    no_extra: _,
                } = def_bind;
                let dst = dst.clone();
                let src = src.clone();
                let key = vertices.bind.insert(VertexBind { dst, src });

                EventKey::Bind(key)
            }
            EventKind::Recv(def_recv) => {
                let EventRecv {
                    message_type,
                    message_data,
                    from,
                    to,
                    no_extra: _,
                } = def_recv;

                let type_fqn = type_aliases
                    .get(message_type)
                    .cloned()
                    .ok_or(BuildError::UnknownAlias(&message_type))?;

                for a in to.as_ref().into_iter().chain(from) {
                    if !actors.contains(a) {
                        return Err(BuildError::UnknownActor(a));
                    }
                }

                let key = vertices.recv.insert(VertexRecv {
                    match_from: from.clone(),
                    match_to: to.clone(),
                    match_type: type_fqn,
                    match_message: message_data.clone(),
                });
                EventKey::Recv(key)
            }
            EventKind::Send(def_send) => {
                let EventSend {
                    from,
                    to,
                    message_type,
                    message_data,
                    no_extra: _,
                } = def_send;

                let type_fqn = type_aliases
                    .get(message_type)
                    .cloned()
                    .ok_or(BuildError::UnknownAlias(message_type))?;

                for a in to.as_ref().into_iter().chain([from]) {
                    if !actors.contains(&a) {
                        return Err(BuildError::UnknownActor(&a));
                    }
                }
                // let marshaller = messages.resolve(&type_fqn).expect("an invalid fqn");
                // let _ = marshaller
                //     .marshall(&Default::default(), def_send.message_data.clone())
                //     .map_err(BuildError::InvalidData)?;

                let key = vertices.send.insert(VertexSend {
                    send_from: from.clone(),
                    send_to: to.clone(),
                    message_type: type_fqn,
                    // TODO: try actually marshalling this value using this `type_fqn`.
                    message_data: message_data.clone(),
                });
                EventKey::Send(key)
            }
            EventKind::Respond(def_respond) => {
                let EventRespond {
                    from,
                    to,
                    data,
                    no_extra: _,
                } = def_respond;

                let causing_event_key = idx_keys.get(&to).ok_or(BuildError::UnknownEvent(&to))?;
                let EventKey::Recv(recv_key) = causing_event_key else {
                    return Err(BuildError::NotARequest(&to));
                };
                let request_fqn = vertices
                    .recv.get(*recv_key)
                    .expect("we do not delete items from `recv`; neither we store keys that are unrelated to our collections")
                    .match_type.clone();

                // TODO: 1. Check whether the `request_fqn` is a request.
                // TODO: 2. Try actually marshalling this value using `request_fqn`.

                if let Some(bad_actor) = from.as_ref().filter(|a| !actors.contains(a)) {
                    return Err(BuildError::UnknownActor(bad_actor));
                }

                let key = vertices.respond.insert(VertexRespond {
                    respond_to: *recv_key,
                    request_fqn,
                    respond_from: from.clone(),
                    message_data: data.clone(),
                });
                EventKey::Respond(key)
            }
        };

        if let Some(required_to_be) = event.require {
            vertices.required.insert(this_key, required_to_be);
        }

        if after.is_empty() {
            let should_be_a_new_element = vertices.entry_points.insert(this_key);
            assert!(
                should_be_a_new_element,
                "non unique entry point? {:?}",
                this_key
            );
        }
        for prerequisite in &after {
            let should_be_a_new_element = vertices
                .key_unblocks_values
                .entry(*prerequisite)
                .or_default()
                .insert(this_key);

            assert!(
                should_be_a_new_element,
                "duplicate unblocks relation? {:?} -> {:?}",
                *prerequisite, this_key
            );
        }

        trace!("  done: {:?} -> {:?}", this_name, this_key);

        priority.push(this_key);
        idx_keys.insert(this_name, this_key);
    }

    vertices.priority = priority
        .into_iter()
        .enumerate()
        .map(|(p, k)| (k, p))
        .collect();

    vertices.names = idx_keys
        .into_iter()
        .map(|(n, id)| (id, n.to_owned()))
        .collect();

    Ok(vertices)
}

fn resolve_event_ids<'def, 'tmp>(
    idx_keys: &'tmp HashMap<&'def EventName, EventKey>,
    names: &'def [EventName],
) -> impl Iterator<Item = Result<EventKey, BuildError<'def>>> + 'tmp
where
    'def: 'tmp,
{
    names.into_iter().map(move |name: &'def EventName| {
        idx_keys
            .get(name)
            .copied()
            .ok_or(BuildError::UnknownEvent(name))
    })
}
