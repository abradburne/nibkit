//! The indicator for a level the volume keys just set, over a game that is playing: nibkit's
//! `levels` (what nibOS announced) and `menu::level` (the panel at the bottom right, fading away
//! after a moment), the one every app's indicator is; this is their C face.
//!
//! A `NibLevels` watches nibOS's announcements from when it is made. Once a frame the app asks
//! `nib_levels_tick` what is on show (0: nothing; another number whenever the picture changes,
//! the fade's steps included) and lays `nib_levels_render`'s picture over its frame where it
//! says, the opacity in its alpha.

use crate::toast::{dim, instant, put};
use nibkit::levels::{self, Level, LevelWatch, What};
use nibkit::menu::level::Indicator;
use nibkit::menu::raster::Fonts;
use nibkit::menu::scale_for;
use std::ffi::c_int;
use std::time::Instant;

/// The volume (the header's NIB_LEVEL_VOLUME; anything but brightness is taken for it).
#[cfg_attr(not(test), allow(dead_code))]
pub const VOLUME: c_int = 0;
/// Both screens' brightness.
pub const BRIGHTNESS: c_int = 1;

/// Behind the C header's opaque `NibLevels`: the watch on nibOS's announcements (none where it
/// makes none), the indicator, the fonts (loaded when it is first drawn), the caller's clock as
/// nibkit's, and what was last on show.
#[derive(Default)]
pub struct NibLevels {
    watch: Option<LevelWatch>,
    indicator: Indicator,
    fonts: Option<Fonts>,
    clock: Option<(u64, Instant)>,
    /// The opacity at the last tick, in 32nds.
    step: u8,
    /// Counts what was shown: a level, then each step of its fade.
    serial: u64,
    shown: Option<(Level, u8)>,
}

/// # Safety
/// `p` is NULL or live.
unsafe fn with<R>(p: *mut NibLevels, or: R, f: impl FnOnce(&mut NibLevels) -> R) -> R {
    // SAFETY: the caller's promise.
    match unsafe { p.as_mut() } {
        Some(l) => f(l),
        None => or,
    }
}

impl NibLevels {
    fn tick(&mut self, now: Instant) -> u64 {
        if let Some(level) = self.watch.as_mut().and_then(LevelWatch::changed) {
            self.indicator.show(level, now);
        }
        let opacity = self.indicator.tick(now);
        self.step = (opacity * 32.0).round() as u8;
        let now_shown = self.indicator.level().filter(|_| self.step > 0);
        let now_shown = now_shown.map(|l| (l, self.step));
        if now_shown != self.shown {
            self.shown = now_shown;
            self.serial += 1;
        }
        if self.shown.is_some() {
            self.serial
        } else {
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn nib_levels_new() -> *mut NibLevels {
    Box::into_raw(Box::new(NibLevels {
        watch: levels::watch(),
        ..NibLevels::default()
    }))
}

/// # Safety
/// `l` is NULL or came from `nib_levels_new` and isn't used after.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_free(l: *mut NibLevels) {
    if !l.is_null() {
        // SAFETY: the caller's promise.
        drop(unsafe { Box::from_raw(l) });
    }
}

/// Whether nibOS's announcements are being watched: 0 where the system makes none (the stock
/// OS) or nibos-hw hasn't made the folder.
///
/// # Safety
/// `l` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_watching(l: *const NibLevels) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { l.as_ref() }.is_some_and(|l| l.watch.is_some()) as c_int
}

/// Shows a level the app set itself (`what` NIB_LEVEL_VOLUME or NIB_LEVEL_BRIGHTNESS), from
/// `now_ms`, as if nibOS had announced it.
///
/// # Safety
/// `l` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_show(
    l: *mut NibLevels,
    what: c_int,
    percent: c_int,
    now_ms: u64,
) {
    let what = if what == BRIGHTNESS {
        What::Brightness
    } else {
        What::Volume
    };
    let level = Level {
        what,
        percent: percent.clamp(0, 100) as u8,
    };
    // SAFETY: the caller's promise.
    unsafe {
        with(l, (), |l| {
            let now = instant(&mut l.clock, now_ms);
            l.indicator.show(level, now);
        })
    }
}

