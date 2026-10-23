//! The device's volume and network, and powering it off, where the system lets an app change
//! them. On nibOS: the volume through PipeWire's PulseAudio server (`pactl`), Wi-Fi with `nib
//! net` (starting and stopping the network needs a polkit rule for the app's user). On the stock
//! OS its own helper keeps the volume, and there is no network switch: `None`.

use crate::platform;
use std::process::Command;

/// The default output's volume, in percent.
pub fn volume() -> Option<u8> {
    if !platform::VOLUME {
        return None;
    }
    let out = Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    parse_volume(&String::from_utf8_lossy(&out.stdout))
}

/// Watches the default output's volume for changes by anyone (the volume keys, another app,
/// this one), through PipeWire's change events (`pactl subscribe`, kept running while it
/// lives). `None` where the system keeps the volume to itself.
pub fn watch_volume() -> Option<VolumeWatch> {
    if !platform::VOLUME {
        return None;
    }
    let mut child = Command::new("pactl")
        .arg("subscribe")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let out = child.stdout.take()?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: fcntl on a descriptor `out` owns.
        unsafe {
            let fd = out.as_raw_fd();
            libc::fcntl(
                fd,
                libc::F_SETFL,
                libc::fcntl(fd, libc::F_GETFL) | libc::O_NONBLOCK,
            );
        }
    }
    Some(VolumeWatch {
        child,
        out,
        line: String::new(),
    })
}

/// Changes to the default output's volume (and to which output that is) since it was last
/// asked. For a poll loop, its descriptor becomes readable on a change (and on other events,
/// which [`VolumeWatch::changed`] passes over).
pub struct VolumeWatch {
    child: std::process::Child,
    out: std::process::ChildStdout,
    /// A line read in part.
    line: String,
}

impl VolumeWatch {
    /// Whether the volume may have changed since the last call. Never waits.
    pub fn changed(&mut self) -> bool {
        use std::io::Read;
        let mut buf = [0u8; 1024];
        let mut any = false;
        while let Ok(n @ 1..) = self.out.read(&mut buf) {
            self.line.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(end) = self.line.find('\n') {
                any |= volume_event(&self.line[..end]);
                self.line.drain(..=end);
            }
        }
        any
    }
}

#[cfg(unix)]
impl std::os::fd::AsRawFd for VolumeWatch {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.out.as_raw_fd()
    }
}

impl Drop for VolumeWatch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether a line of `pactl subscribe` may be a volume change: a sink changed (its volume), or
/// the server (the default sink, as headphones go in). A stream's (`sink-input`) is not.
fn volume_event(line: &str) -> bool {
    line.starts_with("Event 'change' on ") && {
        let what = &line["Event 'change' on ".len()..];
        what.starts_with("sink #") || what.starts_with("server")
    }
}

/// Sets the default output's volume, in percent (100 at most). Does nothing where the
/// system keeps the volume to itself.
pub fn set_volume(percent: u8) {
    if platform::VOLUME {
        let level = format!("{}%", percent.min(100));
        run(Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &level]));
    }
}

/// `pactl get-sink-volume`'s first channel: "Volume: front-left: 13107 /  20% / ...".
fn parse_volume(text: &str) -> Option<u8> {
    let percent = text.split_whitespace().find(|w| w.ends_with('%'))?;
    percent.trim_end_matches('%').parse().ok()
}

/// The first line `nib <words>` prints, if it ran and succeeded.
fn nib(words: &[&str]) -> Option<String> {
    let out = Command::new(platform::NETWORK?).args(words).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next()?.trim().to_owned())
}

/// Whether Wi-Fi is on: nibOS's switch (`nib wifi`, which blocks the Wi-Fi radio alone and
/// leaves Bluetooth be), or, on a nibOS older than it, whether the network is up.
pub fn wifi() -> Option<bool> {
    match nib(&["wifi", "status"]) {
        Some(status) => Some(status == "on"),
        None => Some(nib(&["net", "status"])? == "up"),
    }
}

