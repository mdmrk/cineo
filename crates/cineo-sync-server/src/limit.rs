use std::collections::HashMap;
use std::net::IpAddr;

pub const REQUESTS_PER_MINUTE: u32 = 120;
pub const SPACES_PER_HOUR: u32 = 10;
const MINUTE_MS: u64 = 60_000;
const HOUR_MS: u64 = 3_600_000;
const PRUNE_AT: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct Window {
    start_ms: u64,
    count: u32,
}

#[derive(Debug, Default)]
pub struct RateLimiter {
    requests: HashMap<IpAddr, Window>,
    creations: HashMap<IpAddr, Window>,
}

impl RateLimiter {
    pub fn request(&mut self, ip: IpAddr, now_ms: u64) -> Result<(), u64> {
        let window = current(&mut self.requests, ip, now_ms, MINUTE_MS);
        if window.count >= REQUESTS_PER_MINUTE {
            return Err(retry_after_s(window, now_ms, MINUTE_MS));
        }
        window.count += 1;
        Ok(())
    }

    pub fn may_create(&self, ip: IpAddr, now_ms: u64) -> bool {
        self.creations
            .get(&ip)
            .is_none_or(|w| now_ms >= w.start_ms + HOUR_MS || w.count < SPACES_PER_HOUR)
    }

    pub fn created(&mut self, ip: IpAddr, now_ms: u64) {
        current(&mut self.creations, ip, now_ms, HOUR_MS).count += 1;
    }
}

fn current(
    windows: &mut HashMap<IpAddr, Window>,
    ip: IpAddr,
    now_ms: u64,
    length_ms: u64,
) -> &mut Window {
    if windows.len() >= PRUNE_AT {
        windows.retain(|_, w| now_ms < w.start_ms + length_ms);
    }
    let window = windows.entry(ip).or_insert(Window {
        start_ms: now_ms,
        count: 0,
    });
    if now_ms >= window.start_ms + length_ms {
        *window = Window {
            start_ms: now_ms,
            count: 0,
        };
    }
    window
}

fn retry_after_s(window: &Window, now_ms: u64, length_ms: u64) -> u64 {
    (window.start_ms + length_ms)
        .saturating_sub(now_ms)
        .div_ceil(1000)
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const A: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    const B: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2));

    #[test]
    fn requests_are_limited_per_address_and_minute() {
        let mut limiter = RateLimiter::default();
        for _ in 0..REQUESTS_PER_MINUTE {
            assert_eq!(limiter.request(A, 1_000), Ok(()));
        }
        assert_eq!(limiter.request(A, 31_000), Err(30));
        assert_eq!(limiter.request(B, 31_000), Ok(()), "another address");
        assert_eq!(limiter.request(A, 61_000), Ok(()), "the next minute");
    }

    #[test]
    fn new_spaces_are_limited_per_address_and_hour() {
        let mut limiter = RateLimiter::default();
        for _ in 0..SPACES_PER_HOUR {
            assert!(limiter.may_create(A, 0));
            limiter.created(A, 0);
        }
        assert!(!limiter.may_create(A, HOUR_MS - 1));
        assert!(limiter.may_create(B, 0));
        assert!(limiter.may_create(A, HOUR_MS));
    }
}
