//! What a System page shows, for a C or C++ app: each screen's brightness, the clock (and the
//! 12- or 24-hour choice every app shares), the battery, the volume. Where the system doesn't
//! let an app have one (the stock OS keeps brightness and volume to itself) the call says so
//! with -1.

use crate::{out, NibMenu};
use nibkit::screens::Screen;
use std::ffi::{c_char, c_int};

fn screen(s: c_int) -> Screen {
    if s == 0 {
        Screen::Top
    } else {
        Screen::Bottom
    }
}

/// A screen's brightness in percent (`screen` 0 the top, 1 the bottom); -1 where it can't be had.
#[no_mangle]
pub extern "C" fn nib_backlight_get(screen_: c_int) -> c_int {
    nibkit::backlight::get(screen(screen_)).map_or(-1, c_int::from)
}

/// Sets a screen's brightness at once (as a slider moves).
#[no_mangle]
pub extern "C" fn nib_backlight_set(screen_: c_int, percent: c_int) {
    nibkit::backlight::set(screen(screen_), percent.clamp(0, 100) as u8);
}

/// Keeps both screens' brightness for the next start (when a slider is let go of).
#[no_mangle]
pub extern "C" fn nib_backlight_save() {
    nibkit::backlight::save();
}

/// Whether the clock is shown 12-hour (1) or 24-hour (0): the choice every app shares.
#[no_mangle]
pub extern "C" fn nib_clock_hour12() -> c_int {
    nibkit::clock::hour12() as c_int
}

/// Sets the 12- or 24-hour choice for every app. 0 on success, -1 on failure.
#[no_mangle]
pub extern "C" fn nib_clock_set_hour12(on: c_int) -> c_int {
    if nibkit::clock::set_hour12(on != 0).is_ok() {
        0
    } else {
        -1
    }
}

/// The time now as the bar shows it ("17:05" or "5:05 PM").
///
/// # Safety
/// `m` is a live `NibMenu`; the result is valid until the next call on `m`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_clock(m: *mut NibMenu) -> *const c_char {
    let now = libc::time_t::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    )
    .unwrap_or(0);
    // SAFETY: localtime_r fills the struct given.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    let shown = nibkit::clock::format(tm.tm_hour as u32, tm.tm_min as u32, nibkit::clock::hour12());
    // SAFETY: the caller's promise.
    match unsafe { m.as_mut() } {
        Some(m) => out(&mut m.scratch, Some(shown)),
        None => std::ptr::null(),
    }
}

/// The battery's charge in percent, and whether it is charging (`charging` NULL or writable);
/// -1 where there is none.
///
/// # Safety
/// `charging` is NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn nib_battery(charging: *mut c_int) -> c_int {
    let Some(b) = nibkit::battery::read() else {
        return -1;
    };
    // SAFETY: the caller's promise.
    if let Some(c) = unsafe { charging.as_mut() } {
        *c = (b.state == nibkit::battery::State::Charging) as c_int;
    }
    c_int::from(b.percent)
}

/// The device's volume in percent; -1 where an app can't have it.
#[no_mangle]
pub extern "C" fn nib_volume_get() -> c_int {
    nibkit::device::volume().map_or(-1, c_int::from)
}

#[no_mangle]
pub extern "C" fn nib_volume_set(percent: c_int) {
    nibkit::device::set_volume(percent.clamp(0, 100) as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_and_what_a_computer_lacks() {
        let m = crate::nib_menu_new();
        // SAFETY: m is live; the string is read at once.
        let clock = unsafe { std::ffi::CStr::from_ptr(nib_menu_clock(m)) }
            .to_string_lossy()
            .into_owned();
        assert!(clock.contains(':'), "{clock}");
        assert!((0..=1).contains(&nib_clock_hour12()));
        // On a computer there's no backlight or battery to have: -1, not a guess.
        if !nibkit::on_device() {
            assert_eq!(nib_backlight_get(0), -1);
        }
        // SAFETY: m came from nib_menu_new.
        unsafe { crate::nib_menu_free(m) };
    }
}
