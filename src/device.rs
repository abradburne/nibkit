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

/// Whether the network (Wi-Fi and Bluetooth) is up.
pub fn wifi() -> Option<bool> {
    let nib = platform::NETWORK?;
    let out = Command::new(nib).args(["net", "status"]).output().ok()?;
    let first = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .to_owned();
    Some(first == "up")
}

/// Brings the network up or down. Starting it takes a moment, so this returns at once and
/// does it in the background.
pub fn set_wifi(on: bool) {
    if let Some(nib) = platform::NETWORK {
        let way = if on { "up" } else { "down" };
        std::thread::spawn(move || {
            run(Command::new(nib).args(["net", way]));
            // Status bars show it now, not at their next read.
            crate::net::refresh();
        });
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
