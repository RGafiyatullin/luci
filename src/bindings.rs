use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use bimap::BiHashMap;
use elfo::Addr;
use serde_json::Value;
use tracing::info;

use crate::names::ActorName;

pub type AnError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, Default)]
pub(crate) struct Scope {
    values: HashMap<String, Value>,
    actors: BiHashMap<ActorName, Addr>,
}

#[derive(Debug)]
pub(crate) struct Txn<'a> {
    values_old: &'a mut HashMap<String, Value>,
    values_new: HashMap<String, Value>,

    actors_old: &'a mut BiHashMap<ActorName, Addr>,
    actors_new: BiHashMap<ActorName, Addr>,
}

impl Scope {
    pub(crate) fn txn(&mut self) -> Txn {
        Txn {
            values_old: &mut self.values,
            values_new: Default::default(),

            actors_old: &mut self.actors,
            actors_new: Default::default(),
        }
    }
}

impl<'a> Txn<'a> {
    pub(crate) fn value_of(&self, key: &str) -> Option<&Value> {
        let old_opt = self.values_new.get(key);
        let new_opt = self.values_old.get(key);

        old_opt.or(new_opt)
    }

    pub(crate) fn address_of(&self, name: &ActorName) -> Option<Addr> {
        let old_opt = self.actors_old.get_by_left(name).copied();
        let new_opt = self.actors_new.get_by_left(name).copied();

        old_opt.or(new_opt)
    }

    pub(crate) fn name_of(&self, addr: Addr) -> Option<&ActorName> {
        let old_opt = self.actors_old.get_by_right(&addr);
        let new_opt = self.actors_new.get_by_right(&addr);

        old_opt.or(new_opt)
    }

    pub(crate) fn set_value(&mut self, key: &str, value: &Value) -> bool {
        if let Some(defined_in_state) = self.values_old.get(key) {
            defined_in_state == value
        } else {
            match self.values_new.entry(key.to_owned()) {
                Occupied(o) => o.get() == value,
                Vacant(v) => {
                    v.insert(value.to_owned());
                    true
                }
            }
        }
    }

    pub(crate) fn name_actor(&mut self, name: &ActorName, addr: Addr) -> bool {
        if let Some(existing_name) = self.name_of(addr) {
            return existing_name == name;
        }
        if let Some(existing_addr) = self.address_of(name) {
            assert!(existing_addr != addr);
            return false;
        }

        self.actors_new
            .insert_no_overwrite(name.clone(), addr)
            .expect("none of the sides resolved before!");
        true
    }

    pub(crate) fn commit(self) {
        self.values_old.extend(
            self.values_new
                .into_iter()
                .inspect(|(k, v)| info!("SET VALUE {:?} <- {:?}", k, v)),
        );
        self.actors_old.extend(
            self.actors_new
                .into_iter()
                .inspect(|(k, v)| info!("SET ACTOR {:?} <- {:?}", k, v)),
        );
    }
}

pub(crate) fn bind_to_pattern(value: Value, pattern: &Value, bindings: &mut Txn) -> bool {
    match (value, pattern) {
        (_, Value::String(wildcard)) if wildcard == "$_" => true,

        (value, Value::String(var_name)) if var_name.starts_with('$') => {
            bindings.set_value(&var_name, &value)
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

pub(crate) fn render(template: Value, bindings: &Txn) -> Result<Value, AnError> {
    match template {
        Value::String(wildcard) if wildcard == "$_" => Err("can't render $_".into()),
        Value::String(var_name) if var_name.starts_with('$') => bindings
            .value_of(&var_name)
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    impl Scope {
        pub(crate) fn new() -> Self {
            Default::default()
        }

        pub(crate) fn value_of(&self, key: &str) -> Option<&Value> {
            self.values.get(key)
        }

        // pub(crate) fn address_of(&self, name: &ActorName) -> Option<Addr> {
        //     self.actors.get_by_left(name).copied()
        // }

        // pub(crate) fn name_of(&self, addr: Addr) -> Option<&ActorName> {
        //     self.actors.get_by_right(&addr)
        // }
    }

    #[test]
    fn test_01() {
        let mut values = Scope::new();
        assert!(values.value_of("a").is_none());
        assert!(values.value_of("b").is_none());

        {
            let mut binder = values.txn();
            assert!(binder.value_of("a").is_none());
            assert!(binder.value_of("b").is_none());

            assert!(binder.set_value("a", &json!("a")));
            assert!(binder.set_value("a", &json!("a")));
            assert!(!binder.set_value("a", &json!("b")));
        }

        assert!(values.value_of("a").is_none());
        assert!(values.value_of("b").is_none());

        {
            let mut binder = values.txn();
            assert!(binder.value_of("a").is_none());
            assert!(binder.value_of("b").is_none());

            assert!(binder.set_value("a", &json!("a")));
            assert!(binder.set_value("a", &json!("a")));
            assert!(!binder.set_value("a", &json!("b")));

            binder.commit();
        }

        assert_eq!(values.value_of("a").cloned(), Some(json!("a")));
        assert!(values.value_of("b").is_none());
    }
}
