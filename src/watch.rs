//! Noticing that files changed, without waiting: inotify `IN_MODIFY` on each, which the kernel
//! raises for every write (to a sysfs value too). [`crate::backlight::watch`] and
//! [`crate::battery::watch`] give one for their files.

/// Changes to a few files since it was last asked. For a poll loop, its descriptor becomes
/// readable on a change.
pub struct Watch {
    #[cfg(target_os = "linux")]
    fd: std::os::fd::OwnedFd,
}

impl Watch {
    /// Watches the files that exist of `files`; `None` if none does.
    #[cfg(target_os = "linux")]
    pub(crate) fn new(files: &[String]) -> Option<Watch> {
        use std::os::fd::{AsRawFd, FromRawFd};
        // SAFETY: no pointers; a new descriptor, owned from here on.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        let mut watched = 0;
        for file in files {
            let path = std::ffi::CString::new(file.as_str()).ok()?;
            // SAFETY: a NUL-terminated path.
            let wd =
                unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), libc::IN_MODIFY) };
            watched += usize::from(wd >= 0);
        }
        (watched > 0).then_some(Watch { fd })
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn new(_files: &[String]) -> Option<Watch> {
        None
    }

    /// Whether anything changed since the last call (a burst of writes is one change). Never
    /// waits.
    pub fn changed(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            let mut buf = [0u8; 1024];
            let mut any = false;
            // SAFETY: reads into a buffer of the length given.
            while unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) } > 0
            {
                any = true;
            }
            any
        }
        #[cfg(not(target_os = "linux"))]
        false
    }
}

#[cfg(target_os = "linux")]
impl std::os::fd::AsRawFd for Watch {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.fd.as_raw_fd()
    }
}

// inotify: Linux only.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn sees_writes() {
        let dir = crate::testing::TempDir::new("watch");
        let (a, b) = (dir.path().join("brightness"), dir.path().join("bl_power"));
        std::fs::write(&a, "100").unwrap();
        std::fs::write(&b, "0").unwrap();
        let name = |p: &std::path::Path| p.to_str().unwrap().to_owned();
        let mut w = Watch::new(&[name(&a), name(&b), "/nonexistent".into()]).unwrap();
        assert!(!w.changed());
        std::fs::write(&a, "200").unwrap();
        assert!(w.changed());
        assert!(!w.changed(), "once per change");
        std::fs::write(&b, "4").unwrap();
        std::fs::write(&b, "0").unwrap();
        assert!(w.changed());
        assert!(!w.changed(), "a burst is one");
        assert!(Watch::new(&["/nonexistent".into()]).is_none());
    }
}
