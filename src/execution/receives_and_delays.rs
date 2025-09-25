use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;

use crate::execution::{EventDelay, EventRecv, KeyDelay, KeyRecv};

const RECV_RESOLUTION_DIVISOR: u32 = 1000;

#[derive(Default)]
pub(crate) struct ReceivesAndDelays {
    schedule:   BTreeSet<ScheduleEntry>,
    resolution: BTreeSet<ResolutionEntry>,
    valid_from: HashMap<KeyRecv, Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum KeyDelayOrRecv {
    Delay(KeyDelay),
    Recv(KeyRecv),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ScheduleEntry {
    at:    Instant,
    event: ScheduledEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ScheduledEvent {
    Ripe(KeyDelayOrRecv),
    UnsetResolution(ResolutionEntry),
    SetResolution(ResolutionEntry),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ResolutionEntry {
    resolution: Duration,
    key:        KeyDelayOrRecv,
}

impl ReceivesAndDelays {
    pub(crate) fn next_sleep_until(&self, now: Instant) -> Option<Instant> {
        let ResolutionEntry { resolution, .. } = self.resolution.first().copied()?;
        let one_step_after_now = now.checked_add(resolution).unwrap_or(now);

        let ScheduleEntry { at, .. } = self.schedule.first().copied()?;

        Some(one_step_after_now.min(at))
    }

    pub(crate) fn select_ripe_keys(&mut self, now: Instant) -> Vec<KeyDelayOrRecv> {
        let mut out = vec![];

        while self.schedule.first().is_some_and(|se| se.at <= now) {
            let ScheduleEntry { event, .. } = self
                .schedule
                .pop_first()
                .expect("wouldn't enter this loop otherwise");

            match event {
                ScheduledEvent::Ripe(key) => out.push(key),
                ScheduledEvent::UnsetResolution(re) => {
                    let actually_removed = self.resolution.remove(&re);
                    assert!(actually_removed);
                },
                ScheduledEvent::SetResolution(re) => {
                    let actually_inserted = self.resolution.insert(re);
                    assert!(actually_inserted);
                },
            };
        }

        out
    }

    pub(crate) fn remove_recv_by_key(&mut self, key: KeyRecv) -> Instant {
        let valid_from = self
            .valid_from
            .remove(&key)
            .expect("recv-key should have existed");
        let key = KeyDelayOrRecv::Recv(key);
        self.schedule.retain(|ScheduleEntry { event, .. }| {
            key != match event {
                ScheduledEvent::Ripe(key) => *key,
                ScheduledEvent::SetResolution(ResolutionEntry { key, .. }) => *key,
                ScheduledEvent::UnsetResolution(ResolutionEntry { key, .. }) => *key,
            }
        });
        self.resolution.retain(|re| re.key != key);

        valid_from
    }

    pub(crate) fn insert_delay(&mut self, now: Instant, key: KeyDelay, event: &EventDelay) {
        let delay_for = event.delay_for;
        let resolution = event.delay_step;
        let at = now.checked_add(delay_for).expect("please pretty please");
        let key = KeyDelayOrRecv::Delay(key);

        let r_entry = ResolutionEntry { resolution, key };
        let new_r_entry = self.resolution.insert(r_entry);
        let new_s_entry_1 = self.schedule.insert(ScheduleEntry {
            at,
            event: ScheduledEvent::UnsetResolution(r_entry),
        });
        let new_s_entry_2 = self.schedule.insert(ScheduleEntry {
            at,
            event: ScheduledEvent::Ripe(key),
        });

        assert!(new_r_entry && new_s_entry_1 && new_s_entry_2);
    }

    pub(crate) fn insert_recv(&mut self, now: Instant, key: KeyRecv, event: &EventRecv) {
        let valid_from = now
            .checked_add(event.after_duration)
            .expect("exceeded the range of the Instant");
        self.valid_from.insert(key, valid_from);

        let key = KeyDelayOrRecv::Recv(key);

        // resolution for the period from `now` to `valid_from`
        {
            let resolution = valid_from.saturating_duration_since(now) / RECV_RESOLUTION_DIVISOR;
            let r_entry = ResolutionEntry { key, resolution };
            let new_r_entry = self.resolution.insert(r_entry);
            let new_s_entry = self.schedule.insert(ScheduleEntry {
                at:    valid_from,
                event: ScheduledEvent::UnsetResolution(r_entry),
            });

            assert!(new_r_entry && new_s_entry);
        }

        if let Some(timeout) = event.before_duration {
            let valid_thru = now.checked_add(timeout).expect("oh don't be ridiculous!");

            let resolution =
                valid_thru.saturating_duration_since(valid_from) / RECV_RESOLUTION_DIVISOR;

            let r_entry = ResolutionEntry { resolution, key };

            let new_s_entry_1 = self.schedule.insert(ScheduleEntry {
                at:    valid_thru,
                event: ScheduledEvent::Ripe(key),
            });
            assert!(new_s_entry_1);

            if !resolution.is_zero() {
                let new_s_entry_2 = self.schedule.insert(ScheduleEntry {
                    at:    valid_from,
                    event: ScheduledEvent::SetResolution(r_entry),
                });
                let new_s_entry_3 = self.schedule.insert(ScheduleEntry {
                    at:    valid_thru,
                    event: ScheduledEvent::UnsetResolution(r_entry),
                });
                assert!(new_s_entry_2 && new_s_entry_3);
            }
        }
    }
}
