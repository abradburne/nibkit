//! Bluetooth, through BlueZ's D-Bus API (feature `net-control`): the radio on and off, the
//! devices it knows (paired, connected, their kind and battery where they say), looking for new
//! ones, pairing (with whatever a device asks: a PIN to type, a passkey to confirm or show),
//! connecting, disconnecting and forgetting.
//!
//! Calls block while BlueZ works (pairing waits for the person, connecting a few seconds): make
//! them from a thread of your own. Discovery lasts as long as the [`Bluetooth`] that started it.
//!
//! ```no_run
//! use nibkit::net::bluetooth::{Answer, Bluetooth, Request};
//! let bt = Bluetooth::open()?;
//! bt.set_powered(true)?;
//! for d in bt.devices()?.iter().filter(|d| d.paired) {
//!     println!("{} {:?} {:?}", d.name, d.kind, d.battery);
//! }
//! bt.discover(true)?;
//! // ... later, a device chosen:
//! let new = &bt.devices()?[0];
//! bt.pair(new, |request| match request {
//!     Request::Confirm(passkey) => Answer::Yes, // after showing `passkey` and asking
//!     _ => Answer::No,
//! })?;
//! bt.connect(new)?;
//! # Ok::<(), nibkit::net::Error>(())
//! ```

use super::Error;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const BLUEZ: &str = "org.bluez";
const AGENT_PATH: &str = "/nibkit/bluetooth/agent";

/// What kind of device it is, by what it says it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Headphones or a headset.
    Headphones,
    /// A speaker, or other audio.
    Speaker,
    /// A gamepad.
    Gamepad,
    /// A keyboard.
    Keyboard,
    /// A mouse or tablet.
    Mouse,
    /// A phone.
    Phone,
    /// A computer.
    Computer,
    /// Anything else, or one that doesn't say.
    Other,
}

impl Kind {
    /// From BlueZ's `Icon` (freedesktop icon names).
    fn from_icon(icon: &str) -> Kind {
        match icon {
            "audio-headphones" | "audio-headset" => Kind::Headphones,
            i if i.starts_with("audio") || i.starts_with("multimedia") => Kind::Speaker,
            "input-gaming" => Kind::Gamepad,
            "input-keyboard" => Kind::Keyboard,
            "input-mouse" | "input-tablet" => Kind::Mouse,
            "phone" => Kind::Phone,
            "computer" => Kind::Computer,
            _ => Kind::Other,
        }
    }
}

/// A device BlueZ knows: paired, or seen while looking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Its name, or its address where it gives none.
    pub name: String,
    /// Its address, `AA:BB:CC:DD:EE:FF`.
    pub address: String,
    /// What it is.
    pub kind: Kind,
    /// Whether it is paired with this device.
    pub paired: bool,
    /// Whether it may connect by itself.
    pub trusted: bool,
    /// Whether it is connected now.
    pub connected: bool,
    /// Its battery, in percent, where it says.
    pub battery: Option<u8>,
    /// Its signal while looking, in dBm.
    pub rssi: Option<i16>,
    path: OwnedObjectPath,
}

/// What a device asks while pairing, for the person to answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// A PIN to type (an old keyboard, a car kit): answer [`Answer::Pin`].
    Pin,
    /// A six-digit passkey to type, shown on the other device: answer [`Answer::Passkey`].
    Passkey,
    /// Show this PIN, to be typed on the other device; no answer needed (return
    /// [`Answer::Yes`]).
    ShowPin(String),
    /// Show this passkey, to be typed on the other device; no answer needed.
    ShowPasskey(u32),
    /// Do both devices show this passkey? [`Answer::Yes`] or [`Answer::No`].
    Confirm(u32),
    /// May it pair (or use a service) without a code? [`Answer::Yes`] or [`Answer::No`].
    Allow,
}

/// The person's answer to a [`Request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Yes: go ahead, or the passkeys match.
    Yes,
    /// No: refuse, or cancel.
    No,
    /// The PIN typed.
    Pin(String),
    /// The passkey typed.
    Passkey(u32),
}

/// Bluetooth, through BlueZ.
pub struct Bluetooth {
    bus: Connection,
}

fn string(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| String::try_from(v.clone()).ok())
}

fn boolean(props: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    props.get(key).and_then(|v| bool::try_from(v).ok())
}

impl Bluetooth {
    /// BlueZ's first adapter. [`Error::Unsupported`] where the system has no BlueZ to ask (the
    /// stock OS, a desktop); [`Error::Off`] when BlueZ isn't running or there is no adapter.
    pub fn open() -> Result<Bluetooth, Error> {
        if !crate::platform::RADIOS {
            return Err(Error::Unsupported);
        }
        let bt = Bluetooth {
            bus: Connection::system()?,
        };
        bt.adapter()?;
        Ok(bt)
    }

