use std::{
    collections::{BTreeSet, HashMap, HashSet},
    num::NonZeroUsize,
};

use elfo::_priv::MessageKind;
use elfo::{test::Proxy, Addr, Blueprint, Envelope, Message};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, info, trace, warn};

use crate::{
    execution_graph::{
        EventKey, ExecutionGraph, KeyDelay, KeyRecv, KeyRespond, KeySend, VertexBind, VertexDelay,
        VertexRecv, VertexRespond, VertexSend,
    },
    messages,
    scenario::{ActorName, EventName, Msg, RequiredToBe},
};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("event is not ready: {:?}", _0)]
    EventIsNotReady(ReadyEventKey),

    #[error("name already taken by a dummy: {}", _0)]
    DummyName(ActorName),

    #[error("name already taken by an actor: {}", _0)]
    ActorName(ActorName),

    #[error("name has not yet been bound to an address: {}", _0)]
    UnboundName(ActorName),

    #[error("no request envelope found")]
    NoRequest,

    #[error("marshalling error: {}", _0)]
    Marshalling(messages::AnError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadyEventKey {
    Bind,
    RecvOrDelay,
    Send(KeySend),
    Respond(KeyRespond),
}

impl From<EventKey> for ReadyEventKey {
    fn from(e: EventKey) -> Self {
        match e {
            EventKey::Bind(_) => Self::Bind,
            EventKey::Send(k) => Self::Send(k),
            EventKey::Respond(k) => Self::Respond(k),
            EventKey::Delay(_) | EventKey::Recv(_) => Self::RecvOrDelay,
        }
    }
}
impl TryFrom<ReadyEventKey> for EventKey {
    type Error = ();
    fn try_from(e: ReadyEventKey) -> Result<Self, Self::Error> {
        match e {
            ReadyEventKey::Bind => Err(()),
            ReadyEventKey::Send(k) => Ok(Self::Send(k)),
            ReadyEventKey::Respond(k) => Ok(Self::Respond(k)),
            ReadyEventKey::RecvOrDelay => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub reached: HashMap<EventName, RequiredToBe>,
    pub unreached: HashMap<EventName, RequiredToBe>,
}

pub struct Runner<'a> {
    graph: &'a ExecutionGraph,

    ready_events: BTreeSet<EventKey>,
    key_requires_values: HashMap<EventKey, HashSet<EventKey>>,

    actors: Actors,
    dummies: Dummies,

    proxies: Vec<Proxy>,
    bindings: HashMap<String, Value>,
    envelopes: HashMap<KeyRecv, Envelope>,
    delays: Delays,
}

#[derive(Default)]
struct Delays {
    deadlines: BTreeSet<(Instant, KeyDelay, Duration)>,
    steps: BTreeSet<(Duration, KeyDelay, Instant)>,
}

#[derive(Default)]
struct Actors {
    by_name: HashMap<ActorName, Addr>,
    by_addr: HashMap<Addr, ActorName>,

    excluded: HashSet<ActorName>,
}

#[derive(Default)]
struct Dummies {
    by_name: HashMap<ActorName, (Addr, NonZeroUsize)>,
    by_addr: HashMap<Addr, (ActorName, NonZeroUsize)>,

    excluded: HashSet<ActorName>,
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

impl ExecutionGraph {
    pub async fn make_runner<C>(&self, blueprint: Blueprint, config: C) -> Runner<'_>
    where
        C: for<'de> serde::de::Deserializer<'de>,
    {
        Runner::new(self, blueprint, config).await
    }
}

impl<'a> Runner<'a> {
    pub async fn new<C>(graph: &'a ExecutionGraph, blueprint: Blueprint, config: C) -> Self
    where
        C: for<'de> serde::de::Deserializer<'de>,
    {
        let proxies = vec![elfo::test::proxy(blueprint, config).await];
        let mut delays = Delays::default();

        let ready_events = graph.vertices.entry_points.clone();

        let now = Instant::now();
        ready_events.iter().copied().for_each(|k| {
            if let EventKey::Delay(k) = k {
                delays.insert(now, k, &graph.vertices.delay[k]);
            }
        });

        let key_requires_values = graph
            .vertices
            .key_unblocks_values
            .iter()
            .flat_map(|(&prereq, dependants)| {
                dependants
                    .iter()
                    .copied()
                    .map(move |dependant| (dependant, prereq))
            })
            .fold(
                HashMap::<EventKey, HashSet<EventKey>>::new(),
                |mut acc, (dependant, prereq)| {
                    acc.entry(dependant).or_default().insert(prereq);
                    acc
                },
            );
        Self {
            graph,

            ready_events,
            key_requires_values,
            delays,

            proxies,
            actors: Default::default(),
            dummies: Default::default(),
            bindings: Default::default(),
            envelopes: Default::default(),
        }
    }

    pub async fn run(mut self) -> Result<Report, RunError> {
        let mut unreached = self.graph.vertices.required.clone();
        let mut reached = HashMap::new();
        loop {
            let Some(event_key) = self.ready_events().next() else {
                break;
            };

            debug!("firing: {:?}", event_key);

            let fired_events = self.fire_event(event_key).await?;

            for ek in fired_events.iter() {
                let en = self.event_name(*ek).expect("unknown event-key");
                info!("fired event: {}", en);
            }

            if fired_events.is_empty() {
                info!("no more progress. I think we're done here.");
                break;
            }

            for event_id in fired_events {
                let Some(r) = unreached.remove(&event_id) else {
                    continue;
                };
                reached.insert(event_id, r);
            }
        }

        let reached = reached
            .into_iter()
            .map(|(k, v)| (self.event_name(k).cloned().expect("bad event-key"), v))
            .collect();
        let unreached = unreached
            .into_iter()
            .map(|(k, v)| (self.event_name(k).cloned().expect("bad event-key"), v))
            .collect();

        Ok(Report { reached, unreached })
    }

    pub fn ready_events(&self) -> impl Iterator<Item = ReadyEventKey> + '_ {
        let binds = self
            .ready_events
            .iter()
            .copied()
            .filter(|k| matches!(k, EventKey::Bind(_)))
            .map(ReadyEventKey::from)
            .take(1);
        let send_and_respond = self
            .ready_events
            .iter()
            .copied()
            .filter(|k| matches!(k, EventKey::Send(_) | EventKey::Respond(_)))
            .map(ReadyEventKey::from);

        let recv_or_delay = self
            .ready_events
            .iter()
            .copied()
            .filter(|k| matches!(k, EventKey::Recv(_) | EventKey::Delay(_)))
            .map(ReadyEventKey::from)
            .take(1);

        binds.chain(send_and_respond).chain(recv_or_delay)
    }

    pub fn event_name(&self, event_key: EventKey) -> Option<&EventName> {
        self.graph.vertices.names.get(&event_key)
    }

    pub async fn fire_event(
        &mut self,
        ready_event_key: ReadyEventKey,
    ) -> Result<Vec<EventKey>, RunError> {
        let event_key_opt = EventKey::try_from(ready_event_key).ok();

        if let Some(event_key) = event_key_opt {
            if !self.ready_events.remove(&event_key) {
                return Err(RunError::EventIsNotReady(ready_event_key));
            }
        } else {
            if !self.ready_events.iter().any(|e| {
                matches!(
                    e,
                    EventKey::Recv(_) | EventKey::Delay(_) | EventKey::Bind(_)
                )
            }) {
                return Err(RunError::EventIsNotReady(ready_event_key));
            }
        }

        if let Some(event_key) = event_key_opt {
            let event_name = self
                .graph
                .vertices
                .names
                .get(&event_key)
                .expect("invalid event-key in ready-events?");
            assert!(self.key_requires_values.get(&event_key).is_none());

            debug!("firing {:?}...", event_name);
        } else {
            debug!("doing {:?}", ready_event_key);
        }

        let ExecutionGraph { messages, vertices } = self.graph;

        let mut actually_fired_events = vec![];
        match ready_event_key {
            ReadyEventKey::Bind => {
                let ready_bind_keys = {
                    let mut tmp = self
                        .ready_events
                        .iter()
                        .filter_map(|e| {
                            if let EventKey::Bind(k) = e {
                                Some(*k)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    tmp.sort_by_key(|k| vertices.priority.get(&EventKey::Bind(*k)));
                    tmp
                };

                trace!("ready_bind_keys: {:#?}", ready_bind_keys);

                for bind_key in ready_bind_keys {
                    self.ready_events.remove(&EventKey::Bind(bind_key));

                    trace!(" binding {:?}", bind_key);
                    let VertexBind { dst, src } = &vertices.bind[bind_key];

                    let value = match src {
                        Msg::Literal(value) => value.clone(),
                        Msg::Bind(template) => messages::render(template.clone(), &self.bindings)
                            .map_err(RunError::Marshalling)?,
                        Msg::Inject(key) => {
                            let m = messages.value(key).ok_or(RunError::Marshalling(
                                format!("no such key: {:?}", key).into(),
                            ))?;
                            serde_json::to_value(m).expect("can't serialize a message?")
                        }
                    };

                    let mut kv = Default::default();
                    if !messages::bind_to_pattern(value, dst, &mut kv) {
                        trace!("  could not bind {:?}", bind_key);
                        continue;
                    }

                    let Ok(kv) = kv
                        .into_iter()
                        .map(|(k, v1)| {
                            if self.bindings.get(&k).is_some_and(|v0| !v1.eq(v0)) {
                                Err(())
                            } else {
                                Ok((k, v1))
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()
                    else {
                        trace!("  binding mismatch");
                        continue;
                    };

                    for (k, v) in kv {
                        info!("  bind {} <- {:?}", k, v);
                        self.bindings.insert(k, v);
                    }

                    actually_fired_events.push(EventKey::Bind(bind_key));
                }
            }
            ReadyEventKey::Send(k) => {
                let VertexSend {
                    send_from,
                    send_to,
                    message_type,
                    message_data,
                } = &vertices.send[k];
                debug!(
                    " sending {:?} [from: {:?}; to: {:?}]",
                    message_type, send_from, send_to
                );

                let actor_addr_opt = if let Some(actor_name) = send_to {
                    let addr = self
                        .actors
                        .resolve(actor_name)?
                        .ok_or_else(|| RunError::UnboundName(actor_name.clone()))?;

                    Some(addr)
                } else {
                    None
                };

                let (dummy_addr, proxy_idx) = self
                    .dummies
                    .bind(send_from.clone(), &mut self.proxies, &mut self.actors)
                    .await?;

                let marshaller = self
                    .graph
                    .messages
                    .resolve(&message_type)
                    .expect("invalid FQN");
                let any_message = marshaller
                    .marshall(&messages, &self.bindings, message_data.clone())
                    .map_err(RunError::Marshalling)?;

                let sending_proxy = &mut self.proxies[proxy_idx.get()];
                if let Some(dst_addr) = actor_addr_opt {
                    trace!(
                        " sending directly [from: {}; to: {}]: {:?}",
                        dst_addr,
                        dummy_addr,
                        any_message
                    );
                    let () = sending_proxy.send_to(dst_addr, any_message).await;
                } else {
                    trace!(
                        " sending via routing [from: {}: {:?}",
                        dummy_addr,
                        any_message
                    );
                    let () = sending_proxy.send(any_message).await;
                }

                actually_fired_events.push(EventKey::Send(k));
            }
            ReadyEventKey::Respond(k) => {
                let VertexRespond {
                    respond_to,
                    request_fqn,
                    respond_from,
                    message_data,
                } = &vertices.respond[k];
                debug!(
                    " responding to a {:?} [from: {:?}]",
                    request_fqn, respond_from
                );

                let proxy_idx = if let Some(from) = respond_from {
                    self.dummies
                        .bind(from.clone(), &mut self.proxies, &mut self.actors)
                        .await?
                        .1
                        .get()
                } else {
                    0
                };
                let request_marshaller = self
                    .graph
                    .messages
                    .resolve(&request_fqn)
                    .expect("invalid FQN");
                let response_marshaller = request_marshaller
                    .response()
                    .expect("request_fqn does not point to a Request");

                let Some(request_envelope) = self.envelopes.remove(respond_to) else {
                    return Err(RunError::NoRequest);
                };

                let token = match request_envelope.message_kind() {
                    MessageKind::RequestAny(token) => token.duplicate(),
                    MessageKind::RequestAll(token) => token.duplicate(),
                    _ => return Err(RunError::NoRequest),
                };

                let responding_proxy = &mut self.proxies[proxy_idx];
                response_marshaller
                    // XXX: bindings.clone() — tsk tsk tsk
                    .respond(
                        responding_proxy,
                        token,
                        messages.clone(),
                        self.bindings.clone(),
                        message_data.clone(),
                    )
                    .await
                    .map_err(RunError::Marshalling)?;

                actually_fired_events.push(EventKey::Respond(k));
            }

            ReadyEventKey::RecvOrDelay => 'recv_or_delay: loop {
                for p in self.proxies.iter_mut() {
                    p.sync().await;
                }

                trace!(" receiving...");

                let ready_recv_keys = {
                    let mut tmp = self
                        .ready_events
                        .iter()
                        .filter_map(|e| {
                            if let EventKey::Recv(k) = e {
                                Some(*k)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    tmp.sort_by_key(|k| vertices.priority.get(&EventKey::Recv(*k)));
                    tmp
                };

                trace!("ready_recv_keys: {:#?}", ready_recv_keys);

                let mut unmatched_envelopes = 0;

                for (proxy_idx, proxy) in self.proxies.iter_mut().enumerate() {
                    trace!(" try_recv at proxies[{}]", proxy_idx);
                    let Some(envelope) = proxy.try_recv().await else {
                        continue;
                    };

                    let envelope_message_name = envelope.message().name();

                    let sent_from = envelope.sender();
                    let sent_to_opt = Some(proxy.addr()).filter(|_| proxy_idx != 0);

                    trace!("  from: {:?}", sent_from);
                    trace!("  to:   {:?}", sent_to_opt);
                    trace!("  msg-name: {}", envelope.message().name());

                    let mut envelope_unused = true;

                    for recv_key in ready_recv_keys.iter().copied() {
                        trace!(
                            "   matching against {:?} [{:?}]",
                            recv_key,
                            vertices.names.get(&EventKey::Recv(recv_key)).unwrap()
                        );
                        let VertexRecv {
                            match_type,
                            match_from,
                            match_to,
                            match_message,
                        } = &vertices.recv[recv_key];
                        let marshaller = messages.resolve(&match_type).expect("bad FQN");

                        if let Some(from_name) = match_from {
                            trace!("    expecting source: {:?}", from_name);
                            if !self.actors.can_bind(from_name, sent_from) {
                                trace!("    can't bind");
                                continue;
                            }
                        }

                        match (match_to, sent_to_opt) {
                            (Some(bind_to_name), Some(sent_to_address)) => {
                                trace!(
                                    "   expecting directed to {:?}, sent to address: {}",
                                    bind_to_name,
                                    sent_to_address
                                );
                                if !self.dummies.can_bind(bind_to_name, sent_to_address) {
                                    trace!("    can't bind");
                                    continue;
                                }
                            }

                            (Some(bind_to_name), None) => {
                                trace!(
                                    "   expected directed to {:?}, got routed message",
                                    bind_to_name
                                );
                                continue;
                            }
                            (_, _) => (),
                        }

                        let Some(kv) = marshaller.bind(&envelope, match_message) else {
                            trace!("   marshaller couldn't bind");
                            continue;
                        };

                        trace!("   marshaller bound: {:#?}", kv);

                        let Ok(kv) = kv
                            .into_iter()
                            .map(|(k, v1)| {
                                if self.bindings.get(&k).is_some_and(|v0| !v1.eq(v0)) {
                                    Err(())
                                } else {
                                    Ok((k, v1))
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()
                        else {
                            trace!("     binding mismatch");
                            continue;
                        };

                        for (k, v) in kv {
                            info!("    bind {} <- {:?}", k, v);
                            self.bindings.insert(k, v);
                        }
                        if let Some(from_name) = match_from {
                            let bound_ok = self.actors.bind(
                                from_name.clone(),
                                sent_from,
                                &mut self.dummies,
                            )?;
                            assert!(bound_ok);
                        }

                        self.envelopes.insert(recv_key, envelope);
                        self.ready_events.remove(&EventKey::Recv(recv_key));
                        actually_fired_events.push(EventKey::Recv(recv_key));

                        envelope_unused = false;
                        break;
                    }

                    if envelope_unused {
                        warn!("unmatched envelope with message {}", envelope_message_name);
                        unmatched_envelopes += 1;
                    }
                }

                match (actually_fired_events.is_empty(), unmatched_envelopes == 0) {
                    (true, true) => {
                        let now = Instant::now();
                        let (sleep_until, expired_keys) = self.delays.next(now);

                        trace!(
                            "nothing to do — sleeping for {:?}...",
                            sleep_until.checked_duration_since(now),
                        );

                        tokio::time::sleep_until(sleep_until).await;

                        let some_keys_expired = !expired_keys.is_empty();

                        for delay_key in expired_keys {
                            self.ready_events.remove(&EventKey::Delay(delay_key));
                            actually_fired_events.push(EventKey::Delay(delay_key));
                        }

                        if some_keys_expired || sleep_until == now {
                            break 'recv_or_delay;
                        }
                    }
                    (true, false) => {
                        trace!("no fired events, but some unhandled envelopes");
                    }

                    (false, _) => {
                        trace!("some events fired. Good!");
                        break 'recv_or_delay;
                    }
                }
            },
        };

        for fired_event in actually_fired_events.iter() {
            if let Some(ds) = vertices.key_unblocks_values.get(fired_event) {
                for d in ds.iter().copied() {
                    let std::collections::hash_map::Entry::Occupied(mut remove_from) =
                        self.key_requires_values.entry(d)
                    else {
                        panic!("key_requires_values inconsistent with key_unblocks_values [1]")
                    };
                    let should_have_existed = remove_from.get_mut().remove(fired_event);
                    assert!(
                        should_have_existed,
                        "key_requires_values inconsistent with key_unblocks_values [2]"
                    );
                    if remove_from.get().is_empty() {
                        debug!("  unblocked {:?}", d);
                        remove_from.remove();
                        self.ready_events.insert(d);

                        if let EventKey::Delay(k) = d {
                            self.delays.insert(Instant::now(), k, &vertices.delay[k]);
                        }
                    }
                }
            }
        }

        Ok(actually_fired_events)
    }
}

impl Actors {
    fn can_bind(&self, actor_name: &ActorName, addr: Addr) -> bool {
        match (
            self.excluded.contains(actor_name),
            self.by_name.get(actor_name),
            self.by_addr.get(&addr),
        ) {
            (true, _, _) | (false, Some(_), None) | (false, None, Some(_)) => false,
            (false, None, None) => true,
            (false, Some(same_addr), Some(same_name)) => {
                same_name == actor_name && *same_addr == addr
            }
        }
    }

    fn bind(
        &mut self,
        actor_name: ActorName,
        addr: Addr,
        dummies: &mut Dummies,
    ) -> Result<bool, RunError> {
        use std::collections::hash_map::Entry::*;

        if self.excluded.contains(&actor_name) {
            return Err(RunError::DummyName(actor_name));
        }

        match (self.by_name.entry(actor_name), self.by_addr.entry(addr)) {
            (Occupied(_), Vacant(_)) | (Vacant(_), Occupied(_)) => Ok(false),
            (Vacant(by_name), Vacant(by_addr)) => {
                dummies.exclude(by_name.key().clone())?;

                by_addr.insert(by_name.key().clone());
                by_name.insert(addr);

                Ok(true)
            }
            (Occupied(by_name), Occupied(by_addr)) => {
                assert_eq!(by_name.key(), by_addr.get());
                assert_eq!(by_addr.key(), by_name.get());

                Ok(*by_name.get() == addr)
            }
        }
    }

    fn resolve(&mut self, actor_name: &ActorName) -> Result<Option<Addr>, RunError> {
        if self.excluded.contains(actor_name) {
            return Err(RunError::DummyName(actor_name.clone()));
        }

        let addr_opt = self.by_name.get(actor_name).copied();
        Ok(addr_opt)
    }
    fn exclude(&mut self, actor_name: ActorName) -> Result<(), RunError> {
        if self.by_name.contains_key(&actor_name) {
            return Err(RunError::ActorName(actor_name));
        }
        self.excluded.insert(actor_name);
        Ok(())
    }
}

impl Dummies {
    fn can_bind(&self, actor_name: &ActorName, addr: Addr) -> bool {
        match (
            self.excluded.contains(actor_name),
            self.by_name.get(actor_name),
            self.by_addr.get(&addr),
        ) {
            (true, _, _) | (false, Some(_), None) | (false, None, Some(_)) => false,
            (false, None, None) => true,
            (false, Some((same_addr, _)), Some((same_name, _))) => {
                same_name == actor_name && *same_addr == addr
            }
        }
    }

    async fn bind(
        &mut self,
        actor_name: ActorName,
        proxies: &mut Vec<Proxy>,
        actors: &mut Actors,
    ) -> Result<(Addr, NonZeroUsize), RunError> {
        use std::collections::hash_map::Entry::*;

        if self.excluded.contains(&actor_name) {
            return Err(RunError::ActorName(actor_name));
        }

        match self.by_name.entry(actor_name.clone()) {
            Occupied(o) => Ok(*o.get()),

            Vacant(by_name) => {
                let proxy = proxies[0].subproxy().await;
                let addr = proxy.addr();

                let Vacant(by_addr) = self.by_addr.entry(addr) else {
                    panic!("fresh proxy, seen address — wtf?")
                };

                actors.exclude(actor_name.clone())?;

                let idx: NonZeroUsize = proxies
                    .len()
                    .try_into()
                    .expect("`proxies[0]` was present: expecting `proxies.len > 0`");
                proxies.push(proxy);
                by_addr.insert((actor_name, idx));
                by_name.insert((addr, idx));

                Ok((addr, idx))
            }
        }
    }

    fn exclude(&mut self, actor_name: ActorName) -> Result<(), RunError> {
        if self.by_name.contains_key(&actor_name) {
            return Err(RunError::DummyName(actor_name));
        }
        self.excluded.insert(actor_name);
        Ok(())
    }
}

impl Delays {
    pub fn next(&mut self, now: Instant) -> (Instant, Vec<KeyDelay>) {
        let mut expired = vec![];

        assert_eq!(self.deadlines.len(), self.steps.len());

        let closest_deadline = loop {
            let Some(&(deadline, key, step)) = self.deadlines.first() else {
                break now;
            };
            if deadline < now {
                self.deadlines.remove(&(deadline, key, step));
                self.steps.remove(&(step, key, deadline));

                expired.push(key);
            } else {
                break deadline;
            }
        };

        assert_eq!(self.deadlines.len(), self.steps.len());

        let smallest_step = self
            .steps
            .first()
            .map_or(Duration::ZERO, |(step, _, _)| *step);

        let effective_deadline = now
            .checked_add(smallest_step)
            .unwrap_or(now)
            .min(closest_deadline);

        (effective_deadline, expired)
    }

    pub fn insert(&mut self, now: Instant, key: KeyDelay, delay_vertex: &VertexDelay) {
        let delay_for = delay_vertex.delay_for;
        let step = delay_vertex.delay_step;

        let deadline = now.checked_add(delay_for).expect("please pretty please");

        let new_d_entry = self.deadlines.insert((deadline, key, step));
        let new_s_entry = self.steps.insert((step, key, deadline));

        assert!(new_d_entry && new_s_entry);
    }
}
