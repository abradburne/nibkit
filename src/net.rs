//! The radios, as a status bar shows them: Wi-Fi off, connecting, or connected (and how well),
//! and Bluetooth off, on, or connected (to how many devices, and whether one is a gamepad).
//!
//! They are read from the kernel (sysfs) and two quick commands, never hard: a [`Monitor`]
//! reads them on a thread of its own every few seconds, and a frame reads what it last found.
//! On nibOS, Wi-Fi is iwd's and Bluetooth BlueZ's; elsewhere both are [`Wifi::Unknown`] and
//! [`Bluetooth::Unknown`], for an app to leave out.
//!
//! ```no_run
//! let radios = nibkit::net::Monitor::start(std::time::Duration::from_secs(3));
//! // Each frame:
//! let now = radios.now();
//! ```

use crate::platform;
use std::path::Path;

#[cfg(feature = "net-control")]
pub mod bluetooth;
#[cfg(feature = "net-control")]
pub mod radios;
#[cfg(feature = "net-control")]
pub mod wifi;

/// Why joining, pairing or another change to a radio didn't happen (feature `net-control`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The system has no way to: the stock OS, a desktop.
    Unsupported,
    /// The radio is off (its daemon not running).
    Off,
    /// It was tried and failed, and why.
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported => f.write_str("not on this system"),
            Error::Off => f.write_str("it is off"),
            Error::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "net-control")]
impl From<zbus::Error> for Error {
    fn from(e: zbus::Error) -> Error {
        Error::Failed(e.to_string())
    }
}

#[cfg(feature = "net-control")]
impl From<zbus::fdo::Error> for Error {
    fn from(e: zbus::fdo::Error) -> Error {
        Error::Failed(e.to_string())
    }
}
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// The Wi-Fi radio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wifi {
    /// The system doesn't say, or has none.
    #[default]
    Unknown,
    /// Off: blocked, not brought up, or its daemon not running.
    Off,
    /// On, and not connected to a network (yet).
    Connecting,
    /// Connected to a network.
    Connected {
        /// Its signal, in bars of 4.
        bars: u8,
    },
}

/// The Bluetooth radio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Bluetooth {
    /// The system doesn't say, or has none.
    #[default]
    Unknown,
    /// Off: blocked, its daemon not running, or not powered.
    Off,
    /// On, with nothing connected.
    On,
    /// Connected to devices.
    Connected {
        /// How many (9 at most).
        devices: u8,
        /// Whether one of them is an input device: a gamepad, on a handheld.
        gamepad: bool,
    },
}

/// Both radios, as they were last read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Radios {
    /// The Wi-Fi radio.
    pub wifi: Wifi,
    /// The Bluetooth radio.
    pub bluetooth: Bluetooth,
}

/// The radios now. It takes a few milliseconds (it runs two small commands): call it from a
/// thread of its own, or use a [`Monitor`].
pub fn radios() -> Radios {
    if !platform::RADIOS {
        return Radios::default();
    }
    read(Path::new("/"), &|words: &[&str]| {
        let out = Command::new(words[0]).args(&words[1..]).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    })
}

/// Asks every [`Monitor`] to read the radios again now rather than at its next turn: after
/// anything that changes them (a switch, joining, connecting), so a status bar shows it at once.
pub fn refresh() {
    let (asked, wake) = &WAKE;
    *asked.lock().unwrap_or_else(|e| e.into_inner()) = true;
    wake.notify_all();
}

/// What [`refresh`] sets and the monitors wait on.
static WAKE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// The radios, read on a thread of its own every so often, and at once after [`refresh`].
pub struct Monitor {
    now: Arc<Mutex<Radios>>,
}

impl Monitor {
    /// Reads the radios now and then every `every`, for as long as the program runs. Where the
    /// system says nothing about them, no thread is started.
    pub fn start(every: Duration) -> Monitor {
        let now = Arc::new(Mutex::new(Radios::default()));
        if platform::RADIOS {
            let shared = now.clone();
            let _ = std::thread::Builder::new()
                .name("radios".into())
                .spawn(move || loop {
                    let found = radios();
                    *shared.lock().unwrap_or_else(|e| e.into_inner()) = found;
                    let (asked, wake) = &WAKE;
                    let mut now = asked.lock().unwrap_or_else(|e| e.into_inner());
                    if !*now {
                        now = wake
                            .wait_timeout(now, every)
                            .map_or_else(|e| e.into_inner().0, |r| r.0);
                    }
                    *now = false;
                });
        }
        Monitor { now }
    }

