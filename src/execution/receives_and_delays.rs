use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use tokio::time::Instant;

use crate::execution::{EventDelay, EventRecv, KeyDelay, KeyRecv};

const RECV_RESOLUTION_DIVISOR: u32 = 1000;

#[derive(Default)]
pub(crate) struct ReceivesAndDelays {
    upcoming:   BTreeSet<UpcomingEntry>,
    resolution: BTreeSet<ResolutionEntry>,
    valid_from: HashMap<KeyRecv, Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum KeyDelayOrRecv {
    Delay(KeyDelay),
    Recv(KeyRecv),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct UpcomingEntry {
    at:         Instant,
    key:        KeyDelayOrRecv,
    resolution: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ResolutionEntry {
    resolution: Duration,
    key:        KeyDelayOrRecv,
    at:         Instant,
}

impl ReceivesAndDelays {
    pub(crate) fn next_sleep_until(&self, now: Instant) -> Option<Instant> {
        assert_eq!(self.resolution.len(), self.upcoming.len());

        let ResolutionEntry { resolution, .. } = self.resolution.first().copied()?;
        let one_step_after_now = now.checked_add(resolution).unwrap_or(now);

        let UpcomingEntry { at, .. } = self.upcoming.first().copied()?;

        Some(one_step_after_now.min(at))
    }

    pub(crate) fn select_ripe_keys(&mut self, now: Instant) -> Vec<KeyDelayOrRecv> {
        assert_eq!(self.resolution.len(), self.upcoming.len());

        let mut out = vec![];

        while self.upcoming.first().is_some_and(|ue| ue.at <= now) {
            let UpcomingEntry {
                at,
                key,
                resolution,
            } = self
                .upcoming
                .pop_first()
                .expect("wouldn't enter this loop otherwise");
            let actually_removed = self.resolution.remove(&ResolutionEntry {
                resolution,
                key,
                at,
            });
            assert!(actually_removed);

            out.push(key);
        }

        out
    }

    pub(crate) fn remove_recv_by_key(&mut self, key: KeyRecv) -> Instant {
        let valid_from = self
            .valid_from
            .remove(&key)
            .expect("recv-key should have existed");
        let key = KeyDelayOrRecv::Recv(key);
        self.upcoming.retain(|ue| ue.key != key);
        self.resolution.retain(|re| re.key != key);
        valid_from
    }

    pub(crate) fn insert_delay(&mut self, now: Instant, key: KeyDelay, event: &EventDelay) {
        let delay_for = event.delay_for;
        let resolution = event.delay_step;
        let at = now.checked_add(delay_for).expect("please pretty please");
        let key = KeyDelayOrRecv::Delay(key);

        let new_d_entry = self.upcoming.insert(UpcomingEntry {
            at,
            key,
            resolution,
        });
        let new_s_entry = self.resolution.insert(ResolutionEntry {
            resolution,
            key,
            at,
        });

        assert!(new_d_entry && new_s_entry);
    }

    pub(crate) fn insert_recv(&mut self, now: Instant, key: KeyRecv, event: &EventRecv) {
        let valid_from = now
            .checked_add(event.after_duration)
            .expect("exceeded the range of the Instant");
        self.valid_from.insert(key, valid_from);

        if let Some(timeout) = event.before_duration {
            let valid_thru = now.checked_add(timeout).expect("oh don't be ridiculous!");

            let key = KeyDelayOrRecv::Recv(key);
            let resolution =
                valid_thru.saturating_duration_since(valid_from) / RECV_RESOLUTION_DIVISOR;

            let u_entry = UpcomingEntry {
                at: valid_thru,
                key,
                resolution,
            };
            let r_entry = ResolutionEntry {
                resolution,
                key,
                at: valid_thru,
            };

            let new_u_entry = self.upcoming.insert(u_entry);
            assert!(new_u_entry);

            if !resolution.is_zero() {
                let new_r_entry = self.resolution.insert(r_entry);
                assert!(new_r_entry);
            }
        }
    }
}
