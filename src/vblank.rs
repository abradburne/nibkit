//! A panel's vertical blank: waiting for the next one, and when it came. For an app that paces
//! itself to a panel (an emulator matching its frames to the screen's) rather than leaving it
//! to its renderer's vsync. It uses DRM's WAIT_VBLANK, which does not need to be DRM master,
//! so it works while SDL owns the screens.
//!
//! ```no_run
//! use nibkit::{screens::Screen, vblank::Vblank};
//! let vblank = Vblank::open()?;
//! let when = vblank.wait(Screen::Top)?; // CLOCK_MONOTONIC
//! # Ok::<(), std::io::Error>(())
//! ```

use crate::platform;
use crate::screens::Screen;
use std::io;
use std::time::Duration;

/// The first KMS card, opened for asking about vblanks.
pub struct Vblank {
    #[cfg(not(nibkit_desktop))]
    fd: std::os::fd::OwnedFd,
}

/// Each panel's CRTC, as an index into the card's CRTCs: DRM's "pipe".
fn pipe(screen: Screen) -> u32 {
    match screen {
        Screen::Top => platform::CRTC_TOP,
        Screen::Bottom => platform::CRTC_BOTTOM,
    }
}

#[cfg(not(nibkit_desktop))]
mod drm {
    /// union drm_wait_vblank (drm.h): the request, then the reply in the same place.
    #[repr(C)]
    #[derive(Default)]
    pub struct WaitVblank {
        pub kind: u32,
        pub sequence: u32,
        pub tval_sec: libc::c_long,
        pub tval_usec: libc::c_long,
    }

    pub const WAIT_VBLANK: libc::c_ulong =
        ((3u32 << 30) | ((std::mem::size_of::<WaitVblank>() as u32) << 16) | (0x64 << 8) | 0x3A)
            as libc::c_ulong;
    /// `sequence` counts from now.
    pub const RELATIVE: u32 = 0x1;
    /// The second CRTC, the old way; later ones in the high-CRTC bits.
    pub const SECONDARY: u32 = 0x2000_0000;
    pub const HIGH_CRTC_SHIFT: u32 = 1;
    pub const HIGH_CRTC_MASK: u32 = 0x3e;
}

impl Vblank {
    /// Opens the first KMS card.
    #[cfg(not(nibkit_desktop))]
    pub fn open() -> io::Result<Vblank> {
        use std::os::fd::FromRawFd;
        for i in 0..4 {
            let path =
                std::ffi::CString::new(format!("/dev/dri/card{i}")).map_err(io::Error::other)?;
            // SAFETY: a NUL-terminated path.
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
            if fd >= 0 {
                // SAFETY: a descriptor just opened, owned from here on.
                return Ok(Vblank {
                    fd: unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) },
                });
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "no KMS card"))
    }

    /// Fails: vblanks are only on Linux.
    #[cfg(nibkit_desktop)]
    pub fn open() -> io::Result<Vblank> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "vblanks are only on Linux",
        ))
    }

    /// Waits for `screen`'s next vblank. Returns when it was, on CLOCK_MONOTONIC.
    #[cfg(not(nibkit_desktop))]
    pub fn wait(&self, screen: Screen) -> io::Result<Duration> {
        use std::os::fd::AsRawFd;
        let pipe = pipe(screen);
        let crtc = match pipe {
            0 => 0,
            1 => drm::SECONDARY,
            n => (n << drm::HIGH_CRTC_SHIFT) & drm::HIGH_CRTC_MASK,
        };
        let mut v = drm::WaitVblank {
            kind: drm::RELATIVE | crtc,
            sequence: 1,
            ..Default::default()
        };
        // SAFETY: WAIT_VBLANK reads the request from, and writes the reply to, the struct given.
        if unsafe { libc::ioctl(self.fd.as_raw_fd(), drm::WAIT_VBLANK as _, &mut v) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Duration::new(v.tval_sec as u64, v.tval_usec as u32 * 1000))
    }

    /// Fails: vblanks are only on Linux.
    #[cfg(nibkit_desktop)]
    pub fn wait(&self, screen: Screen) -> io::Result<Duration> {
        let _ = pipe(screen);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "vblanks are only on Linux",
        ))
    }
}
