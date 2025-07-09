use std::{collections::HashMap, sync::Arc};

use elfo::{test::Proxy, AnyMessage, Envelope, ResponseToken};
use futures::{future::LocalBoxFuture, FutureExt};
use ghost::phantom;
use serde_json::Value;
use tracing::trace;

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

#[derive(Default, derive_more::Debug)]
pub struct Messages {
    #[debug(skip)]
    values: HashMap<String, AnyMessage>,

    #[debug(skip)]
    marshallers: HashMap<String, Box<dyn Marshal>>,
}

pub trait SupportedMessage {
    fn register(self, messages: &mut Messages);
}

pub trait Marshal {
    fn bind(&self, envelope: &Envelope, bind_to: &Msg) -> Option<Vec<(String, Value)>>;
    fn marshall(
        &self,
        messages: &Messages,
        bindings: &HashMap<String, Value>,
        value: Msg,
    ) -> Result<AnyMessage, AnError>;
    fn response(&self) -> Option<&dyn DynRespond>;
}

pub trait Respond<'a>: Marshal {
    fn respond(
        &self,
        proxy: &'a mut Proxy,
        token: ResponseToken,
        messages: Arc<Messages>,
        bindings: HashMap<String, Value>,
        value: Msg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>>;
}
pub trait DynRespond: for<'a> Respond<'a> {}
impl<R> DynRespond for R where R: for<'a> Respond<'a> {}

impl Messages {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with<S>(mut self, supported: S) -> Self
    where
        S: SupportedMessage,
    {
        self.add(supported);
        self
    }
    pub fn add<S>(&mut self, supported: S) -> &mut Self
    where
        S: SupportedMessage,
    {
        supported.register(self);
        self
    }

    pub fn resolve(&self, fqn: &str) -> Option<&dyn Marshal> {
        self.marshallers.get(fqn).map(AsRef::as_ref)
    }
}

impl<M> SupportedMessage for Regular<M>
where
    M: elfo::Message,
{
    fn register(self, messages: &mut Messages) {
        let fqn = std::any::type_name::<M>();
        messages.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl<Rq> SupportedMessage for Request<Rq>
where
    Rq: elfo::Request,
{
    fn register(self, messages: &mut Messages) {
        let fqn = std::any::type_name::<Rq>();
        messages.marshallers.insert(fqn.into(), Box::new(self));
    }
}

impl SupportedMessage for Injected {
    fn register(self, messages: &mut Messages) {
        messages.values.insert(self.key, self.value);
    }
}

impl<M> Marshal for Regular<M>
where
    M: elfo::Message,
{
    fn bind(&self, envelope: &Envelope, bind_to: &Msg) -> Option<Vec<(String, Value)>> {
        if !envelope.is::<M>() {
            return None;
        }

        let serialized = serde_json::to_value(envelope.message()).ok()?;

        trace!("      serialized: {:?}", serialized);
        trace!("      bind-to: {:?}", bind_to);

        match bind_to {
            Msg::Exact(value) => {
                if serialized == *value {
                    Some(Default::default())
                } else {
                    None
                }
            }
            Msg::Bind(pattern) => {
                let mut bindings = Default::default();
                if bind_to_pattern(serialized, pattern, &mut bindings) {
                    Some(bindings.into_iter().collect())
                } else {
                    None
                }
            }

            Msg::Injected(_name) => Some(Default::default()),
        }
    }
    fn marshall(
        &self,
        messages: &Messages,
        bindings: &HashMap<String, Value>,
        value: Msg,
    ) -> Result<AnyMessage, AnError> {
        match value {
            Msg::Bind(template) => {
                let value = render(template, bindings)?;
                let m: M = serde_json::from_value(value)?;
                let a = AnyMessage::new(m);
                Ok(a)
            }
            Msg::Injected(name) => {
                let a = messages.values.get(&name).cloned().ok_or("no such value")?;
                Ok(a)
            }
            Msg::Exact(value) => {
                let m: M = serde_json::from_value(value)?;
                let a = AnyMessage::new(m);
                Ok(a)
            }
        }
    }
    fn response(&self) -> Option<&'static dyn DynRespond> {
        None
    }
}

impl<Rq> Marshal for Request<Rq>
where
    Rq: elfo::Request,
{
    fn bind(&self, envelope: &Envelope, bind_to: &Msg) -> Option<Vec<(String, Value)>> {
        if !envelope.is::<Rq>() {
            return None;
        }

        let serialized = serde_json::to_value(envelope.message()).ok()?;

        trace!("      serialized: {:?}", serialized);
        trace!("      bind-to: {:?}", bind_to);

        match bind_to {
            Msg::Exact(value) => {
                if serialized == *value {
                    Some(Default::default())
                } else {
                    None
                }
            }
            Msg::Bind(pattern) => {
                let mut bindings = Default::default();
                if bind_to_pattern(serialized, pattern, &mut bindings) {
                    Some(bindings.into_iter().collect())
                } else {
                    None
                }
            }

            Msg::Injected(_name) => Some(Default::default()),
        }
    }
    fn marshall(
        &self,
        messages: &Messages,
        bindings: &HashMap<String, Value>,
        value: Msg,
    ) -> Result<AnyMessage, AnError> {
        match value {
            Msg::Bind(template) => {
                let value = render(template, bindings)?;
                let m: Rq::Wrapper = serde_json::from_value(value)?;
                let a = AnyMessage::new(m);
                Ok(a)
            }
            Msg::Injected(name) => {
                let a = messages.values.get(&name).cloned().ok_or("no such value")?;
                Ok(a)
            }
            Msg::Exact(value) => {
                let m: Rq::Wrapper = serde_json::from_value(value)?;
                let a = AnyMessage::new(m);
                Ok(a)
            }
        }
    }
    fn response(&self) -> Option<&'static dyn DynRespond> {
        Some(&Response::<Rq>)
    }
}

