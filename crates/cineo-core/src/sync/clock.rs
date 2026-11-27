#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    pub physical_ms: u64,
    pub counter: u32,
    pub device: DeviceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    latest: Stamp,
}

impl Clock {
    pub const fn new(device: DeviceId) -> Self {
        Self::resume(Stamp {
            physical_ms: 0,
            counter: 0,
            device,
        })
    }

    pub const fn resume(latest: Stamp) -> Self {
        Self { latest }
    }

    pub const fn latest(&self) -> Stamp {
        self.latest
    }

    pub const fn device(&self) -> DeviceId {
        self.latest.device
    }

    pub fn tick(&mut self, now_ms: u64) -> Stamp {
        let latest = &mut self.latest;
        if now_ms > latest.physical_ms {
            latest.physical_ms = now_ms;
            latest.counter = 0;
        } else if let Some(counter) = latest.counter.checked_add(1) {
            latest.counter = counter;
        } else {
            latest.physical_ms = latest.physical_ms.saturating_add(1);
            latest.counter = 0;
        }
        self.latest
    }

    pub fn observe(&mut self, remote: Stamp) {
        let latest = &mut self.latest;
        if (remote.physical_ms, remote.counter) > (latest.physical_ms, latest.counter) {
            latest.physical_ms = remote.physical_ms;
            latest.counter = remote.counter;
        }
    }
}
