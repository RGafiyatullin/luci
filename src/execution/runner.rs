use std::collections::{BTreeSet, HashMap, HashSet};

use elfo::_priv::MessageKind;
use elfo::{test::Proxy, Blueprint, Envelope, Message};
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use std::time::Duration;
use tokio::time::Instant;
use tracing::{debug, info, trace, warn};

use crate::execution::{BindScope, KeyScope};
use crate::recorder::records;
use crate::recorder::{RecordLog, Recorder};
use crate::{
    bindings,
    execution::{
        EventBind, EventDelay, EventKey, EventRecv, EventRespond, EventSend, Executable, KeyDelay,
        KeyRecv, KeyRespond, KeySend, Report,
    },
    marshalling,
    names::{ActorName, EventName},
    scenario::Msg,
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

    #[error("bind: {}", _0)]
    BindError(bindings::BindError),

    #[error("marshalling error: {}", _0)]
    Marshalling(marshalling::AnError),
}

/// A key for an event that is ready to be processed by [Runner].
///
/// A trimmed version of [EventKey].
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

/// Runs the set up integration test.
pub struct Runner<'a> {
    executable: &'a Executable,
    ready_events: BTreeSet<EventKey>,
    key_requires_values: HashMap<EventKey, HashSet<EventKey>>,
    scopes: SecondaryMap<KeyScope, bindings::Scope>,

    main_proxy_key: ProxyKey,
    proxies: SlotMap<ProxyKey, Proxy>,
    envelopes: HashMap<KeyRecv, Envelope>,
    delays: Delays,
    receives: Receives,
}

new_key_type! {
    struct ProxyKey;
}

#[derive(Default)]
struct Delays {
    deadlines: BTreeSet<(Instant, KeyDelay, Duration)>,
    steps: BTreeSet<(Duration, KeyDelay, Instant)>,
}

#[derive(Default)]
struct Receives {
    deadlines: BTreeSet<(Instant, KeyRecv)>,
}

impl Executable {
    /// Returns a [Runner] to run the test corresponding to this [Executable]
    /// and specified `blueprint` and `config`.
    pub async fn start<C>(&self, blueprint: Blueprint, config: C) -> Runner<'_>
    where
        C: for<'de> serde::de::Deserializer<'de>,
    {
        Runner::new(self, blueprint, config).await
    }
}