impl<Rq> Marshal for Response<Rq>
where
    Rq: elfo::Request,
{
    fn bind(&self, envelope: &Envelope, bind_to: &Msg) -> Option<Vec<(String, Value)>> {
        if !envelope.is::<Rq::Wrapper>() {
            return None;
        }

        let serialized = serde_json::to_value(envelope.message()).ok()?;

        trace!("      serialized: {:?}", serialized);
        trace!("      bind-to: {:?}", bind_to);

        match bind_to {
            Msg::Exact(value) => {
                if serialized == *value {
                    Some(Default::default())
                } else {
                    None
                }
            }
            Msg::Bind(pattern) => {
                let mut bindings = Default::default();
                if bind_to_pattern(serialized, pattern, &mut bindings) {
                    Some(bindings.into_iter().collect())
                } else {
                    None
                }
            }

            Msg::Injected(_name) => Some(Default::default()),
        }
    }
    fn marshall(
        &self,
        messages: &Messages,
        bindings: &HashMap<String, Value>,
        value: Msg,
    ) -> Result<AnyMessage, AnError> {
        match value {
            Msg::Bind(template) => {
                let value = render(template, bindings)?;
                let w: Rq::Wrapper = serde_json::from_value(value)?;
                let a = AnyMessage::new(w);
                Ok(a)
            }
            Msg::Injected(name) => {
                let a = messages.values.get(&name).cloned().ok_or("no such value")?;
                Ok(a)
            }
            Msg::Exact(value) => {
                let w: Rq::Wrapper = serde_json::from_value(value)?;
                let a = AnyMessage::new(w);
                Ok(a)
            }
        }
    }
    fn response(&self) -> Option<&'static dyn DynRespond> {
        None
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
        messages: Arc<Messages>,
        bindings: HashMap<String, Value>,
        value: Msg,
    ) -> LocalBoxFuture<'a, Result<(), AnError>> {
        async move {
            let token = token.into_received::<Rq>();
            match value {
                Msg::Bind(template) => {
                    let value = render(template, &bindings)?;
                    let de: Result<Rq::Wrapper, _> = serde_json::from_value(value);
                    match de {
                        Ok(w) => {
                            proxy.respond(token, w.into());
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                }
                Msg::Injected(name) => {
                    let a = messages.values.get(&name).cloned().ok_or("no such value")?;
                    if let Ok(response) = a.downcast::<Rq::Wrapper>() {
                        proxy.respond(token, response.into());
                        Ok(())
                    } else {
                        Err("couldn't cast".into())
                    }
                }
                Msg::Exact(value) => {
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

pub fn bind_to_pattern(
    value: Value,
    pattern: &Value,
    bindings: &mut HashMap<String, Value>,
) -> bool {
    use std::collections::hash_map::Entry::*;
    match (value, pattern) {
        (_, Value::String(wildcard)) if wildcard == "$_" => true,

        (value, Value::String(var_name)) if var_name.starts_with('$') => {
            match bindings.entry(var_name.to_owned()) {
                Vacant(v) => {
                    v.insert(value);
                    true
                }
                Occupied(o) => *o.get() == value,
            }
        }

        (Value::Null, Value::Null) => true,
        (Value::Bool(v), Value::Bool(p)) => v == *p,
        (Value::String(v), Value::String(p)) => v == *p,
        (Value::Number(v), Value::Number(p)) => v == *p,
        (Value::Array(values), Value::Array(patterns)) => {
            values.len() == patterns.len()
                && values
                    .into_iter()
                    .zip(patterns)
                    .all(|(v, p)| bind_to_pattern(v, p, bindings))
        }

        (Value::Object(mut v), Value::Object(p)) => p.iter().all(|(pk, pv)| {
            v.remove(pk)
                .is_some_and(|vv| bind_to_pattern(vv, pv, bindings))
        }),

        (_, _) => false,
    }
}

pub fn render(template: Value, bindings: &HashMap<String, Value>) -> Result<Value, AnError> {
    match template {
        Value::String(wildcard) if wildcard == "$_" => Err("can't render $_".into()),
        Value::String(var_name) if var_name.starts_with('$') => bindings
            .get(&var_name)
            .cloned()
            .ok_or_else(|| format!("unknown var: {:?}", var_name).into()),
        Value::Array(items) => Ok(Value::Array(
            items
                .into_iter()
                .map(|item| render(item, bindings))
                .collect::<Result<_, _>>()?,
        )),
        Value::Object(kv) => Ok(Value::Object(
            kv.into_iter()
                .map(|(k, v)| render(v, bindings).map(move |v| (k, v)))
                .collect::<Result<_, _>>()?,
        )),
        as_is => Ok(as_is),
    }
}
