use std::collections::HashMap;

use elfo::{test::Proxy, AnyMessage, AnyMessageRef, Envelope, Message, ResponseToken};
use futures::{future::LocalBoxFuture, FutureExt};
use ghost::phantom;
use serde_json::Value;

use crate::bindings;
use crate::scenario::Msg;

pub type AnError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy)]
#[phantom]
pub struct Regular<M>;

#[derive(Debug, Clone, Copy)]
#[phantom]
pub struct Request<Rq>;

#[derive(Debug, Clone, Copy)]
#[phantom]
pub struct Response<Rq>;

#[derive(derive_more::Debug)]
pub struct Injected {
    pub key: String,
    pub value: AnyMessage,
}

/// Manages [Msg] marshalling.
///
/// Marshalling happens in two ways:
/// - serializable message are marshalled by binding variables in message
///   templates to actual values an marshaling the result as [AnyMessage];
/// - non-serializable messages are marshalled by storing those as predefined
///   [AnyMessage]s which can retrieved by key and injected into message flow
///   as-is.
#[derive(Default, derive_more::Debug)]
pub struct MarshallingRegistry {
    #[debug(skip)]
    values: HashMap<String, AnyMessage>,

    #[debug(skip)]
    marshallers: HashMap<String, Box<dyn Marshal>>,
}

/// Registers self as to [MarshallingRegistry] to be used in marshalling.
pub trait RegisterMarshaller {
    /// Registers `self` to `marshalling`.
    fn register(self, marshalling: &mut MarshallingRegistry);
}

/// Marshals [Msg] as [AnyMessage].
pub(crate) trait Marshal {
    /// Binds values from `envelope` to `bindings` according to patterns
    /// from `msg`.
    ///
    /// Returns true if `envelope` was bound successfully.
    fn match_inbound_message(
        &self,
        envelope: &Envelope,
        msg: &Msg,
        bindings: &mut bindings::Txn,
    ) -> bool;

    /// Binds values in `msg` with `bindings` and marshals it as [AnyMessage].
    fn marshal_outbound_message(
        &self,
        messages: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: Msg,
    ) -> Result<AnyMessage, AnError>;

    /// Returns:
    /// - dyn [DynRespond] to marshal [Msg]s as elfo responses
    /// - `None` in case [Marshal] implementer only send regular elfo messages
    fn response(&self) -> Option<&dyn DynRespond>;
}

/// Marshals [Msg] to [Proxy] as elfo response.
pub(crate) trait Respond<'a> {
    /// Binds values `bindings` according to patterns from `msg` and send those
    /// to `proxy` as elfo response with the specified `token`.
    fn respond(
        &self,
        proxy: &'a mut Proxy,
        token: ResponseToken,
        marshalling: &'a MarshallingRegistry,
        bindings: &'a bindings::Scope,
        msg: Msg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>>;
}
pub(crate) trait DynRespond: for<'a> Respond<'a> {}
impl<R> DynRespond for R where R: for<'a> Respond<'a> {}

impl MarshallingRegistry {
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds `marshaller` to the [MarshallingRegistry].
    pub fn with<R>(mut self, marshaller: R) -> Self
    where
        R: RegisterMarshaller,
    {
        marshaller.register(&mut self);
        self
    }

    /// Resolves a fully qualified name `fqn` to the corresponding [Marshal].
    pub(crate) fn resolve(&self, fqn: &str) -> Option<&dyn Marshal> {
        self.marshallers.get(fqn).map(AsRef::as_ref)
    }

    /// Retrieves predefined [AnyMessage] by `key` to inject into the elfo
    /// message flow.
    pub(crate) fn value(&self, key: &str) -> Option<AnyMessageRef> {
        self.values.get(key).map(|am| am.as_ref())
    }
}

