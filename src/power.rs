//! The power menu. On nibOS a long press of the power button asks the boot UI (DSlot) to show
//! its power menu, by sending its main process [`MENU_SIGNAL`]; a game it started is asked to
//! save its auto-resume state and quit first, with SIGTERM.
//!
//! An app that shows the power menu itself (DSlot does) watches for the request:
//!
//! ```no_run
//! let requests = nibkit::power::watch();
//! loop {
//!     if requests.take() {
//!         // show Sleep / Restart / Power off / Cancel
//!     }
//!     # break;
//! }
//! ```
//!
//! **Every program DSlot starts should save its auto-resume state and exit when it gets
//! SIGTERM** (as when the user leaves it with Menu): that is how a game is put away for the
//! power menu, and systemd sends the same at shutdown. On the stock OS nothing sends the request,
//! and its power button does what it always did.

use std::sync::atomic::{AtomicBool, Ordering};

/// The signal that asks for the power menu: SIGUSR1.
pub const MENU_SIGNAL: libc::c_int = libc::SIGUSR1;

static ASKED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    // Only an atomic store: all a signal handler may safely do here.
    ASKED.store(true, Ordering::SeqCst);
}

/// Requests for the power menu, from when [`watch`] was called.
pub struct Requests(());

impl Requests {
    /// Whether the power menu has been asked for since the last call.
    pub fn take(&self) -> bool {
        ASKED.swap(false, Ordering::SeqCst)
    }
}

/// Starts listening for [`MENU_SIGNAL`]. Call it once, early; a blocking call (waiting for a
/// child, say) is not interrupted by it.
pub fn watch() -> Requests {
    // SAFETY: a handler that only stores to an atomic, installed with SA_RESTART so that
    // interrupted system calls carry on.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(MENU_SIGNAL, &action, std::ptr::null_mut());
    }
    Requests(())
}

/// Asks the boot UI's power menu to open, as nibOS does on a long press of power: for a tool,
/// or a test. `pid` is the boot UI's main process.
pub fn request_menu(pid: u32) -> std::io::Result<()> {
    // SAFETY: kill(2) with a pid and a signal number.
    if unsafe { libc::kill(pid as libc::pid_t, MENU_SIGNAL) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_seen_once() {
        let requests = watch();
        assert!(!requests.take());
        request_menu(std::process::id()).unwrap();
        // The signal is delivered to this process before kill returns (it is not blocked).
        let seen = (0..100).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(5));
            requests.take()
        });
        assert!(seen);
        assert!(!requests.take());
    }
}