impl<'a> Runner<'a> {
    /// Runs the test for which the runner was set up.
    ///
    /// Returns;
    /// - [Report] containing a text description of the test run if test was
    ///   completed without errors, either successfully or not.
    /// - [RunError] in case of any errors during the test run.
    pub async fn run(mut self) -> Result<Report, RunError> {
        let mut record_log = RecordLog::new();
        let mut recorder = record_log.recorder();

        let mut unreached = self.executable.events.required.clone();
        let mut reached = HashMap::new();

        while let Some(event_key) = {
            // NOTE: if we do not introduce a variable `event_key_opt` here, the `self` would remain mutably borrowed.
            let event_key_opt = self.ready_events().next();
            event_key_opt
        } {
            debug!("firing: {:?}", event_key);

            let fired_events = self.fire_event(&mut recorder, event_key).await?;

            for ek in fired_events.iter() {
                // FIXME: show scope info too
                if let Some((scope_id, en)) = self.event_name(*ek) {
                    info!("fired event: {} ({:?}@{:?})", en, ek, scope_id);
                } else {
                    info!("fired unnabled event: {:?}", ek)
                }
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
            // XXX: are we expecting only the root-scope's names here?
            .map(|(k, v)| (self.event_name(k).expect("bad event-key").1.clone(), v))
            .collect();
        let unreached = unreached
            .into_iter()
            // XXX: are we expecting only the root-scope's names here?
            .map(|(k, v)| (self.event_name(k).expect("bad event-key").1.clone(), v))
            .collect();

        Ok(Report {
            reached,
            unreached,
            record_log,
        })
    }

    // #[doc(hidden)]
    // pub
    fn ready_events(&self) -> impl Iterator<Item = ReadyEventKey> + '_ {
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

        // this is just a predictable order of events, no significant scientific basis behind it.
        binds.chain(send_and_respond).chain(recv_or_delay)
    }

    pub fn event_name(&self, event_key: EventKey) -> Option<(KeyScope, &EventName)> {
        self.executable
            .events
            .names
            .get(&event_key)
            .map(|(s, e)| (*s, e))
    }

    // #[doc(hidden)]
    // pub
    async fn fire_event(
        &mut self,
        recorder: &mut Recorder<'_>,
        ready_event_key: ReadyEventKey,
    ) -> Result<Vec<EventKey>, RunError> {
        let mut recorder = recorder.write(records::ProcessEventClass(ready_event_key));

        if let Ok(event_key) = EventKey::try_from(ready_event_key) {
            if !self.ready_events.remove(&event_key) {
                return Err(RunError::EventIsNotReady(ready_event_key));
            }

            let event_name = self
                .executable
                .events
                .names
                .get(&event_key)
                .expect("invalid event-key in ready-events?");
            assert!(self.key_requires_values.get(&event_key).is_none());

            debug!("firing {:?}...", event_name);
        } else {
            if !self.ready_events.iter().any(|e| {
                matches!(
                    e,
                    EventKey::Recv(_) | EventKey::Delay(_) | EventKey::Bind(_)
                )
            }) {
                return Err(RunError::EventIsNotReady(ready_event_key));
            }

            debug!("doing {:?}", ready_event_key);
        }

        let actually_fired_events = match ready_event_key {
            ReadyEventKey::Bind => self.fire_event_bind(&mut recorder).await?,
            ReadyEventKey::Send(k) => self.fire_event_send(&mut recorder, k).await?,
            ReadyEventKey::Respond(k) => self.fire_event_respond(&mut recorder, k).await?,
            ReadyEventKey::RecvOrDelay => self.fire_event_recv_or_delay(&mut recorder).await?,
        };

        self.process_dependencies_of_fired_events(actually_fired_events.iter().copied());

        Ok(actually_fired_events)
    }
}

impl<'a> Runner<'a> {
    fn process_dependencies_of_fired_events(
        &mut self,
        actually_fired_events: impl IntoIterator<Item = EventKey>,
    ) {
        use std::collections::hash_map::Entry::Occupied;

        let Executable { events, .. } = self.executable;
        for fired_event in actually_fired_events.into_iter() {
            if let Some(dependent_keys) = events.key_unblocks_values.get(&fired_event) {
                for dependent_key in dependent_keys.iter().copied() {
                    let Occupied(mut remove_from) = self.key_requires_values.entry(dependent_key)
                    else {
                        panic!("key_requires_values inconsistent with key_unblocks_values [1]")
                    };
                    let should_have_existed = remove_from.get_mut().remove(&fired_event);
                    assert!(
                        should_have_existed,
                        "key_requires_values inconsistent with key_unblocks_values [2]"
                    );
                    if remove_from.get().is_empty() {
                        debug!("  unblocked {:?}", dependent_key);
                        remove_from.remove();
                        self.ready_events.insert(dependent_key);

                        match dependent_key {
                            EventKey::Delay(k) => {
                                self.delays.insert(Instant::now(), k, &events.delay[k])
                            }
                            EventKey::Recv(k) => {
                                self.receives.insert(Instant::now(), k, &events.recv[k])
                            }
                            _ => (),
                        }
                    }
                }
            }
        }
    }

    async fn fire_event_bind(
        &mut self,
        recorder: &mut Recorder<'_>,
    ) -> Result<Vec<EventKey>, RunError> {
        let Executable {
            marshalling,
            events,
            ..
        } = self.executable;

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
            tmp.sort_by_key(|k| events.priority.get(&EventKey::Bind(*k)));
            tmp
        };

