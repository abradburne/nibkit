//! The battery: its charge, whether it is charging, an estimate of the time to full, and
//! **battery protect**, nibOS's switch that stops charging at about 80% so a device left on
//! its charger ages slower (it lowers the charge voltage; the battery stays connected). On
//! nibOS the charge is the RK817's coulomb count. The stock OS keeps all this to itself:
//! `None` there.
//!
//! ```no_run
//! use nibkit::battery;
//! if let Some(b) = battery::read() {
//!     println!("{}% {:?}", b.percent, b.state);
//!     if let Some(t) = b.time_to_full() {
//!         println!("full in about {} min", t.as_secs() / 60);
//!     }
//! }
//! if battery::protect() == Some(false) {
//!     battery::set_protect(true).ok();
//! }
//! ```

use crate::platform;
pub use crate::watch::Watch;
use std::{io, path::Path, process::Command, time::Duration};

/// What the battery is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Discharging,
    Charging,
    /// On the charger, not charging: battery protect holds it, or it won't take more.
    NotCharging,
    Full,
}

/// The battery now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// The state of charge, 0-100.
    pub percent: u8,
    pub state: State,
    /// Whether the charger is plugged in.
    pub plugged_in: bool,
    /// Charge now and when full, in µAh, and the current into the battery (negative out of it),
    /// in µA.
    pub charge_uah: i64,
    pub full_uah: i64,
    pub current_ua: i64,
}

impl Battery {
    /// While charging, the time to full at the current now (or to battery protect's stop, about
    /// [`PROTECT_PERCENT`], if on). An estimate: the current falls near the end, so the last
    /// part takes longer.
    pub fn time_to_full(&self) -> Option<Duration> {
        if self.state != State::Charging || self.current_ua <= 0 {
            return None;
        }
        let end = if protect() == Some(true) {
            PROTECT_PERCENT
        } else {
            100
        } as i64;
        let to_go = (self.full_uah * end / 100 - self.charge_uah).max(0);
        Some(Duration::from_secs((to_go * 3600 / self.current_ua) as u64))
    }
}

fn read_value(dir: &str, file: &str) -> Option<String> {
    std::fs::read_to_string(Path::new(dir).join(file))
        .ok()
        .map(|s| s.trim().to_owned())
}

fn number(dir: &str, file: &str) -> Option<i64> {
    read_value(dir, file)?.parse().ok()
}

/// The battery now; `None` where this system doesn't let apps have it.
pub fn read() -> Option<Battery> {
    let (battery, charger) = platform::BATTERY?;
    let state = match read_value(battery, "status")?.as_str() {
        "Charging" => State::Charging,
        "Not charging" => State::NotCharging,
        "Full" => State::Full,
        _ => State::Discharging,
    };
    Some(Battery {
        percent: number(battery, "capacity")?.clamp(0, 100) as u8,
        state,
        plugged_in: number(charger, "online") == Some(1),
        charge_uah: number(battery, "charge_now").unwrap_or(0),
        full_uah: number(battery, "charge_full").unwrap_or(0),
        current_ua: number(battery, "current_avg").unwrap_or(0),
    })
}

/// About where battery protect stops a charge, in percent: the cell charged to the lowered
/// voltage (4.1 V on the RG DS+) holds about 80-85%.
pub const PROTECT_PERCENT: u8 = 80;

/// Whether battery protect is on (the charge voltage set below the battery's full one);
/// `None` where there is no such switch.
pub fn protect() -> Option<bool> {
    let (battery, _) = platform::BATTERY?;
    let now = number(battery, "constant_charge_voltage")?;
    Some(now < number(battery, "constant_charge_voltage_max")?)
}

/// Turns battery protect on or off, at once and kept across restarts. The user must be in
/// the wheel group (the image's user is).
pub fn set_protect(on: bool) -> io::Result<()> {
    let command = platform::BATTERY_PROTECT.ok_or(io::ErrorKind::Unsupported)?;
    let status = Command::new(command[0])
        .args(&command[1..])
        .arg(if on { "on" } else { "off" })
        .stdout(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{} failed", command.join(" "))))
    }
}

/// Watches battery protect for changes by anyone (`nib battery`, another app). The charge
/// itself changes in the kernel, which inotify doesn't see: read it again every few seconds.
pub fn watch() -> Option<Watch> {
    let (battery, _) = platform::BATTERY?;
    Watch::new(&[format!("{battery}/constant_charge_voltage")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_to_full_at_the_current_now() {
        let b = Battery {
            percent: 50,
            state: State::Charging,
            plugged_in: true,
            charge_uah: 2_000_000,
            full_uah: 4_000_000,
            current_ua: 1_000_000,
        };
        // Off the device there is no battery protect: 2000 mAh to go at 1 A.
        assert_eq!(b.time_to_full(), Some(Duration::from_secs(2 * 3600)));
        let discharging = Battery {
            state: State::Discharging,
            current_ua: -500_000,
            ..b
        };
        assert_eq!(discharging.time_to_full(), None);
    }
}
