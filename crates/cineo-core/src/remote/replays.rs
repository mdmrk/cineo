use std::collections::{HashSet, VecDeque};

use super::{MAX_CLOCK_SKEW_MS, NONCE_LEN};

pub const MAX_REMEMBERED: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Clock,
    Replayed,
    Busy,
}

#[derive(Debug, Clone, Default)]
pub struct Replays {
    seen: HashSet<[u8; NONCE_LEN]>,
    order: VecDeque<(u64, [u8; NONCE_LEN])>,
}

impl Replays {
    pub fn admit(
        &mut self,
        now_ms: u64,
        time_ms: u64,
        nonce: [u8; NONCE_LEN],
    ) -> Result<(), Refusal> {
        if now_ms.abs_diff(time_ms) > MAX_CLOCK_SKEW_MS {
            return Err(Refusal::Clock);
        }
        while let Some(&(time, old)) = self.order.front() {
            if time.saturating_add(MAX_CLOCK_SKEW_MS) >= now_ms {
                break;
            }
            self.order.pop_front();
            self.seen.remove(&old);
        }
        if self.seen.contains(&nonce) {
            return Err(Refusal::Replayed);
        }
        if self.seen.len() >= MAX_REMEMBERED {
            return Err(Refusal::Busy);
        }
        self.seen.insert(nonce);
        self.order.push_back((time_ms, nonce));
        Ok(())
    }
}
