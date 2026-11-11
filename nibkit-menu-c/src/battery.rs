//! nibOS's low-battery alerts over a game that is playing: nibkit's `battery::Alerts` (what
//! nibOS says) and `menu::battery` (the indicator at the top right that stays while the battery
//! is very low); this is their C face.
//!
//! A `NibBattery` watches nibOS's alerts from when it is made. Once a frame the app asks
//! `nib_battery_tick` what is on show (0: nothing; another number whenever the picture changes),
//! takes `nib_battery_news` for a toast when the battery has just got worse, and lays
//! `nib_battery_render`'s picture over its frame where it says.

use crate::toast::{dim, put};
use nibkit::battery::{Alert, Alerts};
use nibkit::menu::battery::Indicator;
use nibkit::menu::raster::Fonts;
use nibkit::menu::scale_for;
use std::ffi::{c_char, c_int, CString};

/// Behind the C header's opaque `NibBattery`: the alerts, the indicator, the fonts (loaded when
/// it is first drawn), what was last on show, and news not yet taken.
pub struct NibBattery {
    alerts: Alerts,
    indicator: Indicator,
    fonts: Option<Fonts>,
    serial: u64,
    shown: Option<u8>,
    news: Option<Alert>,
    words: (CString, CString),
}

/// # Safety
/// `p` is NULL or live.
unsafe fn with<R>(p: *mut NibBattery, or: R, f: impl FnOnce(&mut NibBattery) -> R) -> R {
    // SAFETY: the caller's promise.
    match unsafe { p.as_mut() } {
        Some(b) => f(b),
        None => or,
    }
}

impl NibBattery {
    fn new(alerts: Alerts) -> NibBattery {
        NibBattery {
            alerts,
            indicator: Indicator::new(),
            fonts: None,
            serial: 0,
            shown: None,
            news: None,
            words: (CString::default(), CString::default()),
        }
    }

    /// After the alerts have taken in what nibOS says (`news`: the worse level, if any).
    fn after(&mut self, news: Option<Alert>) -> u64 {
        if news.is_some() {
            self.news = news;
        }
        match Indicator::wanted(self.alerts.now()) {
            Some(p) => self.indicator.show(p),
            None => self.indicator.hide(),
        }
        let now = self.indicator.percent();
        if now != self.shown {
            self.shown = now;
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
pub extern "C" fn nib_battery_new() -> *mut NibBattery {
    Box::into_raw(Box::new(NibBattery::new(Alerts::new())))
}

/// # Safety
/// `b` is NULL or came from `nib_battery_new` and isn't used after.
#[no_mangle]
pub unsafe extern "C" fn nib_battery_free(b: *mut NibBattery) {
    if !b.is_null() {
        // SAFETY: the caller's promise.
        drop(unsafe { Box::from_raw(b) });
    }
}

/// Takes in what nibOS said (cheap: the alert file is read only when it changes) and works out
/// what is on show: 0 for nothing; otherwise a number that changes whenever the picture does.
///
/// # Safety
/// `b` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_battery_tick(b: *mut NibBattery) -> u64 {
    // SAFETY: the caller's promise.
    unsafe {
        with(b, 0, |b| {
            let news = b.alerts.take();
            b.after(news)
        })
    }
}

/// Once per time the battery gets worse (10%, 5%, empty): 1, with a toast's title and the line
/// under it (NUL-terminated, valid until the next call on `b`): show it as a problem toast. 0
/// with nothing new.
///
/// # Safety
/// `b` is NULL or live; the out pointers are NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn nib_battery_news(
    b: *mut NibBattery,
    title: *mut *const c_char,
    detail: *mut *const c_char,
) -> c_int {
    // SAFETY: the caller's promise.
    unsafe {
        with(b, 0, |b| {
            let Some(alert) = b.news.take() else {
                return 0;
            };
            let clean = |s: String| CString::new(s.replace('\0', "")).unwrap_or_default();
            b.words = (clean(alert.title().to_owned()), clean(alert.detail()));
            // SAFETY: the caller's promise that these are NULL or writable.
            if let Some(t) = title.as_mut() {
                *t = b.words.0.as_ptr();
            }
            if let Some(d) = detail.as_mut() {
                *d = b.words.1.as_ptr();
            }
            1
        })
    }
}

/// The indicator as the last tick found it, drawn for a `frame_w` x `frame_h` frame (RGBA,
/// straight alpha, rows top first), and where on the frame its top left goes (the top right
/// corner); NULL with nothing on show.
///
/// # Safety
/// `b` is NULL or live; the out pointers are NULL or writable. The result stays valid until the
/// next call on `b`.
#[no_mangle]
pub unsafe extern "C" fn nib_battery_render(
    b: *mut NibBattery,
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
        with(b, std::ptr::null(), |b| {
            if b.shown.is_none() {
                return std::ptr::null();
            }
            let fonts = b.fonts.get_or_insert_with(Fonts::new);
            let Some(panel) = b.indicator.draw(fonts, scale) else {
                return std::ptr::null();
            };
            let at = panel.place(frame);
            // SAFETY: the caller's promise that these are NULL or writable.
            put(width, panel.width());
            put(height, panel.height());
            put(x, at.0);
            put(y, at.1);
            panel.rgba().as_ptr()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn news_once_and_the_indicator_while_very_low() {
        let b = Box::into_raw(Box::new(NibBattery::new(Alerts::new())));
        unsafe {
            // Off nibOS: nothing, ever.
            assert_eq!(nib_battery_tick(b), 0);
            let (mut t, mut d) = (std::ptr::null(), std::ptr::null());
            assert_eq!(nib_battery_news(b, &mut t, &mut d), 0);
            // nibOS warns: a toast, no indicator.
            let news = (*b).alerts.update(Some(Alert::Warn(9)));
            assert_eq!((*b).after(news), 0);
            assert_eq!(nib_battery_news(b, &mut t, &mut d), 1);
            let title = std::ffi::CStr::from_ptr(t).to_str().unwrap();
            assert_eq!(title, "Battery low");
            assert_eq!(nib_battery_news(b, &mut t, &mut d), 0, "once");
            // Very low: a toast, and the indicator stays.
            let news = (*b).alerts.update(Some(Alert::Low(5)));
            let first = (*b).after(news);
            assert_ne!(first, 0);
            assert_eq!(nib_battery_news(b, &mut t, &mut d), 1);
            let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
            let px = nib_battery_render(b, 1024, 768, &mut w, &mut h, &mut x, &mut y);
            assert!(!px.is_null());
            assert_eq!((x + w, y), (1024 - 16, 16));
            // A percent less: another picture, no news.
            let news = (*b).alerts.update(Some(Alert::Low(4)));
            let next = (*b).after(news);
            assert!(next != 0 && next != first);
            assert_eq!(nib_battery_news(b, &mut t, &mut d), 0);
            // The charger: gone.
            let news = (*b).alerts.update(None);
            assert_eq!((*b).after(news), 0);
            assert!(nib_battery_render(b, 1024, 768, &mut w, &mut h, &mut x, &mut y).is_null());
            nib_battery_free(b);
        }
    }
}