    /// What the radios were when last read.
    pub fn now(&self) -> Radios {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A command's output, if it ran and succeeded.
type Run<'a> = &'a dyn Fn(&[&str]) -> Option<String>;

/// The radios, read from the system whose root is `root`.
fn read(root: &Path, run: Run) -> Radios {
    Radios {
        wifi: wifi(root, run),
        bluetooth: bluetooth(root, run),
    }
}

/// A file's first line.
fn line(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().next().unwrap_or("").trim().to_owned())
}

/// Whether the radio of rfkill type `kind` ("wlan", "bluetooth") is blocked: `None` when the
/// system has no such radio.
fn blocked(root: &Path, kind: &str) -> Option<bool> {
    let radios = std::fs::read_dir(root.join("sys/class/rfkill")).ok()?;
    let mut found = None;
    for radio in radios.flatten().map(|e| e.path()) {
        if line(&radio.join("type")).as_deref() == Some(kind) {
            let block = |f: &str| line(&radio.join(f)).as_deref() == Some("1");
            *found.get_or_insert(false) |= block("soft") || block("hard");
        }
    }
    found
}

/// Whether a process named `name` is running.
fn running(root: &Path, name: &str) -> bool {
    let Ok(procs) = std::fs::read_dir(root.join("proc")) else {
        return false;
    };
    procs
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .any(|e| line(&e.path().join("comm")).as_deref() == Some(name))
}

/// Signal bars, of 4, for a signal of `dbm`.
pub(crate) fn bars(dbm: i32) -> u8 {
    match dbm {
        d if d >= -55 => 4,
        d if d >= -65 => 3,
        d if d >= -75 => 2,
        _ => 1,
    }
}

/// The signal in `iw dev <if> link`'s output: "signal: -49 dBm".
fn signal(link: &str) -> Option<i32> {
    let at = link.find("signal:")? + "signal:".len();
    link[at..].split_whitespace().next()?.parse().ok()
}

fn wifi(root: &Path, run: Run) -> Wifi {
    let blocked = blocked(root, "wlan");
    // The wireless interface: the one with a phy80211 (or wireless) entry.
    let net = root.join("sys/class/net");
    let iface = std::fs::read_dir(&net).ok().and_then(|ifaces| {
        ifaces
            .flatten()
            .map(|e| e.path())
            .find(|p| p.join("phy80211").exists() || p.join("wireless").exists())
    });
    let Some(iface) = iface else {
        return if blocked.is_some() {
            Wifi::Off
        } else {
            Wifi::Unknown
        };
    };
    // Blocked, or not brought up, or no daemon looking after it: off.
    let up = line(&iface.join("flags"))
        .and_then(|f| u32::from_str_radix(f.trim_start_matches("0x"), 16).ok())
        .is_some_and(|f| f & 1 != 0);
    if blocked == Some(true) || !up || !running(root, "iwd") {
        return Wifi::Off;
    }
    if line(&iface.join("operstate")).as_deref() != Some("up") {
        return Wifi::Connecting;
    }
    let name = iface
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let link = run(&["iw", "dev", &name, "link"]);
    // Connected, with no signal to be had: a middling one.
    let bars = link.as_deref().and_then(signal).map_or(2, bars);
    Wifi::Connected { bars }
}

fn bluetooth(root: &Path, run: Run) -> Bluetooth {
    let blocked = blocked(root, "bluetooth");
    let class = root.join("sys/class/bluetooth");
    if !class.join("hci0").exists() {
        return if blocked.is_some() {
            Bluetooth::Off
        } else {
            Bluetooth::Unknown
        };
    }
    if blocked == Some(true) || !running(root, "bluetoothd") {
        return Bluetooth::Off;
    }
    let powered = run(&[
        "busctl",
        "--system",
        "get-property",
        "org.bluez",
        "/org/bluez/hci0",
        "org.bluez.Adapter1",
        "Powered",
    ]);
    if powered.as_deref().map(str::trim) != Some("b true") {
        return Bluetooth::Off;
    }
    // Each connection is an "hci0:<handle>" device beside the adapter.
    let devices = std::fs::read_dir(&class)
        .map(|d| {
            d.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("hci0:"))
                .count()
        })
        .unwrap_or(0);
    if devices == 0 {
        return Bluetooth::On;
    }
    // An input device on the Bluetooth bus (5): a gamepad, most likely, on a handheld.
    let gamepad = std::fs::read_dir(root.join("sys/class/input")).is_ok_and(|inputs| {
        inputs
            .flatten()
            .any(|e| line(&e.path().join("id/bustype")).as_deref() == Some("0005"))
    });
    Bluetooth::Connected {
        devices: devices.min(9) as u8,
        gamepad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;
    use std::fs;

    /// A file of the fake system, its folders made.
    fn put(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn system(root: &Path) {
        put(root, "sys/class/rfkill/rfkill0/type", "wlan\n");
        put(root, "sys/class/rfkill/rfkill0/soft", "0\n");
        put(root, "sys/class/rfkill/rfkill0/hard", "0\n");
        put(root, "sys/class/rfkill/rfkill3/type", "bluetooth\n");
        put(root, "sys/class/rfkill/rfkill3/soft", "0\n");
        put(root, "sys/class/net/lo/flags", "0x9\n");
        put(root, "sys/class/net/wlan0/flags", "0x1003\n");
        put(root, "sys/class/net/wlan0/operstate", "up\n");
        fs::create_dir_all(root.join("sys/class/net/wlan0/phy80211")).unwrap();
        fs::create_dir_all(root.join("sys/class/bluetooth/hci0")).unwrap();
        put(root, "proc/412/comm", "iwd\n");
        put(root, "proc/433/comm", "bluetoothd\n");
        put(root, "proc/self/comm", "dslot\n");
    }

    fn commands(words: &[&str]) -> Option<String> {
        match words[0] {
            "iw" => Some("Connected to 78:8c:b5:7d:76:73 (on wlan0)\n\tsignal: -62 dBm\n".into()),
            "busctl" => Some("b true\n".into()),
            _ => None,
        }
    }

    #[test]
    fn connected_to_wifi_with_bluetooth_on() {
        let dir = TempDir::new("net-on");
        system(dir.path());
        let r = read(dir.path(), &commands);
        assert_eq!(r.wifi, Wifi::Connected { bars: 3 });
        assert_eq!(r.bluetooth, Bluetooth::On);
    }

    #[test]
    fn connecting_off_and_a_gamepad() {
        let dir = TempDir::new("net-states");
        let root = dir.path();
        system(root);
        put(root, "sys/class/net/wlan0/operstate", "dormant\n");
        assert_eq!(wifi(root, &commands), Wifi::Connecting);
        // Its daemon stopped (the network brought down): off.
        fs::remove_dir_all(root.join("proc/412")).unwrap();
        assert_eq!(wifi(root, &commands), Wifi::Off);
        // A pad connected over Bluetooth.
        fs::create_dir_all(root.join("sys/class/bluetooth/hci0:11")).unwrap();
        put(root, "sys/class/input/input7/id/bustype", "0005\n");
        assert_eq!(
            bluetooth(root, &commands),
            Bluetooth::Connected {
                devices: 1,
                gamepad: true
            }
        );
        // Blocked: off, whatever else.
        put(root, "sys/class/rfkill/rfkill3/soft", "1\n");
        assert_eq!(bluetooth(root, &commands), Bluetooth::Off);
    }

    #[test]
    fn no_radios_at_all() {
        let dir = TempDir::new("net-none");
        let r = read(dir.path(), &|_: &[&str]| None);
        assert_eq!(r, Radios::default());
    }

    #[test]
    fn signal_bars() {
        assert_eq!(signal("\tsignal: -49 dBm\n").map(bars), Some(4));
        assert_eq!(bars(-70), 2);
        assert_eq!(bars(-90), 1);
        assert_eq!(signal("Not connected."), None);
    }
}