/// Takes in what nibOS announced and works out what is on show at `now_ms` (any clock in
/// milliseconds that keeps going while the game is paused): 0 for nothing; otherwise a number
/// that changes whenever the picture does (another level, each step of the fade).
///
/// # Safety
/// `l` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_tick(l: *mut NibLevels, now_ms: u64) -> u64 {
    // SAFETY: the caller's promise.
    unsafe {
        with(l, 0, |l| {
            let now = instant(&mut l.clock, now_ms);
            l.tick(now)
        })
    }
}

/// The indicator as the last tick found it, drawn for a `frame_w` x `frame_h` frame with its
/// opacity in its alpha (RGBA, straight alpha, rows top first), and where on the frame its top
/// left goes; NULL with nothing on show.
///
/// # Safety
/// `l` is NULL or live; the out pointers are NULL or writable. The result stays valid until the
/// next call on `l`.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_render(
    l: *mut NibLevels,
    frame_w: c_int,
    frame_h: c_int,
    width: *mut c_int,
    height: *mut c_int,
    x: *mut c_int,
    y: *mut c_int,
) -> *const u8 {
    let frame = (dim(frame_w), dim(frame_h));
    let scale = scale_for(frame.0, frame.1);
    // SAFETY: the caller's promise.
    unsafe {
        with(l, std::ptr::null(), |l| {
            if l.shown.is_none() {
                return std::ptr::null();
            }
            let fonts = l.fonts.get_or_insert_with(Fonts::new);
            let Some(panel) = l.indicator.draw(fonts, scale) else {
                return std::ptr::null();
            };
            let (w, h, at) = (panel.width(), panel.height(), panel.place(frame));
            let opacity = f32::from(l.step) / 32.0;
            let Some(px) = l.indicator.faded(fonts, scale, opacity) else {
                return std::ptr::null();
            };
            // SAFETY: the caller's promise that these are NULL or writable.
            put(width, w);
            put(height, h);
            put(x, at.0);
            put(y, at.1);
            px.as_ptr()
        })
    }
}

/// How far up, in pixels of a `frame_w` x `frame_h` frame, a toast at its corner should move
/// while the indicator shows, to sit above it; 0 with nothing on show.
///
/// # Safety
/// `l` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_lift(
    l: *const NibLevels,
    frame_w: c_int,
    frame_h: c_int,
) -> c_int {
    // SAFETY: the caller's promise.
    let Some(l) = (unsafe { l.as_ref() }) else {
        return 0;
    };
    if l.shown.is_none() {
        return 0;
    }
    l.indicator.lift(scale_for(dim(frame_w), dim(frame_h))) as c_int
}

/// Hides the indicator at once (a menu opens over the game).
///
/// # Safety
/// `l` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_levels_hide(l: *mut NibLevels) {
    // SAFETY: the caller's promise.
    unsafe {
        with(l, (), |l| {
            l.indicator.hide();
            l.shown = None;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_fades_and_is_drawn_where_toasts_go() {
        let l = nib_levels_new();
        unsafe {
            assert_eq!(nib_levels_tick(l, 1000), 0);
            assert!(nib_levels_render(
                l,
                1024,
                768,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            )
            .is_null());
            nib_levels_show(l, VOLUME, 40, 1000);
            let first = nib_levels_tick(l, 1000);
            assert_ne!(first, 0);
            assert_eq!(
                nib_levels_tick(l, 1500),
                first,
                "the same picture while it holds"
            );
            let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
            let px = nib_levels_render(l, 1024, 768, &mut w, &mut h, &mut x, &mut y);
            assert!(!px.is_null());
            assert_eq!((x + w, y + h), (1024 - 16, 768 - 16));
            let mid = *px.add(((h / 2 * w + w / 2) * 4 + 3) as usize);
            assert!(mid > 200);
            assert!(nib_levels_lift(l, 1024, 768) > h);
            // Fading: another picture, fainter.
            let fading = nib_levels_tick(l, 1000 + 1500 + 250);
            assert_ne!(fading, first);
            let px = nib_levels_render(l, 1024, 768, &mut w, &mut h, &mut x, &mut y);
            assert!(*px.add(((h / 2 * w + w / 2) * 4 + 3) as usize) < mid);
            assert_eq!(nib_levels_tick(l, 1000 + 2000), 0);
            assert_eq!(nib_levels_lift(l, 1024, 768), 0);
            nib_levels_free(l);
        }
    }
}
