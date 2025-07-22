//! This module is responsible for building an [`Executable`] from [`Sources`].
//!

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use serde_json::json;
use slotmap::SlotMap;
use tracing::{debug, trace};

use crate::{
    execution::{
        BindScope, EventBind, EventKey, KeyBind, KeyDelay, KeyRecv, KeyRespond, KeyScenario,
        KeyScope, KeySend, ScopeInfo, SourceCode,
    },
    marshalling,
    names::SubroutineName,
    scenario::{
        DefEventBind, DefEventDelay, DefEventRecv, DefEventRespond, DefEventSend, DstPattern,
        RequiredToBe, SrcMsg,
    },
};
use crate::{
    execution::{EventDelay, EventRecv, EventRespond, EventSend, Events, Executable},
    marshalling::MarshallingRegistry,
    names::{ActorName, EventName, MessageName},
    scenario::{DefEvent, DefEventKind, DefTypeAlias},
};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unknown event: {}", _0)]
    UnknownEvent(EventName),

    #[error("duplicate event: {}", _0)]
    DuplicateEventName(EventName),

    #[error("not a request: {}", _0)]
    NotARequest(EventName),

    #[error("unknown actor: {}", _0)]
    UnknownActor(ActorName),

    #[error("unknown subroutine: {}", _0)]
    UnknownSubroutine(SubroutineName),

    #[error("unknown FQN: {}", _0)]
    UnknownFqn(String),

    #[error("unknown alias: {}", _0)]
    UnknownAlias(MessageName),

    #[error("duplicate alias: {}", _0)]
    DuplicateAlias(MessageName),

    #[error("duplicate actor name: {}", _0)]
    DuplicateActorName(ActorName),

    #[error("invalid data: {}", _0)]
    InvalidData(marshalling::AnError),
}

impl Executable {
    /// Build an executable.
    /// Needs
    /// - [`MarshallingRegistry`] with all the used messages registered;
    /// - [`Sources`] with the loaded scenarios;
    /// - [`KeySource`] specifying the entry point in the sources.
    ///
    pub fn build(
        marshalling: MarshallingRegistry,
        source_code: &SourceCode,
        entry_point_key: KeyScenario,
    ) -> Result<Self, BuildError> {
        debug!("building...");

        let mut builder: Builder = Default::default();

        let SubgraphAdded {
            scope_key,
            entry_points,
            require: required,
        } = builder.add_subgraph(&marshalling, source_code, entry_point_key, None)?;
        let Builder {
            scopes,
            event_names,
            definition_order,
            events_delay,
            events_bind,
            events_recv,
            events_send,
            events_respond,
            key_unblocks_values,
        } = builder;

        let priority = definition_order
            .into_iter()
            .enumerate()
            .map(|(p, k)| (k, p))
            .collect();

        let events = Events {
            priority,
            required,
            names: event_names,
            bind: events_bind,
            send: events_send,
            recv: events_recv,
            respond: events_respond,
            delay: events_delay,
            entry_points,
            key_unblocks_values,
        };

        Ok(Executable {
            marshalling,
            events,
            root_scope_key: scope_key,
            scopes,
        })
    }
}

fn type_aliases<'a>(
    marshalling: &MarshallingRegistry,
    imports: impl IntoIterator<Item = &'a DefTypeAlias>,
) -> Result<HashMap<MessageName, Arc<str>>, BuildError> {
    use std::collections::hash_map::Entry::Vacant;
    let mut aliases = HashMap::new();
    for import in imports {
        let Vacant(entry) = aliases.entry(import.type_alias.to_owned()) else {
            return Err(BuildError::DuplicateAlias(import.type_alias.clone()));
        };
        let _marshaller = marshalling
            .resolve(&import.type_name)
            .ok_or(BuildError::UnknownFqn(import.type_name.to_owned()))?;

        entry.insert(import.type_name.as_str().into());
    }

    Ok(aliases)
}

