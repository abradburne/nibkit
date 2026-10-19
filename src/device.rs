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
        std::thread::spawn(move || run(Command::new(nib).args(["net", way])));
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
    fn pactl() {
        let out = "Volume: front-left: 13107 /  20% / -41.94 dB,   front-right: 13107 /  20% / -41.94 dB\n        balance 0.00\n";
        assert_eq!(parse_volume(out), Some(20));
        assert_eq!(parse_volume("Connection failure"), None);
    }
}
