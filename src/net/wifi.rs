//! Wi-Fi, through iwd's D-Bus API (feature `net-control`): the networks in range with their
//! signal and security, joining one (with its passphrase, which iwd asks an agent of ours for),
//! the known networks and forgetting one. Turning the radio off and on is `device::set_wifi`
//! (nibOS's `nib net`, which stops and starts iwd).
//!
//! Every call blocks while iwd works (a scan takes a few seconds, joining up to half a minute):
//! make them from a thread of your own.
//!
//! ```no_run
//! use nibkit::net::wifi;
//! let wifi = wifi::Wifi::open()?;
//! wifi.scan()?;
//! for n in wifi.networks()? {
//!     println!("{} {:?} {} bars{}", n.name, n.security, n.bars, if n.connected { " *" } else { "" });
//! }
//! wifi.connect(&wifi.networks()?[0], Some("passphrase"))?;
//! # Ok::<(), nibkit::net::Error>(())
//! ```

use super::{bars, Error};
use std::collections::HashMap;
use std::sync::Mutex;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const IWD: &str = "net.connman.iwd";
const AGENT_PATH: &str = "/nibkit/wifi/agent";

/// How a network is secured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Security {
    /// No passphrase.
    Open,
    /// WPA/WPA2/WPA3 Personal: a passphrase.
    Psk,
    /// WPA Enterprise: a user name and password, set up by hand (a file of iwd's).
    Enterprise,
    /// WEP, which iwd doesn't join.
    Wep,
}

impl Security {
    fn from_iwd(t: &str) -> Security {
        match t {
            "open" => Security::Open,
            "8021x" => Security::Enterprise,
            "wep" => Security::Wep,
            _ => Security::Psk,
        }
    }

    /// Whether joining it asks for a passphrase.
    pub fn needs_passphrase(self) -> bool {
        self == Security::Psk
    }
}

/// A network in range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    /// Its name (SSID).
    pub name: String,
    /// How it is secured.
    pub security: Security,
    /// Its signal now, in bars of 4.
    pub bars: u8,
    /// Its signal in dBm.
    pub dbm: i32,
    /// Whether it has been joined before (iwd remembers it, and joins it by itself).
    pub known: bool,
    /// Whether it is the one joined now.
    pub connected: bool,
    path: OwnedObjectPath,
}

/// A network iwd remembers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    /// Its name (SSID).
    pub name: String,
    /// How it is secured.
    pub security: Security,
    /// Whether iwd joins it by itself when it is in range.
    pub autoconnect: bool,
    path: OwnedObjectPath,
}

/// What the Wi-Fi is doing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// iwd's station state: "connected", "connecting", "disconnected", "roaming", ...
    pub state: String,
    /// The network it is on, if any.
    pub network: Option<String>,
    /// Whether it is looking for networks now.
    pub scanning: bool,
    /// The interface's name (`wlan0`).
    pub interface: String,
    /// Its hardware address.
    pub address: String,
}

/// The Wi-Fi, through iwd.
pub struct Wifi {
    bus: Connection,
    /// The station's object (the Wi-Fi interface in station mode).
    station: OwnedObjectPath,
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

fn objects(bus: &Connection) -> Result<Objects, Error> {
    let om = Proxy::new(bus, IWD, "/", "org.freedesktop.DBus.ObjectManager")?;
    let found: HashMap<
        OwnedObjectPath,
        HashMap<zbus::names::OwnedInterfaceName, HashMap<String, OwnedValue>>,
    > = om.call("GetManagedObjects", &())?;
    Ok(found
        .into_iter()
        .map(|(path, ifaces)| {
            (
                path,
                ifaces
                    .into_iter()
                    .map(|(i, props)| (i.to_string(), props))
                    .collect(),
            )
        })
        .collect())
}

fn string(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| String::try_from(v.clone()).ok())
}

fn boolean(props: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    props.get(key).and_then(|v| bool::try_from(v).ok())
}

fn path(props: &HashMap<String, OwnedValue>, key: &str) -> Option<OwnedObjectPath> {
    props
        .get(key)
        .and_then(|v| OwnedObjectPath::try_from(v.clone()).ok())
}

