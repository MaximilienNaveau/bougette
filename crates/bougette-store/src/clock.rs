//! A hybrid logical clock (HLC): timestamps that order every change, even
//! when the system clock jumps back or two changes fall in the same
//! millisecond.
//!
//! A timestamp is the wall-clock time in milliseconds, a counter for
//! changes within the same millisecond, and the device's identifier to
//! break ties between devices. Its text form sorts like the timestamp
//! itself, so SQLite can compare timestamps as plain strings:
//! `0001789000000000-0000-1a2b3c4d5e6f7a8b`.

use std::time::{SystemTime, UNIX_EPOCH};

/// The clock of one device.
#[derive(Debug, Clone)]
pub struct Clock {
    /// This device's identifier: 16 hexadecimal digits.
    node: String,
    /// The milliseconds and counter of the last timestamp handed out.
    last_millis: u64,
    counter: u16,
}

impl Clock {
    /// A clock for device `node`, continuing after `last`, the latest
    /// timestamp already written (if any), so time never goes backwards
    /// after a restart.
    pub fn new(node: String, last: Option<&str>) -> Clock {
        // Read back the milliseconds and counter, or start from zero.
        // `and_then` chains steps that may each fail.
        let (last_millis, counter) = last
            .and_then(parse)
            .map(|(millis, counter, _)| (millis, counter))
            .unwrap_or((0, 0));
        Clock {
            node,
            last_millis,
            counter,
        }
    }

    /// Takes into account a timestamp received from another device, so the
    /// next ones handed out here sort after it.
    pub fn observe(&mut self, timestamp: &str) {
        // Tuples compare element by element, like `std::tuple`'s `<`: the
        // milliseconds first, then the counter.
        if let Some((millis, counter, _)) = parse(timestamp)
            && (millis, counter) > (self.last_millis, self.counter)
        {
            self.last_millis = millis;
            self.counter = counter;
        }
    }

    /// A new timestamp, later than every one handed out before.
    pub fn tick(&mut self) -> String {
        let now = now();
        if now > self.last_millis {
            // The wall clock moved on: use it, and restart the counter.
            self.last_millis = now;
            self.counter = 0;
        } else if self.counter < u16::MAX {
            // Same millisecond, or the clock went back: keep the last
            // time and count up instead.
            self.counter += 1;
        } else {
            // The counter is full (65,536 changes in the same millisecond,
            // e.g. a huge import while the clock is behind): move the
            // logical time one millisecond ahead rather than overflow.
            self.last_millis += 1;
            self.counter = 0;
        }
        // Fixed widths, padded with zeros, so text order is time order:
        // `{:016}` is 16 decimal digits, `{:04x}` 4 hexadecimal digits.
        format!(
            "{:016}-{:04x}-{}",
            self.last_millis, self.counter, self.node
        )
    }
}

/// The milliseconds, counter and device of a timestamp, or `None` if
/// `timestamp` doesn't have exactly the shape `tick` writes (text order is
/// time order only with those fixed widths).
// The returned `&str` borrows from `timestamp`: the elided lifetime ties
// them together.
pub fn parse(timestamp: &str) -> Option<(u64, u16, &str)> {
    // `splitn(3, '-')` cuts at the first two dashes only.
    let mut parts = timestamp.splitn(3, '-');
    let (millis, counter, node) = (parts.next()?, parts.next()?, parts.next()?);
    let digits = |text: &str, width: usize, hex: bool| {
        text.len() == width
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (hex && matches!(byte, b'a'..=b'f')))
    };
    if !digits(millis, 16, false) || !digits(counter, 4, true) || !digits(node, 16, true) {
        return None;
    }
    Some((
        millis.parse().ok()?,
        u16::from_str_radix(counter, 16).ok()?,
        node,
    ))
}

/// The wall-clock time in milliseconds since 1970. `duration_since` fails
/// only if the system clock is set before 1970; count that as 0.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// The device that wrote `timestamp`, if it is one.
pub fn node_of(timestamp: &str) -> Option<&str> {
    parse(timestamp).map(|(_, _, node)| node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_always_increase() {
        let mut clock = Clock::new("00000000000000aa".to_string(), None);
        let first = clock.tick();
        let second = clock.tick();
        assert!(second > first);
    }

    #[test]
    fn continues_after_a_timestamp_from_the_future() {
        // A timestamp far ahead of the wall clock, e.g. written by a device
        // whose clock was wrong: the next ones must still sort after it.
        let last = "9999999999999999-0007-00000000000000aa";
        let mut clock = Clock::new("00000000000000aa".to_string(), Some(last));
        let next = clock.tick();
        assert_eq!(next, "9999999999999999-0008-00000000000000aa");
        assert!(next.as_str() > last);
    }

    #[test]
    fn a_full_counter_moves_time_ahead() {
        let last = "9999999999999998-ffff-00000000000000aa";
        let mut clock = Clock::new("00000000000000aa".to_string(), Some(last));
        assert_eq!(clock.tick(), "9999999999999999-0000-00000000000000aa");
    }

    #[test]
    fn reads_timestamps_back() {
        let timestamp = "0001789000000000-002a-1a2b3c4d5e6f7a8b";
        assert_eq!(
            parse(timestamp),
            Some((1789000000000, 42, "1a2b3c4d5e6f7a8b"))
        );
        assert_eq!(node_of(timestamp), Some("1a2b3c4d5e6f7a8b"));
        assert_eq!(parse("1789000000000-002a-1a2b3c4d5e6f7a8b"), None);
        assert_eq!(parse("0001789000000000-+02a-1a2b3c4d5e6f7a8b"), None);
        assert_eq!(parse("0001789000000000-002a-1A2B3C4D5E6F7A8B"), None);
        assert_eq!(parse("0001789000000000-002a"), None);
    }

    #[test]
    fn catches_up_with_other_devices() {
        let mut clock = Clock::new("00000000000000aa".to_string(), None);
        clock.observe("9999999999999990-0003-00000000000000bb");
        assert_eq!(clock.tick(), "9999999999999990-0004-00000000000000aa");
        // An older timestamp changes nothing.
        clock.observe("0000000000000001-0000-00000000000000bb");
        assert_eq!(clock.tick(), "9999999999999990-0005-00000000000000aa");
    }
}
