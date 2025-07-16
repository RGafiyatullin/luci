use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use slotmap::SlotMap;
use tracing::{debug, trace};

use crate::{
    execution::{EventBind, EventKey, KeyBind, KeyDelay, KeyRecv, KeyRespond, KeySend},
    marshalling,
    scenario::{DefEventBind, DefEventDelay, DefEventRecv, DefEventRespond, DefEventSend},
};
use crate::{
    execution::{EventDelay, EventRecv, EventRespond, EventSend, Events, Executable},
    marshalling::MarshallingRegistry,
    names::{ActorName, EventName, MessageName},
    scenario::{DefEvent, DefEventKind, DefTypeAlias, Scenario},
};

#[derive(Debug, thiserror::Error)]
pub enum BuildError<'a> {
    #[error("unknown event: {}", _0)]
    UnknownEvent(&'a EventName),

    #[error("duplicate event: {}", _0)]
    DuplicateEventName(&'a EventName),

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
    InvalidData(marshalling::AnError),
}

impl Executable {
    pub fn build<'s>(
        scenario: &'s Scenario,
        marshalling: Option<&MarshallingRegistry>,
    ) -> Result<Self, BuildError<'s>> {
        debug!("building...");

        debug!("storing type-aliases...");
        let type_aliases = type_aliases(marshalling, &scenario.types)?;
        for (a, fqn) in &type_aliases {
            trace!("- {:?} -> {:?}", a, fqn);
        }

        debug!("checking actor-names...");
        let actors = validate_actor_names(&scenario.cast)?;
        for actor_name in &actors {
            trace!("- {:?}", actor_name);
        }

        debug!("building the graph...");
        let vertices = build_graph(&scenario.events, &type_aliases, &actors, marshalling)?;

        debug!("- bind-vertices:\t{}", vertices.bind.len());
        debug!("- send-vertices:\t{}", vertices.send.len());
        debug!("- recv-vertices:\t{}", vertices.recv.len());
        debug!("- respond-vertices:\t{}", vertices.respond.len());
        debug!("- delay-vertices:\t{}", vertices.delay.len());

        debug!("done!");
        Ok(Executable { events: vertices })
    }
}

fn type_aliases<'a>(
    marshalling: Option<&MarshallingRegistry>,
    imports: impl IntoIterator<Item = &'a DefTypeAlias>,
) -> Result<HashMap<MessageName, Arc<str>>, BuildError<'a>> {
    use std::collections::hash_map::Entry::Vacant;
    let mut aliases = HashMap::new();
    for import in imports {
        let Vacant(entry) = aliases.entry(import.type_alias.to_owned()) else {
            return Err(BuildError::DuplicateAlias(&import.type_alias));
        };
        if let Some(marshalling) = marshalling {
            let _marshaller = marshalling
                .resolve(&import.type_name)
                .ok_or(BuildError::UnknownFqn(&import.type_name))?;
        }

        entry.insert(import.type_name.as_str().into());
    }

    Ok(aliases)
}

fn validate_actor_names<'a>(
    actor_names: impl IntoIterator<Item = &'a ActorName>,
) -> Result<HashSet<ActorName>, BuildError<'a>> {
    let mut out = HashSet::new();

    for name in actor_names {
        if !out.insert(name.clone()) {
            return Err(BuildError::DuplicateActorName(name));
        }
    }

    Ok(out)
}