        trace!("ready_bind_keys: {:#?}", ready_bind_keys);
        recorder.write(records::ReadyBindKeys(ready_bind_keys.clone()));

        let mut actually_fired_events = vec![];
        for bind_key in ready_bind_keys {
            let mut recorder = recorder.write(records::ProcessBindKey(bind_key));
            self.ready_events.remove(&EventKey::Bind(bind_key));

            trace!(" binding {:?}", bind_key);
            let EventBind {
                dst,
                src,
                scope: bind_scope,
            } = &events.bind[bind_key];

            let (src_scope_key, dst_scope_key) = match bind_scope {
                BindScope::Same(scope_id) => (*scope_id, *scope_id),
                BindScope::Two { src, dst, .. } => (*src, *dst),
            };

            let mut recorder_src = recorder.write(records::BindSrcScope(src_scope_key));
            let src_scope = &self.scopes[src_scope_key];

            recorder_src.write(records::UsingMsg(src.clone()));
            let value = match src {
                Msg::Literal(value) => value.clone(),
                Msg::Bind(template) => {
                    bindings::render(template.clone(), src_scope).map_err(RunError::BindError)?
                }
                Msg::Inject(key) => {
                    let m = marshalling.value(key).ok_or(RunError::Marshalling(
                        format!("no such key: {:?}", key).into(),
                    ))?;
                    serde_json::to_value(m).expect("can't serialize a message?")
                }
            };
            recorder_src.write(records::BindValue(value.clone()));

            let mut dst_actor_names = vec![];
            if let BindScope::Two { actors, .. } = bind_scope {
                dst_actor_names.extend(actors.into_iter().filter_map(|(src_name, dst_name)| {
                    src_scope
                        .address_of(src_name)
                        .zip(Some((src_name, dst_name)))
                }));
            }

            let mut recorder_dst = recorder.write(records::BindDstScope(dst_scope_key));
            let mut dst_scope_txn = self.scopes[dst_scope_key].txn();

            if !dst_actor_names
                .into_iter()
                .all(|(addr, (src_name, dst_name))| {
                    let bound = dst_scope_txn.bind_actor(dst_name, addr);
                    recorder_dst.write(records::BindActorName(dst_name.clone(), addr, bound));
                    trace!(
                        "  binding {:?}->{:?} {} [bound: {}]",
                        src_name,
                        dst_name,
                        addr,
                        bound
                    );
                    bound
                })
            {
                recorder.write(records::BindOutcome(false));
                trace!("could not bind actor-names");
                continue;
            }

            // TODO: pass the recorder_dst inside
            if !bindings::bind_to_pattern(value, dst, &mut dst_scope_txn) {
                recorder.write(records::BindOutcome(false));
                trace!("could not bind {:?}", bind_key);
                continue;
            }

            recorder.write(records::BindOutcome(true));
            dst_scope_txn.commit();

            recorder.write(records::EventFired(bind_key.into()));
            actually_fired_events.push(EventKey::Bind(bind_key));
        }

        Ok(actually_fired_events)
    }

