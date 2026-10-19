//! The DS+'s buttons by what is printed on them. Its face buttons are in Nintendo's layout (A on
//! the right, B at the bottom, X at the top, Y on the left), and SDL's game controller API names
//! buttons by position on an Xbox pad, so SDL's "A" is the DS+'s B. [`printed`] turns SDL's
//! name for a face button into the DS+'s label:
//!
//! ```
//! use nibkit::input::{printed, Face};
//! // SDL reported its "B" (the right-hand button): the DS+'s A.
//! assert_eq!(printed(Face::B), Face::A);
//! ```

use crate::platform;

/// A face button.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Face {
    A,
    B,
    X,
    Y,
}

/// The DS+'s label on the face button SDL calls `sdl`.
pub fn printed(sdl: Face) -> Face {
    platform::printed(sdl)
}

/// SDL's name for the face button labelled `label`.
pub fn sdl_name(label: Face) -> Face {
    [Face::A, Face::B, Face::X, Face::Y]
        .into_iter()
        .find(|&f| printed(f) == label)
        .unwrap_or(label)
}

/// The gamepad's name, where it is known: on nibOS, nibos-input's merged gamepad.
pub const GAMEPAD_NAME: Option<&str> = platform::GAMEPAD_NAME;

/// Gamepad mapping files to load (SDL_GameControllerAddMappingsFromFile) before opening the
/// gamepad: the stock emulators' on the stock OS; none on nibOS.
pub const CONTROLLER_MAPPINGS: &[&str] = platform::CONTROLLER_MAPPINGS;

/// Menu (SDL's "Guide") arrives as a press and a release together when it is let go of: nibOS
/// holds it back in case it is the start of a shortcut (Menu + volume: brightness).
pub const MENU_ON_RELEASE: bool = platform::MENU_ON_RELEASE;

/// Back is a key of its own, not a gamepad button. Through sdl2-compat it arrives as SDL2's
/// "Sleep" keycode; a translating sdl2-compat would say "AC Back". Bind both.
pub const BACK_KEYCODES: &[&str] = &["Sleep", "AC Back"];

/// A held button repeating, as a held D-pad or stick moves through a list: the press acts at
/// once (the caller does that), then after `first` the button repeats every `every`, twice as
/// often once it has been held [`Repeat::FASTER_AFTER`]. Repeats are never stored up: if the
/// app falls behind (an animation that can't keep up, a slow frame), the next one is due from
/// when it was taken, not from when it was meant to be. One button repeats at a time: a new
/// press takes over.
///
/// ```
/// use nibkit::input::Repeat;
/// use std::time::{Duration, Instant};
/// let mut r = Repeat::new(Duration::from_millis(340), Duration::from_millis(85));
/// let t = Instant::now();
/// r.press("down", t);
/// assert_eq!(r.due(t + Duration::from_millis(100)), None);
/// assert_eq!(r.due(t + Duration::from_millis(340)), Some("down"));
/// r.release("down");
/// assert_eq!(r.due(t + Duration::from_secs(1)), None);
/// ```
#[derive(Clone, Debug)]
pub struct Repeat<K> {
    first: std::time::Duration,
    every: std::time::Duration,
    held: Option<Held<K>>,
}

#[derive(Clone, Debug)]
struct Held<K> {
    key: K,
    since: std::time::Instant,
    due: std::time::Instant,
}

impl<K: Copy + PartialEq> Repeat<K> {
    /// Held this long, a button repeats twice as often.
    pub const FASTER_AFTER: std::time::Duration = std::time::Duration::from_millis(1500);

    pub fn new(first: std::time::Duration, every: std::time::Duration) -> Repeat<K> {
        Repeat {
            first,
            every,
            held: None,
        }
    }

    /// Changes the timing (a setting changed), keeping what is held.
    pub fn set_times(&mut self, first: std::time::Duration, every: std::time::Duration) {
        (self.first, self.every) = (first, every);
    }

    /// `key` went down at `now`: it repeats from `first` on.
    pub fn press(&mut self, key: K, now: std::time::Instant) {
        self.held = Some(Held {
            key,
            since: now,
            due: now + self.first,
        });
    }

    /// `key` came up: if it was the one repeating, nothing repeats now.
    pub fn release(&mut self, key: K) {
        if self.held.as_ref().is_some_and(|h| h.key == key) {
            self.held = None;
        }
    }

    /// Nothing repeats any more (the page changed, a menu opened).
    pub fn stop(&mut self) {
        self.held = None;
    }

    /// The button held, if any.
    pub fn held(&self) -> Option<K> {
        self.held.as_ref().map(|h| h.key)
    }

    /// The held button once more, if a repeat is due at `now`: at most one a call, the next
    /// one due a whole interval later.
    pub fn due(&mut self, now: std::time::Instant) -> Option<K> {
        let held = self.held.as_mut()?;
        if now < held.due {
            return None;
        }
        let every = if now.duration_since(held.since) >= Self::FASTER_AFTER {
            self.every / 2
        } else {
            self.every
        };
        held.due = now + every;
        Some(held.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats() {
        use std::time::{Duration, Instant};
        let ms = Duration::from_millis;
        let mut r = Repeat::new(ms(340), ms(85));
        let t = Instant::now();
        assert_eq!(r.due(t), None);
        r.press('d', t);
        // Not before the first delay; then one every 85 ms.
        assert_eq!(r.due(t + ms(339)), None);
        assert_eq!(r.due(t + ms(340)), Some('d'));
        assert_eq!(r.due(t + ms(400)), None);
        assert_eq!(r.due(t + ms(425)), Some('d'));
        // Fallen behind by a second: one repeat, not twelve, and the next a whole interval on.
        assert_eq!(r.due(t + ms(1425)), Some('d'));
        assert_eq!(r.due(t + ms(1426)), None);
        assert_eq!(r.due(t + ms(1509)), None);
        assert_eq!(r.due(t + ms(1510)), Some('d'));
        // Held past 1.5 s: twice as often (42.5 ms).
        assert_eq!(r.due(t + ms(1552)), None);
        assert_eq!(r.due(t + ms(1553)), Some('d'));
        // Another button takes over; letting go of the first does nothing then.
        r.press('u', t + ms(1600));
        r.release('d');
        assert_eq!(r.held(), Some('u'));
        assert_eq!(r.due(t + ms(1940)), Some('u'));
        r.release('u');
        assert_eq!(r.due(t + ms(5000)), None);
        // Counting how many come in a second held, from the press: 1 + (1000 - 340) / 85.
        let mut r = Repeat::new(ms(340), ms(85));
        r.press('r', t);
        let n = (0..=1000).filter(|&m| r.due(t + ms(m)).is_some()).count();
        assert_eq!(n, 8);
    }

    #[test]
    fn labels() {
        // A and B are swapped on both systems; X and Y on the stock OS only.
        assert_eq!(printed(Face::B), Face::A);
        assert_eq!(printed(Face::A), Face::B);
        for f in [Face::A, Face::B, Face::X, Face::Y] {
            assert_eq!(printed(sdl_name(f)), f);
        }
    }
}