fn build_graph<'a>(
    event_defs: impl IntoIterator<Item = &'a DefEvent>,
    type_aliases: &HashMap<MessageName, Arc<str>>,
    actors: &HashSet<ActorName>,
    marshalling: Option<&MarshallingRegistry>,
) -> Result<Events, BuildError<'a>> {
    let mut v_delay = SlotMap::<KeyDelay, _>::default();
    let mut v_bind = SlotMap::<KeyBind, _>::default();
    let mut v_recv = SlotMap::<KeyRecv, _>::default();
    let mut v_send = SlotMap::<KeySend, _>::default();
    let mut v_respond = SlotMap::<KeyRespond, _>::default();

    let mut entry_points = BTreeSet::new();
    let mut key_unblocks_values = HashMap::<_, BTreeSet<_>>::new();
    let mut required = HashMap::new();
    let mut idx_keys = HashMap::new();

    let mut priority = vec![];

    for event in event_defs {
        debug!(" processing event[{:?}]...", event.id);

        let this_name = &event.id;
        let prerequisites =
            resolve_event_ids(&idx_keys, &event.prerequisites).collect::<Result<Vec<_>, _>>()?;

        let this_key = match &event.kind {
            DefEventKind::Delay(def_delay) => {
                let DefEventDelay {
                    delay_for,
                    delay_step,
                    no_extra: _,
                } = def_delay;
                let delay_for = *delay_for;
                let delay_step = *delay_step;

                let key = v_delay.insert(EventDelay {
                    delay_for,
                    delay_step,
                });
                EventKey::Delay(key)
            }

            DefEventKind::Bind(def_bind) => {
                let DefEventBind {
                    dst,
                    src,
                    no_extra: _,
                } = def_bind;
                let dst = dst.clone();
                let src = src.clone();
                let key = v_bind.insert(EventBind { dst, src });

                EventKey::Bind(key)
            }
            DefEventKind::Recv(def_recv) => {
                let DefEventRecv {
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

                for actor_name in to.as_ref().into_iter().chain(from) {
                    if !actors.contains(actor_name) {
                        return Err(BuildError::UnknownActor(actor_name));
                    }
                }

                let key = v_recv.insert(EventRecv {
                    from: from.clone(),
                    to: to.clone(),
                    fqn: type_fqn,
                    payload: message_data.clone(),
                });
                EventKey::Recv(key)
            }
            DefEventKind::Send(def_send) => {
                let DefEventSend {
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

                for actor_name in to.as_ref().into_iter().chain([from]) {
                    if !actors.contains(&actor_name) {
                        return Err(BuildError::UnknownActor(&actor_name));
                    }
                }

                let key = v_send.insert(EventSend {
                    from: from.clone(),
                    to: to.clone(),
                    fqn: type_fqn,
                    payload: message_data.clone(),
                });
                EventKey::Send(key)
            }
            DefEventKind::Respond(def_respond) => {
                let DefEventRespond {
                    from,
                    to_request: to,
                    data,
                    no_extra: _,
                } = def_respond;

                let causing_event_key = idx_keys.get(&to).ok_or(BuildError::UnknownEvent(&to))?;
                let EventKey::Recv(recv_key) = causing_event_key else {
                    return Err(BuildError::NotARequest(&to));
                };
                let request_fqn = v_recv.get(*recv_key)
                    .expect("we do not delete items from `recv`; neither we store keys that are unrelated to our collections")
                    .fqn.clone();

                if let Some(bad_actor) = from.as_ref().filter(|a| !actors.contains(a)) {
                    return Err(BuildError::UnknownActor(bad_actor));
                }

                if let Some(marshalling) = marshalling {
                    if marshalling
                        .resolve(&request_fqn)
                        .is_none_or(|m| m.response().is_none())
                    {
                        return Err(BuildError::NotARequest(&to));
                    }
                }

                let key = v_respond.insert(EventRespond {
                    respond_to: *recv_key,
                    request_type: request_fqn,
                    respond_from: from.clone(),
                    payload: data.clone(),
                });
                EventKey::Respond(key)
            }
        };

        if let Some(required_to_be) = event.require {
            required.insert(this_key, required_to_be);
        }

        if prerequisites.is_empty() {
            let should_be_a_new_element = entry_points.insert(this_key);
            assert!(
                should_be_a_new_element,
                "non unique entry point? {:?}",
                this_key
            );
        }
        for prerequisite in &prerequisites {
            let should_be_a_new_element = key_unblocks_values
                .entry(*prerequisite)
                .or_default()
                .insert(this_key);

            assert!(
                should_be_a_new_element,
                "duplicate  relation: {:?} unblocks {:?}",
                *prerequisite, this_key
            );
        }

        trace!("  done: {:?} -> {:?}", this_name, this_key);

        priority.push(this_key);
        if idx_keys.insert(this_name, this_key).is_some() {
            return Err(BuildError::DuplicateEventName(&event.id));
        }
    }

    let priority = priority
        .into_iter()
        .enumerate()
        .map(|(p, k)| (k, p))
        .collect();

    let names = idx_keys
        .into_iter()
        .map(|(n, id)| (id, n.to_owned()))
        .collect();

    let vertices = Events {
        priority,
        required,
        names,
        bind: v_bind,
        send: v_send,
        recv: v_recv,
        respond: v_respond,
        delay: v_delay,
        entry_points,
        key_unblocks_values,
    };

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
