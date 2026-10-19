//! Each screen's brightness, as a percentage of what the screen can do above a floor below
//! which it could be left too dark to see: 0% is the floor, never darker. On nibOS the screens'
//! backlights are `/run/nibos/backlight/top` and `bottom` (nibos-hw), writable by the user, and
//! nibOS restores their levels at boot. The stock OS keeps brightness to itself: `None` there.
//!
//! ```no_run
//! use nibkit::{backlight, screens::Screen};
//! if let Some(now) = backlight::get(Screen::Top) {
//!     backlight::set(Screen::Top, (now + 10).min(100)); // seen at once
//!     backlight::save();                                 // when the user lets go
//! }
//! backlight::set_off(Screen::Bottom, true);  // the screen dark, its level kept
//! backlight::set_off(Screen::Bottom, false); // and lit again, before the app exits
//! ```
//!
//! The kernel's level is the one source of truth: the Menu + volume shortcut, `nib brightness`
//! and every app write it, and nibOS saves it. An app that shows brightness notices changes
//! made elsewhere with [`watch`], and reads the levels again:
//!
//! ```no_run
//! # use nibkit::{backlight, screens::Screen};
//! let mut changes = backlight::watch();
//! loop {
//!     // Once a frame: one read, which doesn't wait.
//!     if changes.as_mut().is_some_and(|w| w.changed()) {
//!         let (_top, _bottom) = (backlight::get(Screen::Top), backlight::get(Screen::Bottom));
//!     }
//!     # break;
//! }
//! ```

use crate::platform;
use crate::screens::Screen;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The screen's brightness now, in percent; `None` where this system doesn't let apps have it.
pub fn get(screen: Screen) -> Option<u8> {
    Backlight::open(screen)?.percent()
}

/// Sets the screen's brightness, in percent: at once, as a slider moves.
pub fn set(screen: Screen, percent: u8) {
    if let Some(backlight) = Backlight::open(screen) {
        backlight.set_percent(percent);
    }
}

/// Turns the screen's backlight off (or on again) without changing its level, so nothing dark
/// is saved: `bl_power` 4 or 0. Any brightness nibOS sets lights it again; an app that turns
/// a screen off turns it on before it exits (on SIGTERM too). Does nothing on the stock OS.
pub fn set_off(screen: Screen, off: bool) {
    if let Some(backlight) = Backlight::open(screen) {
        backlight.set_power(off);
    }
}

/// Watches both screens' brightness and backlight power for changes by anyone (the shortcut,
/// `nib`, another app, this one); `None` where this system doesn't let apps have brightness.
pub fn watch() -> Option<Watch> {
    let (top, bottom) = platform::BACKLIGHTS?;
    Watch::new(&[top, bottom])
}

/// Brightness changes, seen as inotify `IN_MODIFY` on each screen's `brightness` and
/// `bl_power` (the kernel raises it for every write to them). For a poll loop, its descriptor
/// becomes readable on a change.
pub struct Watch {
    #[cfg(target_os = "linux")]
    fd: std::os::fd::OwnedFd,
}

impl Watch {
    #[cfg(target_os = "linux")]
    fn new(dirs: &[&str]) -> Option<Watch> {
        use std::os::fd::{AsRawFd, FromRawFd};
        // SAFETY: no pointers; a new descriptor, owned from here on.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }
        let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        let mut watched = 0;
        for dir in dirs {
            for file in ["brightness", "bl_power"] {
                let path = std::ffi::CString::new(format!("{dir}/{file}")).ok()?;
                // SAFETY: a NUL-terminated path.
                let wd = unsafe {
                    libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), libc::IN_MODIFY)
                };
                watched += usize::from(wd >= 0);
            }
        }
        (watched > 0).then_some(Watch { fd })
    }

    #[cfg(not(target_os = "linux"))]
    fn new(_dirs: &[&str]) -> Option<Watch> {
        None
    }

    /// Whether anything changed since the last call. Never waits.
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

/// Keeps the levels as they are now across an unclean power-off (nibOS saves them at shutdown
/// anyway). Call it when the user lets go of a slider, not on every step. Runs in the
/// background.
pub fn save() {
    if let Some(command) = platform::BACKLIGHT_SAVE {
        std::thread::spawn(move || {
            let run = Command::new(command[0]).args(&command[1..]).status();
            if !run.is_ok_and(|s| s.success()) {
                eprintln!("nibkit: {} failed", command.join(" "));
            }
        });
    }
}

/// A screen's backlight device.
struct Backlight {
    dir: PathBuf,
    max: i64,
    floor: i64,
    /// The hardware's own steps already look even ("non-linear" in sysfs); otherwise the
    /// percentage is squared, so equal steps look equal.
    perceptual: bool,
}

