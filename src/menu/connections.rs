//! The Connections page, ready-made for any app's menu (features `menu` and `net-control`):
//! Wi-Fi on or off, Bluetooth on or off, and the paired Bluetooth devices, each connected or
//! disconnected with one press, with its battery where it says. It looks and answers as DSlot's
//! Wi-Fi and Bluetooth pages do. Joining networks and pairing new devices are DSlot's pages':
//! this page is the quick one, for a game's pause menu.
//!
//! An app keeps a [`Connections`] while its menu is open, puts its [`Connections::rows`] on a page
//! of its own (with [`Connections::title`] and [`HINT`]), and hands it the row's
//! [`Effect::Choose`] or [`Effect::Change`]:
//!
//! ```no_run
//! use nibkit::menu::{connections::{self, Connections}, Effect, Menu};
//! let mut c = Connections::start().expect("nibOS");
//! let mut menu = Menu::default();
//! // Each frame the page shows:
//! c.refresh();
//! (menu.title, menu.rows, menu.hint) = (c.title(), c.rows(), connections::HINT.into());
//! if let Some(Effect::Choose(row) | Effect::Change { row, .. }) = menu.key(nibkit::menu::Key::A) {
//!     c.act(row);
//! }
//! ```

use super::glyphs::Glyph;
use super::model::{Row, Value};
use crate::net::radios::{Do, Gadget, Radios, Snapshot};
use std::time::{Duration, Instant};

/// The page's hint line.
pub const HINT: &str =
    "A  turn on or off, connect or disconnect   \u{b7}   L / R  page   \u{b7}   B  back";

/// How often the Wi-Fi switch is read again (it takes a command).
const WIFI_EVERY: Duration = Duration::from_secs(3);

/// What a row on the page is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Wifi,
    Bluetooth,
    Heading,
    Device(String),
}

/// The Connections page's state, while the menu is open: dropped, its worker stops.
pub struct Connections {
    radios: Radios,
    now: Snapshot,
    /// The Wi-Fi switch, and when it was read.
    wifi: (Option<bool>, Instant),
}

/// A device's value on a row, as DSlot's Bluetooth page shows it: "headphones · Connected",
/// and its battery as the battery icon where it says.
pub fn device_value(g: &Gadget) -> Value {
    let state = match (g.paired, g.connected) {
        (false, _) => "Press A to pair",
        (true, true) => "Connected",
        (true, false) => "Not connected",
    };
    Value::Status {
        text: format!("{}  \u{b7}  {state}", g.kind),
        glyphs: g.battery.map(Glyph::Battery).into_iter().collect(),
    }
}

/// A network's signal, with a lock where it needs a passphrase and a check where it is the
/// one joined, as DSlot's Wi-Fi page shows them: `text` before them.
pub fn network_value(text: impl Into<String>, bars: u8, locked: bool, joined: bool) -> Value {
    let mut glyphs = Vec::new();
    if joined {
        glyphs.push(Glyph::Check);
    }
    if locked {
        glyphs.push(Glyph::Lock);
    }
    glyphs.push(Glyph::Signal(bars));
    Value::Status {
        text: text.into(),
        glyphs,
    }
}

impl Connections {
    /// The page, where the system has Wi-Fi or Bluetooth to show (nibOS); `None` elsewhere.
    pub fn start() -> Option<Connections> {
        Some(Connections {
            radios: Radios::start()?,
            now: Snapshot::default(),
            wifi: (crate::device::wifi(), Instant::now()),
        })
    }

    /// Takes in how things are now: call it before drawing the page.
    pub fn refresh(&mut self) {
        self.now = self.radios.now();
        if self.wifi.1.elapsed() >= WIFI_EVERY {
            self.wifi = (crate::device::wifi(), Instant::now());
        }
    }

