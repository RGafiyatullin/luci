use std::collections::HashMap;

use elfo::{test::Proxy, AnyMessage, AnyMessageRef, Envelope, Message, ResponseToken};
use futures::{future::LocalBoxFuture, FutureExt};
use ghost::phantom;
use serde_json::Value;
use tracing::debug;

use crate::bindings;
use crate::scenario::{DstPattern, SrcMsg};

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

// This one is used in the tests, that do not require to actually run their scenarios,
// but instead just check the how build works.
#[doc(hidden)]
#[derive(derive_more::Debug)]
pub struct Mock {
    fqn: String,
    is_request: bool,
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
        msg: &DstPattern,
        bindings: &mut bindings::Txn,
    ) -> bool;

    /// Binds values in `msg` with `bindings` and marshals it as [AnyMessage].
    fn marshal_outbound_message(
        &self,
        marshalling: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: SrcMsg,
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
        msg: SrcMsg,
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

impl Mock {
    pub fn new(fqn: impl Into<String>, is_request: bool) -> Self {
        let fqn = fqn.into();
        Self { fqn, is_request }
    }
    pub fn regular(fqn: impl Into<String>) -> Self {
        Self::new(fqn, false)
    }
    pub fn request(fqn: impl Into<String>) -> Self {
        Self::new(fqn, true)
    }
}

impl<M> RegisterMarshaller for Regular<M>
where
    M: elfo::Message,
{
    fn register(self, marshalling: &mut MarshallingRegistry) {
        let fqn = std::any::type_name::<M>();
        debug!("registering regular message: {}", fqn);
        marshalling.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl<Rq> RegisterMarshaller for Request<Rq>
where
    Rq: elfo::Request,
{
    fn register(self, marshalling: &mut MarshallingRegistry) {
        let fqn = std::any::type_name::<Rq>();
        debug!("registering request message: {}", fqn);
        marshalling.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl RegisterMarshaller for Injected {
    fn register(self, marshalling: &mut MarshallingRegistry) {
        marshalling.values.insert(self.key, self.value);
    }
}

impl RegisterMarshaller for Mock {
    fn register(self, marshalling: &mut MarshallingRegistry) {
        let fqn = self.fqn.clone();
        let should_be_none = marshalling.marshallers.insert(fqn, Box::new(self));
        assert!(should_be_none.is_none(), "duplicate FQN");
    }
}

impl Marshal for Mock {
    fn marshal_outbound_message(
        &self,
        _marshalling: &MarshallingRegistry,
        _bindings: &bindings::Scope,
        _msg: SrcMsg,
    ) -> Result<AnyMessage, AnError> {
        panic!("it's a mock!")
    }

    fn match_inbound_message(
        &self,
        _envelope: &Envelope,
        _msg: &DstPattern,
        _bindings: &mut bindings::Txn,
    ) -> bool {
        panic!("it's a mock!")
    }

    fn response(&self) -> Option<&dyn DynRespond> {
        let dyn_respond: &dyn DynRespond = self;
        Some(dyn_respond).filter(|_| self.is_request)
    }
}

impl<'a> Respond<'a> for Mock {
    fn respond(
        &self,
        _proxy: &'a mut Proxy,
        _token: ResponseToken,
        _marshalling: &'a MarshallingRegistry,
        _bindings: &'a bindings::Scope,
        _msg: SrcMsg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>> {
        panic!("it's a mock!")
    }
}

impl<M> Marshal for Regular<M>
where
    M: elfo::Message,
{
    fn match_inbound_message(
        &self,
        envelope: &Envelope,
        bind_to: &DstPattern,
        bindings: &mut bindings::Txn,
    ) -> bool {
        if !envelope.is::<M>() {
            return false;
        }

        let serialized = extract_message_payload(envelope)
            .expect("AnyMessage has changed serialization format?");

        bindings::bind_to_pattern(serialized, bind_to, bindings)
    }
    fn marshal_outbound_message(
        &self,
        marshalling: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: SrcMsg,
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
        bind_to: &DstPattern,
        bindings: &mut bindings::Txn,
    ) -> bool {
        if !envelope.is::<Rq>() {
            return false;
        }

        let serialized = extract_message_payload(envelope)
            .expect("AnyMessage has changed serialization format?");

        bindings::bind_to_pattern(serialized, bind_to, bindings)
    }
    fn marshal_outbound_message(
        &self,
        marshalling: &MarshallingRegistry,
        bindings: &bindings::Scope,
        msg: SrcMsg,
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
        value: SrcMsg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>> {
        async move {
            let token = token.into_received::<Rq>();
            match value {
                SrcMsg::Bind(template) => {
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
                SrcMsg::Inject(name) => {
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
                SrcMsg::Literal(value) => {
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

fn do_marshal_message<M: Message>(
    marshalling: &MarshallingRegistry,
    bindings: &bindings::Scope,
    msg: SrcMsg,
) -> Result<AnyMessage, AnError> {
    match msg {
        SrcMsg::Bind(template) => {
            let value = bindings::render(template, bindings)?;
            let m: M = serde_json::from_value(value)?;
            let a = AnyMessage::new(m);
            Ok(a)
        }
        SrcMsg::Inject(name) => {
            let a = marshalling
                .values
                .get(&name)
                .cloned()
                .ok_or("no such value")?;
            Ok(a)
        }
        SrcMsg::Literal(value) => {
            let m: M = serde_json::from_value(value)?;
            let a = AnyMessage::new(m);
            Ok(a)
        }
    }
}