impl<M> RegisterMarshaller for Regular<M>
where
    M: elfo::Message,
{
    fn register(self, marshalling: &mut MarshallingRegistry) {
        let fqn = std::any::type_name::<M>();
        marshalling.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl<Rq> RegisterMarshaller for Request<Rq>
where
    Rq: elfo::Request,
{
    fn register(self, marshalling: &mut MarshallingRegistry) {
        let fqn = std::any::type_name::<Rq>();
        marshalling.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl RegisterMarshaller for Injected {
    fn register(self, marshalling: &mut MarshallingRegistry) {
        marshalling.values.insert(self.key, self.value);
    }
}

impl<M> Marshal for Regular<M>
where
    M: elfo::Message,
{
    fn match_inbound_message(
        &self,
        envelope: &Envelope,
        bind_to: &Msg,
        bindings: &mut bindings::Txn,
    ) -> bool {
        if !envelope.is::<M>() {
            return false;
        }

        let serialized = extract_message_payload(envelope)
            .expect("AnyMessage has changed serialization format?");

        do_match_message(bind_to, serialized, bindings)
    }
    fn marshal_outbound_message(
        &self,
        marshalling: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: Msg,
    ) -> Result<AnyMessage, AnError> {
        do_marshal_message::<M>(marshalling, bindings, msg)
    }
    fn response(&self) -> Option<&'static dyn DynRespond> {
        None
    }
}

impl<Rq> Marshal for Request<Rq>
where
    Rq: elfo::Request,
{
    fn match_inbound_message(
        &self,
        envelope: &Envelope,
        bind_to: &Msg,
        bindings: &mut bindings::Txn,
    ) -> bool {
        if !envelope.is::<Rq>() {
            return false;
        }

        let serialized = extract_message_payload(envelope)
            .expect("AnyMessage has changed serialization format?");

        do_match_message(bind_to, serialized, bindings)
    }
    fn marshal_outbound_message(
        &self,
        marshalling: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: Msg,
    ) -> Result<AnyMessage, AnError> {
        do_marshal_message::<Rq::Wrapper>(marshalling, bindings, msg)
    }
    fn response(&self) -> Option<&'static dyn DynRespond> {
        Some(&Response::<Rq>)
    }
}

impl<'a, Rq> Respond<'a> for Response<Rq>
where
    Rq: elfo::Request,
{
    fn respond(
        &self,
        proxy: &'a mut Proxy,
        token: ResponseToken,
        marshalling: &'a MarshallingRegistry,
        bindings: &'a bindings::Scope,
        value: Msg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>> {
        async move {
            let token = token.into_received::<Rq>();
            match value {
                Msg::Bind(template) => {
                    let value = bindings::render(template, bindings)?;
                    let de: Result<Rq::Wrapper, _> = serde_json::from_value(value);
                    match de {
                        Ok(w) => {
                            proxy.respond(token, w.into());
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                }
                Msg::Inject(name) => {
                    let a = marshalling
                        .values
                        .get(&name)
                        .cloned()
                        .ok_or("no such value")?;
                    if let Ok(response) = a.downcast::<Rq::Wrapper>() {
                        proxy.respond(token, response.into());
                        Ok(())
                    } else {
                        Err("couldn't cast".into())
                    }
                }
                Msg::Literal(value) => {
                    let de: Result<Rq::Wrapper, _> = serde_json::from_value(value);
                    match de {
                        Ok(w) => {
                            proxy.respond(token, w.into());
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                }
            }
        }
        .boxed_local()
    }
}

fn extract_message_payload(envelope: &Envelope) -> Option<Value> {
    let mut message_parts = serde_json::to_value(envelope.message()).ok()?;
    let &mut [ref mut _proto, ref mut _name, ref mut payload] =
        &mut message_parts.as_array_mut()?[..]
    else {
        return None;
    };
    let payload = std::mem::take(payload);
    Some(payload)
}

fn do_match_message(bind_to: &Msg, serialized: Value, bindings: &mut bindings::Txn) -> bool {
    match bind_to {
        Msg::Literal(value) => serialized == *value,
        Msg::Bind(pattern) => bindings::bind_to_pattern(serialized, pattern, bindings),
        Msg::Inject(_name) => false,
    }
}

fn do_marshal_message<M: Message>(
    marshalling: &MarshallingRegistry,
    bindings: &bindings::Scope,
    msg: Msg,
) -> Result<AnyMessage, AnError> {
    match msg {
        Msg::Bind(template) => {
            let value = bindings::render(template, bindings)?;
            let m: M = serde_json::from_value(value)?;
            let a = AnyMessage::new(m);
            Ok(a)
        }
        Msg::Inject(name) => {
            let a = marshalling.values.get(&name).cloned().ok_or("no such value")?;
            Ok(a)
        }
        Msg::Literal(value) => {
            let m: M = serde_json::from_value(value)?;
            let a = AnyMessage::new(m);
            Ok(a)
        }
    }
}
