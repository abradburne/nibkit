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
//!
//! **The low battery.** nibOS warns at 10% and 5% and powers off cleanly near empty (a running
//! game gets SIGTERM, as for the power menu). The shelf shows the warning itself; an app that
//! takes the screens shows it over its game with an [`Alerts`], asked once a frame:
//!
//! ```no_run
//! let mut alerts = nibkit::battery::Alerts::new();
//! // Once a frame:
//! if let Some(alert) = alerts.take() {
//!     // e.g. toasts.push(Notice::new(Kind::Problem, alert.title()).detail(alert.detail()))
//!     println!("{}: {}", alert.title(), alert.detail());
//! }
//! ```

use crate::platform;
pub use crate::watch::Watch;
use std::{io, path::Path, process::Command, time::Duration};

/// What the battery is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Running on the battery.
    Discharging,
    /// Taking charge from the charger.
    Charging,
    /// On the charger, not charging: battery protect holds it, or it won't take more.
    NotCharging,
    /// Charged full, on the charger.
    Full,
}

/// The battery now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    /// The state of charge, 0-100.
    pub percent: u8,
    /// What it is doing.
    pub state: State,
    /// Whether the charger is plugged in.
    pub plugged_in: bool,
    /// The charge now, in µAh.
    pub charge_uah: i64,
    /// The charge when full, in µAh.
    pub full_uah: i64,
    /// The current into the battery, in µA; negative out of it.
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

/// A low-battery warning from nibOS, worse than the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alert {
    /// Under the first line (10%): the charge left, in percent.
    Warn(u8),
    /// Under the second (5%): nibOS will save the game and power off before it runs out.
    Low(u8),
    /// Empty: nibOS is powering off now; the game is about to be asked to save and quit.
    Critical,
}

impl Alert {
    /// A line of the alert file: `ok 50`, `warn 9`, `low 5`, `critical 2`. `None` for ok or
    /// anything else.
    pub fn parse(line: &str) -> Option<Alert> {
        let mut words = line.split_whitespace();
        let level = words.next()?;
        let percent = words.next().and_then(|p| p.parse::<u8>().ok()).unwrap_or(0);
        match level {
            "warn" => Some(Alert::Warn(percent)),
            "low" => Some(Alert::Low(percent)),
            "critical" => Some(Alert::Critical),
            _ => None,
        }
    }

    /// How bad it is, for comparing: 1 warn, 2 low, 3 critical.
    fn rank(self) -> u8 {
        match self {
            Alert::Warn(_) => 1,
            Alert::Low(_) => 2,
            Alert::Critical => 3,
        }
    }

    /// A toast's title.
    pub fn title(self) -> &'static str {
        match self {
            Alert::Warn(_) => "Battery low",
            Alert::Low(_) => "Battery very low",
            Alert::Critical => "Battery empty",
        }
    }

    /// The line under it.
    pub fn detail(self) -> String {
        match self {
            Alert::Warn(p) => format!("{p}% left: plug in the charger soon"),
            Alert::Low(p) => format!("{p}% left: saving and powering off soon"),
            Alert::Critical => "Saving and powering off".into(),
        }
    }
}

/// The alert now, `None` while the battery is fine or where the system says nothing.
pub fn alert() -> Option<Alert> {
    Alert::parse(&std::fs::read_to_string(platform::BATTERY_ALERT?).ok()?)
}

/// nibOS's low-battery alerts as they come, for an app to show over its game: each one once,
/// when the battery gets worse (a level down, not every percent). Cheap to ask every frame: it
/// reads the file only when inotify says it changed.
pub struct Alerts {
    watch: Option<Watch>,
    shown: Option<Alert>,
    now: Option<Alert>,
    looked: bool,
}

impl Alerts {
    /// Starts watching; the first [`Alerts::take`] says the alert already up, if any.
    pub fn new() -> Alerts {
        Alerts {
            watch: platform::BATTERY_ALERT.and_then(|f| Watch::new(&[f.to_owned()])),
            shown: None,
            now: None,
            looked: false,
        }
    }

    /// A new, worse alert since the last call, to show once.
    pub fn take(&mut self) -> Option<Alert> {
        let changed = self.watch.as_mut().is_some_and(Watch::changed);
        if self.looked && !changed {
            return None;
        }
        self.looked = true;
        self.update(alert())
    }

    /// The alert up now, as the last [`Alerts::take`] read it (every change, not only the news):
    /// for an indicator that stays on screen while the battery is low.
    pub fn now(&self) -> Option<Alert> {
        self.now
    }

    /// Takes in the alert up `now`, read some other way, and says the news as
    /// [`Alerts::take`] does: news only when worse; back to fine (the charger) forgets, so the
    /// next drain warns again.
    pub fn update(&mut self, now: Option<Alert>) -> Option<Alert> {
        self.now = now;
        match now {
            None => {
                self.shown = None;
                None
            }
            Some(a) if self.shown.is_none_or(|s| a.rank() > s.rank()) => {
                self.shown = Some(a);
                Some(a)
            }
            Some(_) => None,
        }
    }
}

impl Default for Alerts {
    fn default() -> Alerts {
        Alerts::new()
    }
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

    #[test]
    fn alert_lines() {
        assert_eq!(Alert::parse("warn 9\n"), Some(Alert::Warn(9)));
        assert_eq!(Alert::parse("low 5"), Some(Alert::Low(5)));
        assert_eq!(Alert::parse("critical 2"), Some(Alert::Critical));
        assert_eq!(Alert::parse("ok 100"), None);
        assert_eq!(Alert::parse(""), None);
        assert!(Alert::Warn(9).detail().contains("9%"));
    }

    #[test]
    fn each_alert_once_and_only_when_worse() {
        let mut alerts = Alerts {
            watch: None,
            shown: None,
            now: None,
            looked: true,
        };
        assert_eq!(alerts.update(Some(Alert::Warn(10))), Some(Alert::Warn(10)));
        assert_eq!(alerts.update(Some(Alert::Warn(9))), None);
        assert_eq!(
            alerts.now(),
            Some(Alert::Warn(9)),
            "every change, not only news"
        );
        assert_eq!(alerts.update(Some(Alert::Low(5))), Some(Alert::Low(5)));
        assert_eq!(alerts.update(Some(Alert::Warn(11))), None);
        // The charger: forgotten, and the next drain warns again.
        assert_eq!(alerts.update(None), None);
        assert_eq!(alerts.update(Some(Alert::Warn(10))), Some(Alert::Warn(10)));
    }
}
