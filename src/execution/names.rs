use crate::{
    execution::{EventKey, Executable, KeyScope},
    names::EventName,
};

impl Executable {
    pub fn event_name(&self, key: EventKey) -> Option<(KeyScope, EventName)> {
        self.events.names.get(&key).cloned()
    }
}