    /// The adapter, as BlueZ lists it now: [`Error::Off`] while there is none (BlueZ stopped,
    /// or just started and not yet found it). Looked up every time, never kept: BlueZ is
    /// stopped and started with nibOS's network (`nib net`).
    fn adapter(&self) -> Result<OwnedObjectPath, Error> {
        let found = self.objects().map_err(|_| Error::Off)?;
        let mut adapters: Vec<OwnedObjectPath> = found
            .into_iter()
            .filter(|(_, i)| i.contains_key("org.bluez.Adapter1"))
            .map(|(p, _)| p)
            .collect();
        adapters.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        adapters.into_iter().next().ok_or(Error::Off)
    }

    /// The adapter, waiting up to `wait` for BlueZ to find it.
    fn adapter_within(&self, wait: Duration) -> Result<OwnedObjectPath, Error> {
        let start = Instant::now();
        loop {
            match self.adapter() {
                Ok(a) => return Ok(a),
                Err(e) if start.elapsed() >= wait => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_millis(250)),
            }
        }
    }

    /// BlueZ's objects. Never starts BlueZ, which nibOS's network does (`nib net`).
    fn objects(&self) -> Result<super::Objects, Error> {
        super::managed_objects(&self.bus, BLUEZ)
    }

    fn proxy(&self, path: &OwnedObjectPath, iface: &'static str) -> Result<Proxy<'_>, Error> {
        Ok(Proxy::new(
            &self.bus,
            BLUEZ,
            path.as_str().to_owned(),
            iface,
        )?)
    }

    /// Whether BlueZ has an adapter now (none while nibOS's radios are off).
    pub fn present(&self) -> bool {
        self.adapter().is_ok()
    }

    /// Whether the radio is on: off while there is no adapter.
    pub fn powered(&self) -> Result<bool, Error> {
        // From BlueZ's objects (objects()), so that the reading never starts BlueZ.
        let Ok(found) = self.objects() else {
            return Ok(false);
        };
        let mut adapters: Vec<_> = found
            .iter()
            .filter_map(|(p, i)| Some((p, i.get("org.bluez.Adapter1")?)))
            .collect();
        adapters.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        Ok(adapters
            .first()
            .and_then(|(_, props)| boolean(props, "Powered"))
            .unwrap_or(false))
    }

    /// Turns the radio on or off (off disconnects everything). Off with no adapter (BlueZ
    /// stopped, or gone while it was asked) is off already; on waits a few seconds for an
    /// adapter BlueZ is still finding.
    pub fn set_powered(&self, on: bool) -> Result<(), Error> {
        let adapter = match self.adapter_within(Duration::from_secs(if on { 5 } else { 0 })) {
            Ok(a) => a,
            Err(Error::Off) if !on => return Ok(()),
            Err(e) => return Err(e),
        };
        // BlueZ just started may refuse it while it brings the adapter up: tried again for a
        // few seconds, done once the adapter says so.
        let start = Instant::now();
        loop {
            let tried = self
                .proxy(&adapter, "org.bluez.Adapter1")?
                .set_property("Powered", on);
            match tried {
                Ok(()) => return Ok(()),
                // Gone as it was turned off: off.
                Err(e) if !on && gone(&e.to_string()) => return Ok(()),
                Err(_) if self.powered().ok() == Some(on) => return Ok(()),
                Err(e) if start.elapsed() >= Duration::from_secs(3) => return Err(e.into()),
                Err(_) => std::thread::sleep(Duration::from_millis(300)),
            }
        }
    }

    /// The devices BlueZ knows: the paired ones first (connected first among them), then any
    /// found while looking, strongest first.
    pub fn devices(&self) -> Result<Vec<Device>, Error> {
        let found = self.objects()?;
        let mut devices: Vec<Device> = found
            .iter()
            .filter_map(|(p, i)| {
                let props = i.get("org.bluez.Device1")?;
                let address = string(props, "Address")?;
                let battery = i
                    .get("org.bluez.Battery1")
                    .and_then(|b| b.get("Percentage"))
                    .and_then(|v| u8::try_from(v).ok());
                Some(Device {
                    name: string(props, "Alias")
                        .or_else(|| string(props, "Name"))
                        .unwrap_or_else(|| address.clone()),
                    kind: Kind::from_icon(&string(props, "Icon").unwrap_or_default()),
                    paired: boolean(props, "Paired").unwrap_or(false),
                    trusted: boolean(props, "Trusted").unwrap_or(false),
                    connected: boolean(props, "Connected").unwrap_or(false),
                    battery,
                    rssi: props.get("RSSI").and_then(|v| i16::try_from(v).ok()),
                    address,
                    path: p.clone(),
                })
            })
            .collect();
        devices.sort_by(|a, b| {
            (
                !a.paired,
                !a.connected,
                -a.rssi.unwrap_or(-200),
                a.name.to_lowercase(),
            )
                .cmp(&(
                    !b.paired,
                    !b.connected,
                    -b.rssi.unwrap_or(-200),
                    b.name.to_lowercase(),
                ))
        });
        Ok(devices)
    }

    /// Starts or stops looking for devices to pair. Found ones join [`Bluetooth::devices`];
    /// looking stops by itself when this [`Bluetooth`] is dropped.
    pub fn discover(&self, on: bool) -> Result<(), Error> {
        let adapter = self.proxy(&self.adapter()?, "org.bluez.Adapter1")?;
        let result = adapter.call_method(
            if on {
                "StartDiscovery"
            } else {
                "StopDiscovery"
            },
            &(),
        );
        match result {
            Ok(_) => Ok(()),
            // Already doing it, or already not.
            Err(e)
                if e.to_string().contains("InProgress") || e.to_string().contains("NotReady") =>
            {
                Ok(())
            }
            Err(e) if !on && e.to_string().contains("Failed") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Pairs with `device`, asking `answer` whatever the device asks (on BlueZ's thread: it may
    /// wait for the person), then trusts it so it can connect by itself. Returns once paired,
    /// or why not.
    pub fn pair(
        &self,
        device: &Device,
        answer: impl Fn(Request) -> Answer + Send + Sync + 'static,
    ) -> Result<(), Error> {
        let server = self.bus.object_server();
        server.at(
            AGENT_PATH,
            Agent {
                answer: Box::new(answer),
            },
        )?;
        let agent = OwnedObjectPath::try_from(AGENT_PATH).map_err(zbus::Error::from)?;
        let root = OwnedObjectPath::try_from("/org/bluez").map_err(zbus::Error::from)?;
        let manager = self.proxy(&root, "org.bluez.AgentManager1")?;
        let paired = manager
            .call_method("RegisterAgent", &(&agent, "KeyboardDisplay"))
            .map_err(Error::from)
            .and_then(|_| {
                self.proxy(&device.path, "org.bluez.Device1")?
                    .call_method("Pair", &())
                    .map_err(|e| Error::Failed(pairing(&e.to_string())))
            })
            .and_then(|_| {
                self.proxy(&device.path, "org.bluez.Device1")?
                    .set_property("Trusted", true)
                    .map_err(Error::from)
            });
        let _ = manager.call_method("UnregisterAgent", &(&agent,));
        let _ = server.remove::<Agent, _>(AGENT_PATH);
        paired
    }

    /// Connects a paired device (its audio, its controls). Returns once connected.
    pub fn connect(&self, device: &Device) -> Result<(), Error> {
        self.proxy(&device.path, "org.bluez.Device1")?
            .call_method("Connect", &())
            .map_err(|e| Error::Failed(connecting(&e.to_string())))?;
        Ok(())
    }

    /// Disconnects a device (it stays paired).
    pub fn disconnect(&self, device: &Device) -> Result<(), Error> {
        self.proxy(&device.path, "org.bluez.Device1")?
            .call_method("Disconnect", &())?;
        Ok(())
    }

    /// Forgets a device: unpaired, and gone from the list.
    pub fn remove(&self, device: &Device) -> Result<(), Error> {
        self.proxy(&self.adapter()?, "org.bluez.Adapter1")?
            .call_method("RemoveDevice", &(&device.path,))?;
        Ok(())
    }
}

/// Whether a D-Bus error says the object or BlueZ itself isn't there (any more).
fn gone(error: &str) -> bool {
    [
        "UnknownObject",
        "ServiceUnknown",
        "NameHasNoOwner",
        "UnknownMethod",
    ]
    .iter()
    .any(|k| error.contains(k))
}

/// A BlueZ or D-Bus error as a person would want it said: short, no D-Bus names. The raw one
/// is for the log.
pub fn friendly(error: &str) -> String {
    if gone(error) {
        "Bluetooth isn't ready yet: try again in a moment".into()
    } else if error.contains("NotReady") {
        "Bluetooth is still starting".into()
    } else if error.contains("Blocked") || error.contains("rfkill") {
        "Bluetooth is blocked".into()
    } else if error.contains("InProgress") || error.contains("Busy") {
        "Bluetooth is busy: try again".into()
    } else if error.contains("org.freedesktop.DBus") || error.contains("org.bluez") {
        "That didn't work: try again".into()
    } else {
        error.to_owned()
    }
}

/// Why pairing failed, in a person's words, from BlueZ's error.
fn pairing(error: &str) -> String {
    if error.contains("AuthenticationFailed") || error.contains("AuthenticationRejected") {
        "The codes didn't match, or it was refused".into()
    } else if error.contains("AuthenticationCanceled") || error.contains("Canceled") {
        "Pairing was cancelled".into()
    } else if error.contains("AuthenticationTimeout") || error.contains("Timeout") {
        "It took too long: try again with the device in pairing mode".into()
    } else if error.contains("AlreadyExists") {
        "It is already paired".into()
    } else if error.contains("ConnectionAttemptFailed") || error.contains("Page Timeout") {
        "Couldn't reach it: is it in pairing mode, and near?".into()
    } else {
        eprintln!("nibkit: pairing: {error}");
        "Couldn't pair: try again with the device in pairing mode".into()
    }
}

/// Why connecting failed, in a person's words.
fn connecting(error: &str) -> String {
    if error.contains("Page Timeout") || error.contains("Host is down") {
        "Couldn't reach it: is it on, and near?".into()
    } else if error.contains("InProgress") {
        "It is already connecting".into()
    } else {
        eprintln!("nibkit: connecting: {error}");
        "Couldn't connect: is it on, and near?".into()
    }
}

/// The agent BlueZ asks while pairing, which asks the person through `answer`.
struct Agent {
    answer: Box<dyn Fn(Request) -> Answer + Send + Sync>,
}

#[derive(zbus::DBusError, Debug)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
}

fn rejected() -> AgentError {
    AgentError::Rejected("refused".into())
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {}

    fn request_pin_code(&self, _device: OwnedObjectPath) -> Result<String, AgentError> {
        match (self.answer)(Request::Pin) {
            Answer::Pin(pin) => Ok(pin),
            _ => Err(rejected()),
        }
    }

    fn display_pin_code(&self, _device: OwnedObjectPath, pincode: String) {
        (self.answer)(Request::ShowPin(pincode));
    }

    fn request_passkey(&self, _device: OwnedObjectPath) -> Result<u32, AgentError> {
        match (self.answer)(Request::Passkey) {
            Answer::Passkey(key) => Ok(key),
            _ => Err(rejected()),
        }
    }

    fn display_passkey(&self, _device: OwnedObjectPath, passkey: u32, _entered: u16) {
        (self.answer)(Request::ShowPasskey(passkey));
    }

    fn request_confirmation(
        &self,
        _device: OwnedObjectPath,
        passkey: u32,
    ) -> Result<(), AgentError> {
        match (self.answer)(Request::Confirm(passkey)) {
            Answer::Yes => Ok(()),
            _ => Err(rejected()),
        }
    }

    fn request_authorization(&self, _device: OwnedObjectPath) -> Result<(), AgentError> {
        match (self.answer)(Request::Allow) {
            Answer::Yes => Ok(()),
            _ => Err(rejected()),
        }
    }

    /// A paired device using a service (audio, input): allowed, it is ours.
    fn authorize_service(&self, _device: OwnedObjectPath, _uuid: String) -> Result<(), AgentError> {
        Ok(())
    }

    fn cancel(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_reasons() {
        assert_eq!(Kind::from_icon("audio-headphones"), Kind::Headphones);
        assert_eq!(Kind::from_icon("audio-card"), Kind::Speaker);
        assert_eq!(Kind::from_icon("input-gaming"), Kind::Gamepad);
        assert_eq!(Kind::from_icon(""), Kind::Other);
        assert!(pairing("org.bluez.Error.AuthenticationFailed").contains("codes"));
        assert!(connecting("br-connection-page-timeout Page Timeout").contains("reach"));
        // The user's report: a raw D-Bus error, never shown as it is.
        let raw = "org.freedesktop.DBus.Error.UnknownObject: Method \"Set\" with signature \"ssv\" on interface \"org.freedesktop.DBus.Properties\" doesn't exist";
        assert!(gone(raw));
        let shown = friendly(raw);
        assert!(!shown.contains("org.") && !shown.contains("ssv"), "{shown}");
        assert!(!friendly("org.bluez.Error.Failed: whatever").contains("org."));
        assert_eq!(friendly("Joined Home"), "Joined Home");
    }

    #[test]
    fn unsupported_off_the_handheld() {
        if !crate::platform::RADIOS {
            assert!(matches!(Bluetooth::open(), Err(Error::Unsupported)));
        }
    }
}