    async fn fire_event_recv_or_delay(
        &mut self,
        recorder: &mut Recorder<'_>,
    ) -> Result<Vec<EventKey>, RunError> {
        let Executable {
            marshalling,
            events,
            ..
        } = self.executable;

        let mut actually_fired_events = vec![];

        'recv_or_delay: loop {
            self.proxies[self.main_proxy_key].sync().await;

            for timed_out_recv_key in self.receives.select_timed_out(Instant::now()) {
                recorder.write(records::TimedOutRecvKey(timed_out_recv_key));
                trace!("recv timed out: {:?}", timed_out_recv_key);
                self.ready_events
                    .remove(&EventKey::Recv(timed_out_recv_key));
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
                tmp.sort_by_key(|k| events.priority.get(&EventKey::Recv(*k)));
                tmp
            };

            trace!("ready_recv_keys: {:#?}", ready_recv_keys);
            recorder.write(records::ReadyRecvKeys(ready_recv_keys.clone()));

            let mut unmatched_envelopes = 0;

            for (proxy_key, proxy) in self.proxies.iter_mut() {
                trace!(" try_recv at proxies[{:?}]", proxy_key);
                let Some(envelope) = proxy.try_recv().await else {
                    continue;
                };

                let envelope_message_name = envelope.message().name();

                let sent_from = envelope.sender();
                let sent_to_opt = Some(proxy.addr()).filter(|_| proxy_key != self.main_proxy_key);

                trace!("  from: {:?}", sent_from);
                trace!("  to:   {:?}", sent_to_opt);
                trace!("  msg-name: {}", envelope.message().name());

                let mut recorder = recorder.write(records::EnvelopeReceived {
                    message_name: envelope_message_name,
                    from: sent_from,
                    to_opt: sent_to_opt,
                });

                let mut envelope_unused = true;

                for recv_key in ready_recv_keys.iter().copied() {
                    let mut recorder = recorder.write(records::MatchingRecv(recv_key));

                    trace!(
                        "   matching against {:?} [{:?}]",
                        recv_key,
                        events.names.get(&EventKey::Recv(recv_key)).unwrap()
                    );
                    let EventRecv {
                        fqn: match_type,
                        from: match_from,
                        to: match_to,
                        payload: match_message,
                        timeout: _,
                        scope_key,
                    } = &events.recv[recv_key];

                    let mut scope_txn = self.scopes[*scope_key].txn();

                    let marshaller = marshalling.resolve(&match_type).expect("bad FQN");

                    if let Some(from_name) = match_from {
                        trace!("expecting source: {:?}", from_name);
                        let bound = scope_txn.bind_actor(from_name, sent_from);
                        recorder.write(records::BindActorName(from_name.clone(), sent_from, bound));

                        if !bound {
                            trace!(
                                "could not bind source [name: {}; addr: {}]",
                                from_name,
                                sent_from
                            );

                            continue;
                        }
                    }

                    match (match_to, sent_to_opt) {
                        (Some(bind_to_name), Some(sent_to_address)) => {
                            trace!(
                                "expecting directed to {:?}, sent to address: {}",
                                bind_to_name,
                                sent_to_address
                            );
                            let bound = scope_txn.bind_actor(bind_to_name, sent_to_address);
                            recorder.write(records::BindActorName(
                                bind_to_name.clone(),
                                sent_to_address,
                                bound,
                            ));

                            if !bound {
                                trace!(
                                    "could not bind destination [name: {}; addr: {}]",
                                    bind_to_name,
                                    sent_to_address
                                );
                                continue;
                            }
                        }

                        (Some(bind_to_name), None) => {
                            trace!(
                                "   expected directed to {:?}, got routed message",
                                bind_to_name
                            );
                            recorder
                                .write(records::ExpectedDirectedGotRouted(bind_to_name.clone()));
                            continue;
                        }
                        (_, _) => (),
                    }

                    let bound =
                        marshaller.match_inbound_message(&envelope, match_message, &mut scope_txn);
                    recorder.write(records::BindOutcome(bound));
                    if !bound {
                        trace!("   marshaller couldn't bind");
                        continue;
                    };

                    scope_txn.commit();
                    self.envelopes.insert(recv_key, envelope);
                    self.ready_events.remove(&EventKey::Recv(recv_key));
                    self.receives.remove_by_key(recv_key);
                    actually_fired_events.push(EventKey::Recv(recv_key));

                    recorder.write(records::EventFired(recv_key.into()));

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
        }

        Ok(actually_fired_events)
    }

    async fn fire_event_send(
        &mut self,
        recorder: &mut Recorder<'_>,
        event_key: KeySend,
    ) -> Result<Vec<EventKey>, RunError> {
        let Executable {
            marshalling,
            events: vertices,
            ..
        } = self.executable;
        let EventSend {
            from: send_from,
            to: send_to,
            fqn: message_type,
            payload: message_data,
            scope_key,
        } = &vertices.send[event_key];
        debug!(
            " sending {:?} [from: {:?}; to: {:?}]",
            message_type, send_from, send_to
        );
        recorder.write(records::ProcessSend(event_key));

        let send_to_addr_opt = send_to
            .as_ref()
            .map(|actor_name| {
                let addr = self.scopes[*scope_key]
                    .address_of(&actor_name)
                    .ok_or_else(|| RunError::UnboundName(actor_name.clone()))?;
                if self.proxies.iter().any(|(_, p)| p.addr() == addr) {
                    return Err(RunError::DummyName(actor_name.clone()));
                }

                recorder.write(records::ResolveActorName(actor_name.clone(), addr));

                Ok(addr)
            })
            .transpose()?;

        let proxy = if let Some(addr) = self.scopes[*scope_key].address_of(send_from) {
            let proxy = self
                .proxies
                .values_mut()
                .find(|p| p.addr() == addr)
                .ok_or_else(|| RunError::ActorName(send_from.clone()))?;

            recorder.write(records::ResolveActorName(send_from.clone(), proxy.addr()));

            proxy
        } else {
            let new_proxy = self.proxies[self.main_proxy_key].subproxy().await;
            let new_addr = new_proxy.addr();
            let mut txn = self.scopes[*scope_key].txn();
            assert!(
                txn.bind_actor(send_from, new_addr),
                "this name has just been unbound!"
            );
            txn.commit();
            recorder.write(records::BindActorName(send_from.clone(), new_addr, true));

            let proxy_key = self.proxies.insert(new_proxy);
            &mut self.proxies[proxy_key]
        };

        recorder.write(records::SendMessageType(message_type.clone()));
        recorder.write(records::UsingMsg(message_data.clone()));

        let marshaller = self
            .executable
            .marshalling
            .resolve(&message_type)
            .expect("invalid FQN");

        // TODO: pass the recorder inside of marshaller to record the actually rendered message
        let any_message = marshaller
            .marshal_outbound_message(&marshalling, &self.scopes[*scope_key], message_data.clone())
            .map_err(RunError::Marshalling)?;

        recorder.write(records::SendTo(send_to_addr_opt));
        if let Some(dst_addr) = send_to_addr_opt {
            trace!(
                "sending directly [from: {}; to: {}]: {:?}",
                dst_addr,
                proxy.addr(),
                any_message
            );
            let () = proxy.send_to(dst_addr, any_message).await;
        } else {
            trace!(
                "sending via routing [from: {}]: {:?}",
                proxy.addr(),
                any_message
            );
            let () = proxy.send(any_message).await;
        }

        recorder.write(records::EventFired(event_key.into()));
        Ok(vec![EventKey::Send(event_key)])
    }

    async fn fire_event_respond(
        &mut self,
        recorder: &mut Recorder<'_>,
        event_key: KeyRespond,
    ) -> Result<Vec<EventKey>, RunError> {
        let Executable {
            marshalling,
            events: vertices,
            ..
        } = self.executable;

        let EventRespond {
            respond_to,
            request_type: request_fqn,
            respond_from,
            payload: message_data,
            scope_key,
        } = &vertices.respond[event_key];
        debug!(
            " responding to a {:?} [from: {:?}]",
            request_fqn, respond_from
        );

        recorder.write(records::ProcessRespond(event_key));

        let proxy_key = if let Some(respond_from) = respond_from {
            if let Some(addr) = self.scopes[*scope_key].address_of(respond_from) {
                self.proxies
                    .iter()
                    .find_map(|(k, p)| Some(k).filter(|_| p.addr() == addr))
                    .ok_or_else(|| RunError::ActorName(respond_from.clone()))?
            } else {
                let new_proxy = self.proxies[self.main_proxy_key].subproxy().await;
                let new_addr = new_proxy.addr();
                let mut txn = self.scopes[*scope_key].txn();
                assert!(
                    txn.bind_actor(respond_from, new_addr),
                    "this name has just been unbound!"
                );
                txn.commit();

                let proxy_key = self.proxies.insert(new_proxy);
                proxy_key
            }
        } else {
            self.main_proxy_key
        };

        let request_marshaller = self
            .executable
            .marshalling
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

        let responding_proxy = &mut self.proxies[proxy_key];

        recorder.write(records::UsingMsg(message_data.clone()));

        // TODO: pass the recorder inside to record what actual value is being sent
        response_marshaller
            .respond(
                responding_proxy,
                token,
                &marshalling,
                &self.scopes[*scope_key],
                message_data.clone(),
            )
            .await
            .map_err(RunError::Marshalling)?;

        recorder.write(records::EventFired(event_key.into()));
        Ok(vec![EventKey::Respond(event_key)])
    }
}

impl<'a> Runner<'a> {
    async fn new<C>(executable: &'a Executable, blueprint: Blueprint, config: C) -> Self
    where
        C: for<'de> serde::de::Deserializer<'de>,
    {
        let main_proxy = elfo::test::proxy(blueprint, config).await;

        let mut proxies: SlotMap<ProxyKey, Proxy> = Default::default();
        let main_proxy_key = proxies.insert(main_proxy);

        let mut delays = Delays::default();
        let mut receives = Receives::default();

        let ready_events = executable.events.entry_points.clone();

        let now = Instant::now();
        for k in ready_events.iter().copied() {
            match k {
                EventKey::Delay(k) => delays.insert(now, k, &executable.events.delay[k]),
                EventKey::Recv(k) => receives.insert(now, k, &executable.events.recv[k]),
                _ => (),
            }
        }

        let key_requires_values = executable
            .events
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

        let mut scopes: SecondaryMap<KeyScope, bindings::Scope> = executable
            .scopes
            .iter()
            .map(|(key, _info)| (key, Default::default()))
            .collect();
        scopes.insert(executable.root_scope_key, Default::default());

        Self {
            executable,
            ready_events,
            key_requires_values,
            delays,
            receives,
            main_proxy_key,
            proxies,
            scopes,
            envelopes: Default::default(),
        }
    }
}

impl Receives {
    fn insert(&mut self, now: Instant, key: KeyRecv, recv_event: &EventRecv) {
        if let Some(timeout) = recv_event.timeout {
            let deadline = now.checked_add(timeout).expect("oh don't be ridiculous!");
            let new_entry = self.deadlines.insert((deadline, key));
            assert!(new_entry);
        }
    }

    fn select_timed_out(&mut self, now: Instant) -> impl Iterator<Item = KeyRecv> + use<'_> {
        std::iter::repeat_with(move || {
            let (deadline, _) = self.deadlines.first().copied()?;
            if deadline < now {
                let (_, key) = self
                    .deadlines
                    .pop_first()
                    .expect("we've just seen it be there!");
                Some(key)
            } else {
                None
            }
        })
        .map_while(std::convert::identity)
    }

    fn remove_by_key(&mut self, key: KeyRecv) {
        self.deadlines.retain(|(_deadline, k)| *k != key);
    }
}

impl Delays {
    fn next(&mut self, now: Instant) -> (Instant, Vec<KeyDelay>) {
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

    fn insert(&mut self, now: Instant, key: KeyDelay, delay_event: &EventDelay) {
        let delay_for = delay_event.delay_for;
        let step = delay_event.delay_step;

        let deadline = now.checked_add(delay_for).expect("please pretty please");

        let new_d_entry = self.deadlines.insert((deadline, key, step));
        let new_s_entry = self.steps.insert((step, key, deadline));

        assert!(new_d_entry && new_s_entry);
    }
}