    fn lines(&self) -> Vec<Line> {
        let mut lines = Vec::new();
        if self.wifi.0.is_some() {
            lines.push(Line::Wifi);
        }
        lines.push(Line::Bluetooth);
        if let Some(bt) = self.now.bluetooth.as_ref().filter(|b| b.powered) {
            let paired: Vec<_> = bt.devices.iter().filter(|g| g.paired).collect();
            if !paired.is_empty() {
                lines.push(Line::Heading);
                lines.extend(paired.iter().map(|g| Line::Device(g.address.clone())));
            }
        }
        lines
    }

    /// The line under the tabs: what is being done, what it came to, or the page's name.
    pub fn title(&self) -> String {
        match (&self.now.busy, &self.now.said) {
            (Some(busy), _) => busy.clone(),
            (_, Some(said)) if !said.is_empty() => said.clone(),
            _ => "Wi-Fi and Bluetooth".into(),
        }
    }

    /// The page's rows.
    pub fn rows(&self) -> Vec<Row> {
        let on_off = |on: bool| Value::Text((if on { "On" } else { "Off" }).into());
        self.lines()
            .into_iter()
            .map(|line| match line {
                Line::Wifi => {
                    let joined = self.now.wifi.as_ref().and_then(|w| w.joined.clone());
                    match joined {
                        // On and joined: the network, with its signal.
                        Some((name, bars)) if self.wifi.0 == Some(true) => {
                            Row::new("Wi-Fi", network_value("On", bars, false, true))
                                .with_note(name)
                        }
                        _ => Row::new("Wi-Fi", on_off(self.wifi.0 == Some(true))),
                    }
                }
                Line::Bluetooth => Row::new(
                    "Bluetooth",
                    match &self.now.bluetooth {
                        Some(b) => on_off(b.powered),
                        None => Value::Text("\u{2013}".into()),
                    },
                ),
                Line::Heading => Row::heading("Paired devices"),
                Line::Device(address) => {
                    let g = self
                        .now
                        .bluetooth
                        .iter()
                        .flat_map(|b| &b.devices)
                        .find(|g| g.address == address);
                    match g {
                        Some(g) => Row::new(g.name.clone(), device_value(g)),
                        None => Row::new(address, Value::Text("\u{2013}".into())),
                    }
                }
            })
            .collect()
    }

    /// Acts on row `row` (A, or left or right): turns a radio on or off, connects or
    /// disconnects a device. Returns whether it did anything.
    pub fn act(&mut self, row: usize) -> bool {
        let Some(line) = self.lines().into_iter().nth(row) else {
            return false;
        };
        match line {
            Line::Wifi => {
                let on = self.wifi.0 != Some(true);
                crate::device::set_wifi(on);
                self.wifi = (Some(on), Instant::now());
            }
            Line::Bluetooth => {
                let on = self.now.bluetooth.as_ref().is_some_and(|b| b.powered);
                self.radios.ask(Do::Power(!on));
            }
            Line::Heading => return false,
            Line::Device(address) => {
                let connected = self
                    .now
                    .bluetooth
                    .iter()
                    .flat_map(|b| &b.devices)
                    .any(|g| g.address == address && g.connected);
                self.radios.ask(if connected {
                    Do::Disconnect(address)
                } else {
                    Do::Connect(address)
                });
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_as_dslot_shows_it() {
        let g = Gadget {
            name: "Buds".into(),
            address: "AA".into(),
            kind: "headphones",
            paired: true,
            connected: true,
            battery: Some(80),
        };
        assert_eq!(
            device_value(&g),
            Value::Status {
                text: "headphones  \u{b7}  Connected".into(),
                glyphs: vec![Glyph::Battery(80)]
            }
        );
        let g = Gadget {
            connected: false,
            battery: None,
            ..g
        };
        assert!(matches!(device_value(&g), Value::Status { glyphs, .. } if glyphs.is_empty()));
        assert_eq!(
            network_value("", 3, true, true),
            Value::Status {
                text: String::new(),
                glyphs: vec![Glyph::Check, Glyph::Lock, Glyph::Signal(3)]
            }
        );
    }

    #[test]
    fn nothing_off_the_handheld() {
        if !crate::platform::RADIOS {
            assert!(Connections::start().is_none());
        }
    }
}