fn validate_actor_names<'a>(
    actor_names: impl IntoIterator<Item = &'a ActorName>,
) -> Result<HashSet<ActorName>, BuildError> {
    let mut out = HashSet::new();

    for name in actor_names {
        if !out.insert(name.clone()) {
            return Err(BuildError::DuplicateActorName(name.clone()));
        }
    }

    Ok(out)
}

fn resolve_event_ids<'a>(
    idx_keys: &'a HashMap<&'a EventName, EventKey>,
    names: &'a [EventName],
) -> impl Iterator<Item = Result<EventKey, BuildError>> + 'a {
    names.into_iter().map(move |name: &EventName| {
        idx_keys
            .get(name)
            .copied()
            .ok_or(BuildError::UnknownEvent(name.clone()))
    })
}

#[derive(Debug, Default)]
struct Builder {
    scopes: SlotMap<KeyScope, ScopeInfo>,

    event_names: HashMap<EventKey, (KeyScope, EventName)>,

    definition_order: Vec<EventKey>,

    events_delay: SlotMap<KeyDelay, EventDelay>,
    events_bind: SlotMap<KeyBind, EventBind>,
    events_recv: SlotMap<KeyRecv, EventRecv>,
    events_send: SlotMap<KeySend, EventSend>,
    events_respond: SlotMap<KeyRespond, EventRespond>,

    key_unblocks_values: HashMap<EventKey, BTreeSet<EventKey>>,
}

#[derive(Debug)]
struct SubgraphAdded {
    scope_key: KeyScope,
    entry_points: BTreeSet<EventKey>,
    require: HashMap<EventKey, RequiredToBe>,
}