impl Backlight {
    fn open(screen: Screen) -> Option<Backlight> {
        let (top, bottom) = platform::BACKLIGHTS?;
        let dir = PathBuf::from(match screen {
            Screen::Top => top,
            Screen::Bottom => bottom,
        });
        let read = |f: &str| {
            std::fs::read_to_string(dir.join(f))
                .ok()
                .map(|s| s.trim().to_owned())
        };
        let max: i64 = read("max_brightness")?.parse().ok()?;
        let perceptual = read("scale").as_deref() == Some("non-linear");
        let floor = floor(max, min_percent(Path::new(platform::BOARD_CONF)));
        Some(Backlight {
            dir,
            max,
            floor,
            perceptual,
        })
    }

    fn percent(&self) -> Option<u8> {
        let level = std::fs::read_to_string(self.dir.join("brightness"))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        Some(to_percent(level, self.floor, self.max, self.perceptual))
    }

    fn set_percent(&self, percent: u8) {
        let level = to_level(percent, self.floor, self.max, self.perceptual);
        if let Err(e) = std::fs::write(self.dir.join("brightness"), level.to_string()) {
            eprintln!("nibkit: {}: {e}", self.dir.display());
        }
    }
}

impl Backlight {
    fn set_power(&self, off: bool) {
        if let Err(e) = std::fs::write(self.dir.join("bl_power"), bl_power(off)) {
            eprintln!("nibkit: {}: {e}", self.dir.display());
        }
    }
}

/// The kernel's `bl_power` value: FB_BLANK_POWERDOWN (4) or FB_BLANK_UNBLANK (0).
fn bl_power(off: bool) -> &'static str {
    if off {
        "4"
    } else {
        "0"
    }
}

/// The floor, in percent of the maximum: the board's NIBOS_BACKLIGHT_MIN_PERCENT, else 2.
fn min_percent(board_conf: &Path) -> i64 {
    let conf = std::fs::read_to_string(board_conf).unwrap_or_default();
    conf.lines()
        .find_map(|l| l.strip_prefix("NIBOS_BACKLIGHT_MIN_PERCENT="))
        .and_then(|v| v.trim().trim_matches('"').parse().ok())
        .unwrap_or(2)
}

fn floor(max: i64, min_percent: i64) -> i64 {
    (max * min_percent / 100).max(1)
}

fn to_level(percent: u8, floor: i64, max: i64, perceptual: bool) -> i64 {
    let p = percent.min(100) as f64 / 100.0;
    let p = if perceptual { p } else { p * p };
    (floor + ((max - floor) as f64 * p).round() as i64).clamp(floor, max)
}

fn to_percent(level: i64, floor: i64, max: i64, perceptual: bool) -> u8 {
    if max <= floor {
        return 100;
    }
    let p = (level - floor).max(0) as f64 / (max - floor) as f64;
    let p = if perceptual { p } else { p.sqrt() };
    (p * 100.0).round().min(100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve() {
        // The DS+: 1666 levels, a 2% floor, steps that already look even.
        let lowest = floor(1666, 2);
        assert_eq!(lowest, 33);
        assert_eq!(to_level(0, lowest, 1666, true), 33);
        assert_eq!(to_level(100, lowest, 1666, true), 1666);
        assert_eq!(to_level(49, lowest, 1666, true), 33 + 800);
        assert_eq!(to_level(200, lowest, 1666, true), 1666);
        // And back; below the floor is 0%.
        assert_eq!(to_percent(833, lowest, 1666, true), 49);
        assert_eq!(to_percent(10, lowest, 1666, true), 0);
        // A linear backlight: the percentage squared, and back.
        assert_eq!(to_level(50, 1, 255, false), 1 + 64);
        assert_eq!(to_percent(65, 1, 255, false), 50);
        assert_eq!(floor(10, 2), 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn watch_sees_writes() {
        let dir = crate::testing::TempDir::new("watch");
        let screen = dir.path().join("top");
        std::fs::create_dir(&screen).unwrap();
        std::fs::write(screen.join("brightness"), "100").unwrap();
        std::fs::write(screen.join("bl_power"), "0").unwrap();
        let mut w = Watch::new(&[screen.to_str().unwrap()]).unwrap();
        assert!(!w.changed());
        std::fs::write(screen.join("brightness"), "200").unwrap();
        assert!(w.changed());
        assert!(!w.changed(), "once per change");
        std::fs::write(screen.join("bl_power"), "4").unwrap();
        assert!(w.changed());
        assert!(Watch::new(&["/nonexistent"]).is_none());
    }

    #[test]
    fn power() {
        assert_eq!(bl_power(true), "4");
        assert_eq!(bl_power(false), "0");
    }

    #[test]
    fn board_floor() {
        let dir = crate::testing::TempDir::new("board");
        let conf = dir.path().join("board.conf");
        std::fs::write(
            &conf,
            "BOARD_NAME=\"RG DS+\"\nNIBOS_BACKLIGHT_MIN_PERCENT=\"3\"\n",
        )
        .unwrap();
        assert_eq!(min_percent(&conf), 3);
        assert_eq!(min_percent(&dir.path().join("none")), 2);
    }
}
