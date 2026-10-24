//! The Connections page for a C or C++ app's pause menu: nibkit's `menu::connections` (Wi-Fi
//! and Bluetooth on or off, the paired devices to connect or disconnect), as DSlot shows it. On
//! the stock OS there is none: `nib_connections_new` gives NULL.

use crate::{out, NibMenu};
use std::ffi::{c_char, c_int, CString};

/// The page's state while the menu is open.
pub struct NibConnections {
    #[cfg(feature = "nibos")]
    page: nibkit::menu::connections::Connections,
    scratch: CString,
}

/// The page, where the system has Wi-Fi or Bluetooth (nibOS); NULL elsewhere. Free it with
/// `nib_connections_free` when the menu closes (that stops its worker).
#[no_mangle]
pub extern "C" fn nib_connections_new() -> *mut NibConnections {
    #[cfg(feature = "nibos")]
    {
        match nibkit::menu::connections::Connections::start() {
            Some(page) => Box::into_raw(Box::new(NibConnections {
                page,
                scratch: CString::default(),
            })),
            None => std::ptr::null_mut(),
        }
    }
    #[cfg(not(feature = "nibos"))]
    {
        std::ptr::null_mut()
    }
}

/// # Safety
/// `c` came from `nib_connections_new` and isn't used again (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn nib_connections_free(c: *mut NibConnections) {
    if !c.is_null() {
        // SAFETY: the caller's promise: it is ours, from Box::into_raw.
        drop(unsafe { Box::from_raw(c) });
    }
}

/// The page's title (what is being done, what it came to, or its name), for `nib_menu_begin`.
///
/// # Safety
/// `c` is a live `NibConnections`; the result is valid until the next call on `c`.
#[no_mangle]
pub unsafe extern "C" fn nib_connections_title(c: *mut NibConnections) -> *const c_char {
    // SAFETY: the caller's promise.
    let Some(c) = (unsafe { c.as_mut() }) else {
        return std::ptr::null();
    };
    #[cfg(feature = "nibos")]
    let title = c.page.title();
    #[cfg(not(feature = "nibos"))]
    let title = String::new();
    out(&mut c.scratch, Some(title))
}

/// The page's hint line, for `nib_menu_begin`.
#[no_mangle]
pub extern "C" fn nib_connections_hint() -> *const c_char {
    static HINT: std::sync::OnceLock<CString> = std::sync::OnceLock::new();
    #[cfg(feature = "nibos")]
    let hint = nibkit::menu::connections::HINT;
    #[cfg(not(feature = "nibos"))]
    let hint = "";
    HINT.get_or_init(|| CString::new(hint).unwrap_or_default())
        .as_ptr()
}

/// Reads how things are and adds the page's rows to `m` (after `nib_menu_begin` and the tabs).
/// Returns the index of the first row it added: give `nib_connections_act` a row's index less
/// that.
///
/// # Safety
/// `c` and `m` are live.
#[no_mangle]
pub unsafe extern "C" fn nib_connections_fill(c: *mut NibConnections, m: *mut NibMenu) -> c_int {
    // SAFETY: the caller's promise.
    let (Some(c), Some(m)) = (unsafe { c.as_mut() }, unsafe { m.as_mut() }) else {
        return 0;
    };
    let first = m.menu.rows.len() as c_int;
    #[cfg(feature = "nibos")]
    {
        c.page.refresh();
        m.menu.rows.extend(c.page.rows());
        m.settle();
    }
    #[cfg(not(feature = "nibos"))]
    let _ = c;
    first
}

/// Acts on the page's row `row` (counting from its first, on A or left / right): turns a radio
/// on or off, connects or disconnects a device. 1 if it did something, else 0. Fill the page
/// again to show it.
///
/// # Safety
/// `c` is live.
#[no_mangle]
pub unsafe extern "C" fn nib_connections_act(c: *mut NibConnections, row: c_int) -> c_int {
    // SAFETY: the caller's promise.
    let Some(c) = (unsafe { c.as_mut() }) else {
        return 0;
    };
    #[cfg(feature = "nibos")]
    {
        (row >= 0 && c.page.act(row as usize)) as c_int
    }
    #[cfg(not(feature = "nibos"))]
    {
        let _ = (c, row);
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_off_the_handheld() {
        if !nibkit::on_device() {
            assert!(nib_connections_new().is_null());
        }
        // NULL is answered, not followed.
        // SAFETY: NULL is allowed.
        unsafe {
            assert_eq!(nib_connections_act(std::ptr::null_mut(), 0), 0);
            nib_connections_free(std::ptr::null_mut());
        }
        assert!(!nib_connections_hint().is_null());
    }
}