impl Builder {
    fn add_subgraph(
        &mut self,
        marshalling: &MarshallingRegistry,
        sources: &SourceCode,
        source_key: KeyScenario,
        invoked_as: Option<(KeyScope, EventName, SubroutineName)>,
    ) -> Result<SubgraphAdded, BuildError> {
        let this_source = &sources[source_key];

        debug!("storing type-aliases...");
        let type_aliases = type_aliases(&marshalling, &this_source.scenario.types)?;
        for (a, fqn) in &type_aliases {
            trace!("- {:?} -> {:?}", a, fqn);
        }

        debug!("checking actor-names...");
        let actors = validate_actor_names(&this_source.scenario.cast)?;
        for actor_name in &actors {
            trace!("- {:?}", actor_name);
        }

        let this_scope_key = self.scopes.insert(ScopeInfo {
            source_key,
            invoked_as,
        });

        let mut this_scope_name_to_key = HashMap::new();
        let mut this_scope_entry_points = BTreeSet::new();
        let mut this_scope_requires = HashMap::new();

        for DefEvent {
            id: this_name,
            require: this_event_required_to_be,
            prerequisites,
            kind,
            ..
        } in this_source.scenario.events.iter()
        {
            let prerequisites = resolve_event_ids(&mut this_scope_name_to_key, &prerequisites)
                .collect::<Result<Vec<_>, _>>()?;

            let (head_key, tail_key) = match kind {
                DefEventKind::Call(def_call) => {
                    let sub_source_key = this_source
                        .subroutines
                        .get(&def_call.subroutine_name)
                        .copied()
                        .ok_or_else(|| {
                            BuildError::UnknownSubroutine(def_call.subroutine_name.clone())
                        })?;
                    let SubgraphAdded {
                        scope_key: sub_scope_key,
                        entry_points: sub_entry_points,
                        require: sub_required_to_be,
                    } = self.add_subgraph(
                        marshalling,
                        sources,
                        sub_source_key,
                        Some((
                            this_scope_key,
                            this_name.clone(),
                            def_call.subroutine_name.clone(),
                        )),
                    )?;

                    // create two bind nodes:
                    // - one for input (bind from `scope_key` to `sub_scope_key`, choose the nodes using `entrypoints`)
                    // - one for output (bind from `sub_scope_key` to `scope_key`, choose the nodes using `required`)
                    //
                    // the latter bind will be referred to by `this_key`, so that it can be depended on
                    // (the events that want to happen after this call — should take place after the output-bind).

                    let event_bind_in = {
                        let (dst, src) = if let Some(def_bind_in) = def_call.input.as_ref() {
                            (
                                def_bind_in.dst.clone(),
                                SrcMsg::Bind(def_bind_in.src.clone()),
                            )
                        } else {
                            (DstPattern(json!(null)), SrcMsg::Literal(json!(null)))
                        };
                        EventBind {
                            dst,
                            src,
                            scope: BindScope::Two {
                                src: this_scope_key,
                                dst: sub_scope_key,
                                actors: def_call.cast.clone().unwrap_or_default(),
                            },
                        }
                    };
                    let bind_in = self.events_bind.insert(event_bind_in);
                    let ek_bind_in = EventKey::Bind(bind_in);
                    self.event_names.insert(
                        ek_bind_in,
                        (this_scope_key, this_name.with_suffix(" (ENTER)")),
                    );

                    for sub_entry_point in sub_entry_points {
                        let hasnt_been_added_before = self
                            .key_unblocks_values
                            .entry(ek_bind_in)
                            .or_default()
                            .insert(sub_entry_point);
                        assert!(hasnt_been_added_before);
                    }

                    let event_bind_out = {
                        let (dst, src) = if let Some(def_bind_out) = def_call.output.as_ref() {
                            (
                                def_bind_out.dst.clone(),
                                SrcMsg::Bind(def_bind_out.src.clone()),
                            )
                        } else {
                            (DstPattern(json!(null)), SrcMsg::Literal(json!(null)))
                        };
                        EventBind {
                            dst,
                            src,
                            scope: BindScope::Two {
                                src: sub_scope_key,
                                dst: this_scope_key,
                                actors: def_call
                                    .cast
                                    .clone()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|(l, r)| (r, l))
                                    .collect(),
                            },
                        }
                    };
                    let bind_out = self.events_bind.insert(event_bind_out);
                    let ek_bind_out = EventKey::Bind(bind_out);

                    for (sub_key, requirement) in sub_required_to_be {
                        if matches!(requirement, RequiredToBe::Reached) {
                            let hasnt_been_added_before = self
                                .key_unblocks_values
                                .entry(sub_key)
                                .or_default()
                                .insert(ek_bind_out);
                            assert!(hasnt_been_added_before);
                        }
                    }

                    (ek_bind_in, ek_bind_out)
                }
                DefEventKind::Delay(def_delay) => {
                    let DefEventDelay {
                        delay_for,
                        delay_step,
                        no_extra: _,
                    } = def_delay;
                    let delay_for = *delay_for;
                    let delay_step = *delay_step;

                    let key = self.events_delay.insert(EventDelay {
                        delay_for,
                        delay_step,
                    });
                    let ek_delay = EventKey::Delay(key);
                    (ek_delay, ek_delay)
                }
                DefEventKind::Bind(def_bind) => {
                    let DefEventBind {
                        dst,
                        src,
                        no_extra: _,
                    } = def_bind;
                    let dst = dst.clone();
                    let src = src.clone();
                    let key = self.events_bind.insert(EventBind {
                        dst,
                        src,
                        scope: BindScope::Same(this_scope_key),
                    });

                    let ek_bind = EventKey::Bind(key);
                    (ek_bind, ek_bind)
                }
                DefEventKind::Recv(def_recv) => {
                    let DefEventRecv {
                        message_type,
                        message_data,
                        also_match_data,
                        from,
                        to,
                        timeout,
                        no_extra: _,
                    } = def_recv;

                    let type_fqn = type_aliases
                        .get(message_type)
                        .cloned()
                        .ok_or(BuildError::UnknownAlias(message_type.clone()))?;

                    for actor_name in to.as_ref().into_iter().chain(from) {
                        if !actors.contains(actor_name) {
                            return Err(BuildError::UnknownActor(actor_name.clone()));
                        }
                    }

                    let key = self.events_recv.insert(EventRecv {
                        from: from.clone(),
                        to: to.clone(),
                        fqn: type_fqn,
                        payload_matchers: [message_data.clone()]
                            .into_iter()
                            .chain(also_match_data.into_iter().cloned())
                            .collect(),
                        timeout: *timeout,
                        scope_key: this_scope_key,
                    });
                    let ek_recv = EventKey::Recv(key);
                    (ek_recv, ek_recv)
                }
                DefEventKind::Respond(def_respond) => {
                    let DefEventRespond {
                        from,
                        to_request: to,
                        data,
                        no_extra: _,
                    } = def_respond;

                    let causing_event_key = this_scope_name_to_key
                        .get(&to)
                        .ok_or(BuildError::UnknownEvent(to.clone()))?;
                    let EventKey::Recv(recv_key) = causing_event_key else {
                        return Err(BuildError::NotARequest(to.clone()));
                    };
                    let request_fqn = self.events_recv.get(*recv_key)
                        .expect("we do not delete items from `recv`; neither we store keys that are unrelated to our collections")
                        .fqn.clone();

                    if let Some(bad_actor) = from.as_ref().filter(|a| !actors.contains(a)) {
                        return Err(BuildError::UnknownActor(bad_actor.clone()));
                    }

                    if marshalling
                        .resolve(&request_fqn)
                        .is_none_or(|m| m.response().is_none())
                    {
                        return Err(BuildError::NotARequest(to.clone()));
                    }

                    let key = self.events_respond.insert(EventRespond {
                        respond_to: *recv_key,
                        request_type: request_fqn,
                        respond_from: from.clone(),
                        payload: data.clone(),
                        scope_key: this_scope_key,
                    });
                    let ek_respond = EventKey::Respond(key);
                    (ek_respond, ek_respond)
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
                        .ok_or(BuildError::UnknownAlias(message_type.clone()))?;

                    for actor_name in to.as_ref().into_iter().chain([from]) {
                        if !actors.contains(&actor_name) {
                            return Err(BuildError::UnknownActor(actor_name.clone()));
                        }
                    }

                    let key = self.events_send.insert(EventSend {
                        from: from.clone(),
                        to: to.clone(),
                        fqn: type_fqn,
                        payload: message_data.clone(),
                        scope_key: this_scope_key,
                    });
                    let ek_send = EventKey::Send(key);
                    (ek_send, ek_send)
                }
            };

            if let Some(r) = this_event_required_to_be {
                this_scope_requires.insert(tail_key, *r);
            }

            if prerequisites.is_empty() {
                let should_be_a_new_element = this_scope_entry_points.insert(head_key);
                assert!(
                    should_be_a_new_element,
                    "non unique entry point? {:?}",
                    head_key
                );
            }
            for prerequisite in &prerequisites {
                let should_be_a_new_element = self
                    .key_unblocks_values
                    .entry(*prerequisite)
                    .or_default()
                    .insert(head_key);

                assert!(
                    should_be_a_new_element,
                    "duplicate  relation: {:?} unblocks {:?}",
                    *prerequisite, head_key
                );
            }

            trace!("  done: {:?} -> {:?}-{:?}", this_name, head_key, tail_key);

            if this_scope_name_to_key.insert(this_name, tail_key).is_some() {
                return Err(BuildError::DuplicateEventName(this_name.clone()));
            }
            self.definition_order.push(head_key);
            self.definition_order.push(tail_key);
        }

        for (name, key) in this_scope_name_to_key {
            let should_be_none = self.event_names.insert(key, (this_scope_key, name.clone()));
            assert!(should_be_none.is_none());
        }

        Ok(SubgraphAdded {
            scope_key: this_scope_key,
            entry_points: this_scope_entry_points,
            require: this_scope_requires,
        })
    }
}
