//! The Help page for a C or C++ app's pause menu: nibkit's `menu::help` (What's new in the
//! running release and the system's help topics, each read as a page of text), as DSlot's Help
//! section shows it. Where the system keeps no such files (the stock OS), `nib_help_new` gives
//! NULL.

use crate::{out, NibMenu};
use nibkit::menu::help::Help;
use std::ffi::{c_char, c_int, CString};

/// The page's state while the menu is open.
pub struct NibHelp {
    page: Help,
    scratch: CString,
    hint: CString,
}

/// Runs `f` on the page behind `h`, if it isn't NULL.
///
/// # Safety
/// `h` is NULL or a live `NibHelp` not used elsewhere during the call.
unsafe fn with<T>(h: *mut NibHelp, or: T, f: impl FnOnce(&mut NibHelp) -> T) -> T {
    // SAFETY: the caller's promise.
    match unsafe { h.as_mut() } {
        Some(h) => f(h),
        None => or,
    }
}

/// The page, with the system's files as they are now; NULL where there are none. Free it with
/// `nib_help_free` when the menu closes.
#[no_mangle]
pub extern "C" fn nib_help_new() -> *mut NibHelp {
    let page = Help::load();
    if page.is_empty() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(NibHelp {
        page,
        scratch: CString::default(),
        hint: CString::default(),
    }))
}

/// # Safety
/// `h` came from `nib_help_new` and isn't used again (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn nib_help_free(h: *mut NibHelp) {
    if !h.is_null() {
        // SAFETY: the caller's promise: it is ours, from Box::into_raw.
        drop(unsafe { Box::from_raw(h) });
    }
}

/// The page's title (the topic being read, or "Help"), for `nib_menu_begin`.
///
/// # Safety
/// `h` is a live `NibHelp`; the result is valid until the next call on `h`.
#[no_mangle]
pub unsafe extern "C" fn nib_help_title(h: *mut NibHelp) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(h, std::ptr::null(), |h| {
            let title = h.page.title();
            out(&mut h.scratch, Some(title))
        })
    }
}

/// The page's hint line, for `nib_menu_begin`: the list's, or the one for reading.
///
/// # Safety
/// `h` is a live `NibHelp`; the result is valid until the next call of this on `h`.
#[no_mangle]
pub unsafe extern "C" fn nib_help_hint(h: *mut NibHelp) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(h, std::ptr::null(), |h| {
            let hint = h.page.hint().to_owned();
            out(&mut h.hint, Some(hint))
        })
    }
}

/// Puts the page on `m` (after `nib_menu_begin` and the tabs): its title, hint and rows, the
/// list or the topic's lines; a topic is read with no highlight, scrolled by the D-pad and by a
/// finger (`nib_menu_drag`). The rows are the page's whole: returns 0, the index of its first.
///
/// # Safety
/// `h` and `m` are live.
#[no_mangle]
pub unsafe extern "C" fn nib_help_fill(h: *mut NibHelp, m: *mut NibMenu) -> c_int {
    // SAFETY: the caller's promise.
    let (Some(h), Some(m)) = (unsafe { h.as_mut() }, unsafe { m.as_mut() }) else {
        return 0;
    };
    h.page.fill(&mut m.menu);
    0
}

/// NIB_EFFECT_CHOOSE on the list's row `row`: reads that topic. 1 if it opened one; fill the
/// page again to show it.
///
/// # Safety
/// `h` is live.
#[no_mangle]
pub unsafe extern "C" fn nib_help_choose(h: *mut NibHelp, row: c_int) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { with(h, 0, |h| (row >= 0 && h.page.choose(row as usize)) as c_int) }
}

/// NIB_EFFECT_BACK: from a topic back to the list, 1; 0 on the list, where B is the app's (it
/// leaves the page, or closes the menu).
///
/// # Safety
/// `h` is live.
#[no_mangle]
pub unsafe extern "C" fn nib_help_back(h: *mut NibHelp) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { with(h, 0, |h| h.page.back() as c_int) }
}

/// 1 while a topic is read, 0 on the list.
///
/// # Safety
/// `h` is live.
#[no_mangle]
pub unsafe extern "C" fn nib_help_reading(h: *mut NibHelp) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { with(h, 0, |h| h.page.reading().is_some() as c_int) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn s(p: *const c_char) -> String {
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn a_topic_read_and_dragged_through_the_c_calls() {
        let d = std::env::temp_dir().join(format!("nibkit-menu-c-help-{}", std::process::id()));
        std::fs::create_dir_all(d.join("help")).unwrap();
        let long: String = (0..30).map(|i| format!("Line {i}.\n\n")).collect();
        std::fs::write(d.join("whats-new.txt"), format!("nibOS 9.9\n{long}")).unwrap();
        std::fs::write(d.join("help/10-controls.txt"), "Controls\nA plays.").unwrap();
        // SAFETY: the only test that sets it.
        unsafe { std::env::set_var("NIBOS_GUIDE", &d) };
        let h = nib_help_new();
        unsafe { std::env::remove_var("NIBOS_GUIDE") };
        assert!(!h.is_null());
        let m = nib_menu_new();
        let label = std::ffi::CString::new("System").unwrap();
        unsafe {
            let fill = || {
                nib_menu_begin(m, nib_help_title(h), nib_help_hint(h));
                nib_menu_tab(m, label.as_ptr(), 1);
                nib_help_fill(h, m)
            };
            assert_eq!(fill(), 0);
            assert_eq!(s(nib_help_title(h)), "Help");
            assert_eq!((*m).menu.rows.len(), 2);
            assert_eq!((*m).menu.tabs.len(), 1);
            let e = nib_menu_key(m, 4);
            assert_eq!((e.kind, e.index), (2, 0), "A on What's new");
            assert_eq!(nib_help_choose(h, e.index), 1);
            fill();
            assert_eq!(nib_help_reading(h), 1);
            assert_eq!(s(nib_help_title(h)), "nibOS 9.9");
            assert!((*m).menu.reading);
            nib_menu_key(m, 1);
            assert_eq!((*m).menu.scroll, 1, "down scrolls");
            // A finger drags it three lines on, then lifts.
            let step = nibkit::menu::layout::ROW_STEP;
            assert_eq!(nib_menu_drag(m, 0, 400.0), 0);
            assert_eq!(nib_menu_drag(m, 1, 400.0 - step * 3.0), 1);
            assert_eq!(nib_menu_drag(m, 2, 0.0), 0);
            assert_eq!((*m).menu.scroll, 4);
            assert_eq!(nib_menu_drag(m, 1, 0.0), 0, "no finger down: nothing");
            fill();
            assert_eq!((*m).menu.scroll, 4, "kept from one filling to the next");
            let e = nib_menu_key(m, 5);
            assert_eq!(e.kind, 5, "B");
            assert_eq!(nib_help_back(h), 1);
            fill();
            assert!(!(*m).menu.reading);
            assert_eq!(nib_help_back(h), 0, "on the list B is the app's");
            // Another page filled after it is no page to read.
            nib_menu_begin(m, std::ptr::null(), std::ptr::null());
            assert!(!(*m).menu.reading);
            nib_help_free(h);
            nib_menu_free(m);
        }
        std::fs::remove_dir_all(&d).unwrap();
    }
}