impl Wifi {
    /// iwd's Wi-Fi station. [`Error::Unsupported`] where the system has no iwd to ask (the
    /// stock OS, a desktop); [`Error::Off`] when iwd isn't running or has no station (Wi-Fi off).
    pub fn open() -> Result<Wifi, Error> {
        if !crate::platform::RADIOS {
            return Err(Error::Unsupported);
        }
        let bus = Connection::system()?;
        let found = objects(&bus).map_err(|e| match e {
            Error::Failed(m) if m.contains("ServiceUnknown") || m.contains("not provided") => {
                Error::Off
            }
            e => e,
        })?;
        let station = found
            .into_iter()
            .find(|(_, i)| i.contains_key("net.connman.iwd.Station"))
            .map(|(p, _)| p)
            .ok_or(Error::Off)?;
        Ok(Wifi { bus, station })
    }

    fn proxy(&self, path: &OwnedObjectPath, iface: &'static str) -> Result<Proxy<'_>, Error> {
        Ok(Proxy::new(&self.bus, IWD, path.as_str().to_owned(), iface)?)
    }

    /// What the Wi-Fi is doing, and the network it is on.
    pub fn status(&self) -> Result<Status, Error> {
        let found = objects(&self.bus)?;
        let empty = HashMap::new();
        let ifaces = found.get(&self.station);
        let station = ifaces
            .and_then(|i| i.get("net.connman.iwd.Station"))
            .unwrap_or(&empty);
        let device = ifaces
            .and_then(|i| i.get("net.connman.iwd.Device"))
            .unwrap_or(&empty);
        let network = path(station, "ConnectedNetwork").and_then(|n| {
            found
                .get(&n)
                .and_then(|i| i.get("net.connman.iwd.Network"))
                .and_then(|p| string(p, "Name"))
        });
        Ok(Status {
            state: string(station, "State").unwrap_or_default(),
            network,
            scanning: boolean(station, "Scanning").unwrap_or(false),
            interface: string(device, "Name").unwrap_or_default(),
            address: string(device, "Address").unwrap_or_default(),
        })
    }

    /// Looks for networks; returns when iwd has started (it takes a few seconds, while
    /// [`Status::scanning`]). Asking while it already is does nothing.
    pub fn scan(&self) -> Result<(), Error> {
        match self
            .proxy(&self.station, "net.connman.iwd.Station")?
            .call_method("Scan", &())
        {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("Busy") => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// The networks in range, strongest first.
    pub fn networks(&self) -> Result<Vec<Network>, Error> {
        let ordered: Vec<(OwnedObjectPath, i16)> = self
            .proxy(&self.station, "net.connman.iwd.Station")?
            .call("GetOrderedNetworks", &())?;
        let found = objects(&self.bus)?;
        Ok(ordered
            .into_iter()
            .filter_map(|(p, signal)| {
                let props = found.get(&p)?.get("net.connman.iwd.Network")?;
                // iwd gives the signal in hundredths of a dBm.
                let dbm = i32::from(signal) / 100;
                Some(Network {
                    name: string(props, "Name")?,
                    security: Security::from_iwd(&string(props, "Type").unwrap_or_default()),
                    bars: bars(dbm),
                    dbm,
                    known: props.contains_key("KnownNetwork"),
                    connected: boolean(props, "Connected").unwrap_or(false),
                    path: p,
                })
            })
            .collect())
    }

    /// Joins `network`. A network that needs a passphrase and isn't known needs `passphrase`;
    /// iwd asks for it (through an agent of ours), and remembers it when joining works. Returns
    /// when it has joined, or failed: [`Error::Failed`] says why ("the passphrase is wrong",
    /// ...).
    pub fn connect(&self, network: &Network, passphrase: Option<&str>) -> Result<(), Error> {
        let agent = Agent {
            passphrase: Mutex::new(passphrase.map(str::to_owned)),
        };
        let server = self.bus.object_server();
        server.at(AGENT_PATH, agent)?;
        let manager = Proxy::new(
            &self.bus,
            IWD,
            "/net/connman/iwd",
            "net.connman.iwd.AgentManager",
        )?;
        let agent_path = OwnedObjectPath::try_from(AGENT_PATH).map_err(zbus::Error::from)?;
        let registered = manager.call_method("RegisterAgent", &(&agent_path,));
        let joined = registered.map_err(Error::from).and_then(|_| {
            self.proxy(&network.path, "net.connman.iwd.Network")?
                .call_method("Connect", &())
                .map(|_| ())
                .map_err(|e| Error::Failed(joining(&e.to_string())))
        });
        let _ = manager.call_method("UnregisterAgent", &(&agent_path,));
        let _ = server.remove::<Agent, _>(AGENT_PATH);
        joined
    }

    /// Leaves the network it is on (iwd won't join another by itself until asked, or until
    /// it starts again).
    pub fn disconnect(&self) -> Result<(), Error> {
        self.proxy(&self.station, "net.connman.iwd.Station")?
            .call_method("Disconnect", &())?;
        Ok(())
    }

    /// The networks iwd remembers.
    pub fn known(&self) -> Result<Vec<Known>, Error> {
        let found = objects(&self.bus)?;
        let mut known: Vec<Known> = found
            .into_iter()
            .filter_map(|(p, i)| {
                let props = i.get("net.connman.iwd.KnownNetwork")?;
                Some(Known {
                    name: string(props, "Name")?,
                    security: Security::from_iwd(&string(props, "Type").unwrap_or_default()),
                    autoconnect: boolean(props, "AutoConnect").unwrap_or(true),
                    path: p,
                })
            })
            .collect();
        known.sort_by_key(|k| k.name.to_lowercase());
        Ok(known)
    }

    /// Forgets a known network: its passphrase is gone, and iwd no longer joins it.
    pub fn forget(&self, known: &Known) -> Result<(), Error> {
        self.proxy(&known.path, "net.connman.iwd.KnownNetwork")?
            .call_method("Forget", &())?;
        Ok(())
    }
}

/// Why joining failed, in a person's words, from iwd's error.
fn joining(error: &str) -> String {
    if error.contains("InvalidFormat") {
        "That passphrase isn't valid for this network".into()
    } else if error.contains("Failed") || error.contains("Canceled") {
        "Couldn't join: the passphrase may be wrong".into()
    } else if error.contains("Aborted") {
        "Joining was interrupted".into()
    } else if error.contains("NotSupported") {
        "This kind of network isn't supported".into()
    } else if error.contains("Timeout") || error.contains("NoReply") {
        "The network didn't answer".into()
    } else {
        format!("Couldn't join ({error})")
    }
}

/// The agent iwd asks for a network's passphrase while joining it.
struct Agent {
    passphrase: Mutex<Option<String>>,
}

#[derive(zbus::DBusError, Debug)]
#[zbus(prefix = "net.connman.iwd.Agent.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Canceled(String),
}

#[zbus::interface(name = "net.connman.iwd.Agent")]
impl Agent {
    fn release(&self) {}

    fn request_passphrase(&self, _network: OwnedObjectPath) -> Result<String, AgentError> {
        self.passphrase
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or_else(|| AgentError::Canceled("no passphrase".into()))
    }

    fn request_private_key_passphrase(
        &self,
        _network: OwnedObjectPath,
    ) -> Result<String, AgentError> {
        Err(AgentError::Canceled("not supported".into()))
    }

    fn request_user_name_and_password(
        &self,
        _network: OwnedObjectPath,
    ) -> Result<(String, String), AgentError> {
        Err(AgentError::Canceled("not supported".into()))
    }

    fn request_user_password(
        &self,
        _network: OwnedObjectPath,
        _user: String,
    ) -> Result<String, AgentError> {
        Err(AgentError::Canceled("not supported".into()))
    }

    fn cancel(&self, _reason: String) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_and_reasons() {
        assert_eq!(Security::from_iwd("psk"), Security::Psk);
        assert_eq!(Security::from_iwd("open"), Security::Open);
        assert!(Security::Psk.needs_passphrase() && !Security::Open.needs_passphrase());
        assert!(joining("net.connman.iwd.Failed: Operation failed").contains("passphrase"));
        assert!(joining("net.connman.iwd.InvalidFormat").contains("valid"));
    }

    #[test]
    fn unsupported_off_the_handheld() {
        if !crate::platform::RADIOS {
            assert!(matches!(Wifi::open(), Err(Error::Unsupported)));
        }
    }
}
