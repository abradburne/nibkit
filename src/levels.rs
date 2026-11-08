//! What the volume keys just changed, for an indicator over whatever is on the screens
//! ([`crate::menu::level`] draws it). nibOS's daemons change the volume (the volume keys,
//! `nibos-audio`) and both screens' brightness (Menu + volume, `nibos-input`) from anywhere, so
//! the program on the screens doesn't hear about the press; after each step the daemon
//! announces the new level in [`DIR`], in a file named for what changed (`volume` or
//! `brightness`): the percent in decimal and a newline, written aside and renamed into place,
//! so a reader never sees half of one.
//!
//! Only the keys announce. An app's own slider, a screensaver dimming the screens or `nib
//! brightness` change a level without a word, so an indicator shows only what the player has
//! just pressed for.
//!
//! An app [`watch`]es and asks [`LevelWatch::changed`] once a frame (it never waits), and shows
//! what it gives:
//!
//! ```no_run
//! let mut levels = nibkit::levels::watch();
//! // Once a frame:
//! if let Some(level) = levels.as_mut().and_then(|w| w.changed()) {
//!     println!("{:?} now {}%", level.what, level.percent);
//! }
//! ```

use crate::platform;
use std::io;
use std::path::{Path, PathBuf};

/// Where nibOS announces the levels the keys set: `/run/nibos/osd` (a tmpfs, the `video`
/// group's to write). `None` where the system makes no announcements (the stock OS, a
/// desktop).
pub const DIR: Option<&str> = platform::LEVELS;

/// What a level is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum What {
    /// The volume of the sound playing (the default output).
    Volume,
    /// Both screens' brightness.
    Brightness,
}

impl What {
    /// The file it is announced in, in [`DIR`].
    pub const fn file(self) -> &'static str {
        match self {
            What::Volume => "volume",
            What::Brightness => "brightness",
        }
    }

    #[cfg(target_os = "linux")]
    fn from_file(name: &[u8]) -> Option<What> {
        [What::Volume, What::Brightness]
            .into_iter()
            .find(|w| w.file().as_bytes() == name)
    }
}

/// A level the keys set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// What it is of.
    pub what: What,
    /// The level, 0 to 100.
    pub percent: u8,
}

/// Announces `level` in [`DIR`]: for the daemons that change it. Nothing where the system makes
/// no announcements.
pub fn announce(level: Level) -> io::Result<()> {
    match DIR {
        Some(dir) => announce_in(Path::new(dir), level),
        None => Ok(()),
    }
}

/// Announces `level` in `dir`: written to a hidden file of the writer's own, then renamed over
/// the level's file.
pub fn announce_in(dir: &Path, level: Level) -> io::Result<()> {
    let file = level.what.file();
    let aside = dir.join(format!(".{file}.{}", std::process::id()));
    std::fs::write(&aside, format!("{}\n", level.percent.min(100)))?;
    std::fs::rename(&aside, dir.join(file)).inspect_err(|_| {
        let _ = std::fs::remove_file(&aside);
    })
}

/// The level last announced in `dir` for `what`; `None` if none was, or it can't be read.
pub fn read_in(dir: &Path, what: What) -> Option<u8> {
    let text = std::fs::read_to_string(dir.join(what.file())).ok()?;
    Some(text.trim().parse::<u32>().ok()?.min(100) as u8)
}

/// Watches [`DIR`] for announcements. `None` where the system makes none, or the folder isn't
/// there (nibos-hw not installed).
pub fn watch() -> Option<LevelWatch> {
    watch_in(Path::new(DIR?))
}

/// Watches `dir` for announcements.
pub fn watch_in(dir: &Path) -> Option<LevelWatch> {
    LevelWatch::new(dir)
}

/// Announcements since it was last asked. For a poll loop, its descriptor becomes readable when
/// one is made.
pub struct LevelWatch {
    #[cfg(target_os = "linux")]
    fd: std::os::fd::OwnedFd,
    dir: PathBuf,
}

impl LevelWatch {
    #[cfg(target_os = "linux")]
    fn new(dir: &Path) -> Option<LevelWatch> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
        // SAFETY: no pointers; a new descriptor, owned from here on.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        // SAFETY: `fd` is ours and open.
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        // A rename into place, or a file written in place (`echo 40 > volume`).
        let mask = libc::IN_MOVED_TO | libc::IN_CLOSE_WRITE;
        // SAFETY: a NUL-terminated path.
        let wd = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) };
        (wd >= 0).then(|| LevelWatch {
            fd,
            dir: dir.to_owned(),
        })
    }

    #[cfg(not(target_os = "linux"))]
    fn new(_dir: &Path) -> Option<LevelWatch> {
        None
    }

    /// The newest level announced since the last call (when both were, the one announced
    /// last), read from its file; `None` if there was none. Never waits.
    pub fn changed(&mut self) -> Option<Level> {
        let what = self.last_announced()?;
        Some(Level {
            what,
            percent: read_in(&self.dir, what)?,
        })
    }

    #[cfg(target_os = "linux")]
    fn last_announced(&mut self) -> Option<What> {
        use std::os::fd::AsRawFd;
        const HEAD: usize = std::mem::size_of::<libc::inotify_event>();
        let mut buf = [0u8; 4096];
        let mut last = None;
        loop {
            // SAFETY: reads into a buffer of the length given.
            let n = unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                return last;
            }
            let mut at = 0;
            while at + HEAD <= n as usize {
                // SAFETY: the kernel wrote a whole event here; read unaligned, as the buffer is
                // bytes.
                let ev: libc::inotify_event =
                    unsafe { std::ptr::read_unaligned(buf[at..].as_ptr().cast()) };
                let name = &buf[at + HEAD..(at + HEAD + ev.len as usize).min(n as usize)];
                let name = name.split(|&b| b == 0).next().unwrap_or_default();
                last = What::from_file(name).or(last);
                at += HEAD + ev.len as usize;
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn last_announced(&mut self) -> Option<What> {
        None
    }
}

#[cfg(target_os = "linux")]
impl std::os::fd::AsRawFd for LevelWatch {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.fd.as_raw_fd()
    }
}

// inotify: Linux only.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn announcements_are_seen_newest_first() {
        let dir = crate::testing::TempDir::new("levels");
        let mut w = watch_in(dir.path()).unwrap();
        assert_eq!(w.changed(), None);
        let volume = |percent| Level {
            what: What::Volume,
            percent,
        };
        announce_in(dir.path(), volume(40)).unwrap();
        assert_eq!(w.changed(), Some(volume(40)));
        assert_eq!(w.changed(), None, "once per announcement");
        // Two in one frame: the later.
        announce_in(dir.path(), volume(45)).unwrap();
        let bright = Level {
            what: What::Brightness,
            percent: 70,
        };
        announce_in(dir.path(), bright).unwrap();
        assert_eq!(w.changed(), Some(bright));
        // Nothing left aside, and a file written in place counts too.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        std::fs::write(dir.path().join("volume"), "150\n").unwrap();
        assert_eq!(w.changed(), Some(volume(100)), "held at 100");
        // Other files are not levels.
        std::fs::write(dir.path().join("other"), "1").unwrap();
        assert_eq!(w.changed(), None);
    }

    #[test]
    fn no_folder_no_watch() {
        assert!(watch_in(Path::new("/nonexistent/osd")).is_none());
    }
}