/// Turns Wi-Fi on or off with nibOS's switch (`nib wifi`: off blocks the Wi-Fi radio alone,
/// Bluetooth carries on, and the choice is kept across restarts; on brings the network up
/// first where it isn't). It takes a moment, so this returns at once and does it in the
/// background.
pub fn set_wifi(on: bool) {
    if let Some(nib) = platform::NETWORK {
        let way = if on { "on" } else { "off" };
        std::thread::spawn(move || {
            run(Command::new(nib).args(["wifi", way]));
            // Status bars show it now, not at their next read.
            crate::net::refresh();
        });
    }
}

/// Whether Bluetooth's switch is on: nibOS's (`nib bluetooth status`, rfkill, kept across
/// restarts). `None` on a nibOS older than it, where the switch is BlueZ's `Powered` alone.
pub fn bluetooth() -> Option<bool> {
    Some(nib(&["bluetooth", "status"])? == "on")
}

/// Turns Bluetooth on or off with nibOS's switch (`nib bluetooth`: on brings the network up
/// where it isn't, then unblocks the radio; off blocks it; the choice is kept across restarts).
/// Blocks until done. `false` where there is no such switch (an older nibOS): then BlueZ's
/// `Powered` is all there is.
pub fn set_bluetooth(on: bool) -> bool {
    let way = if on { "on" } else { "off" };
    let done = platform::NETWORK.is_some_and(|nib| run(Command::new(nib).args(["bluetooth", way])));
    crate::net::refresh();
    done
}

/// Brings nibOS's network up (both radios' daemons, iwd and BlueZ: `nib net up`), where its
/// mode left it down, and returns when it has. Wi-Fi stays off if it was: its switch is set
/// off first, which `nib net up` keeps.
pub fn network_up() {
    if let Some(nib) = platform::NETWORK {
        if wifi() == Some(false) {
            run(Command::new(nib).args(["wifi", "off"]));
        }
        run(Command::new(nib).args(["net", "up"]));
        crate::net::refresh();
    }
}

/// Whether [`sleep`] can work here.
pub fn can_sleep() -> bool {
    platform::SLEEP.is_some()
}

/// Sleeps (suspends to RAM), returning once the device has woken up. `false` where this
/// system doesn't let an app do it.
pub fn sleep() -> bool {
    match platform::SLEEP {
        Some(command) if crate::on_device() => run(Command::new(command[0]).args(&command[1..])),
        _ => false,
    }
}

/// Restarts the device. Only on the device itself.
pub fn restart() {
    if crate::on_device() {
        let _ = Command::new("sync").status();
        let command = platform::RESTART;
        run(Command::new(command[0]).args(&command[1..]));
    }
}

/// Powers the device off, as the system does it. Only on the device itself.
pub fn power_off() {
    if crate::on_device() {
        let _ = Command::new("sync").status();
        let command = platform::POWER_OFF;
        run(Command::new(command[0]).args(&command[1..]));
    }
}

/// Runs a command, logging its failure. Returns whether it worked.
fn run(command: &mut Command) -> bool {
    match command.output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "nibkit: {command:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            false
        }
        Err(e) => {
            eprintln!("nibkit: {command:?}: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_events() {
        assert!(volume_event("Event 'change' on sink #52"));
        assert!(volume_event("Event 'change' on server #4294967295"));
        assert!(!volume_event("Event 'change' on sink-input #88"));
        assert!(!volume_event("Event 'new' on sink #52"));
        assert!(!volume_event("Event 'change' on source #51"));
    }

    #[test]
    fn pactl() {
        let out = "Volume: front-left: 13107 /  20% / -41.94 dB,   front-right: 13107 /  20% / -41.94 dB\n        balance 0.00\n";
        assert_eq!(parse_volume(out), Some(20));
        assert_eq!(parse_volume("Connection failure"), None);
    }
}
