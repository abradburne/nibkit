//! Wi-Fi and Bluetooth for a menu's pages (feature `net-control`): a worker thread of its own
//! that talks to iwd and BlueZ ([`super::wifi`], [`super::bluetooth`], which block), does what
//! the pages ask one thing at a time, and keeps a [`Snapshot`] of how things are for the pages to
//! draw from, read again every couple of seconds while it runs. Pairing's questions (a PIN, a
//! passkey to confirm) wait on that thread for the person's [`Radios::reply`].
//!
//! DSlot's Wi-Fi and Bluetooth pages, and every app's Connections page
//! (`menu::connections`), are made from this.

pub use real::*;

/// What the pages show.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Wi-Fi: `None` while it is off (iwd not running), or not read yet.
    pub wifi: Option<WifiNow>,
    /// Bluetooth: `None` while BlueZ isn't there, or not read yet.
    pub bluetooth: Option<BluetoothNow>,
    /// What is being done: "Joining Home…".
    pub busy: Option<String>,
    /// What the last thing done came to: "Joined Home", or why not.
    pub said: Option<String>,
    /// A question from a device being paired, waiting for the person.
    pub asking: Option<Ask>,
}

/// The Wi-Fi as read.
#[derive(Clone, Debug, Default)]
pub struct WifiNow {
    /// The network joined, if any, and its signal in bars.
    pub joined: Option<(String, u8)>,
    /// iwd's station state: "connected", "disconnected", ...
    pub state: String,
    /// Whether it is looking for networks now.
    pub scanning: bool,
    /// The networks in range, strongest first, the joined one included.
    pub networks: Vec<Net>,
}

/// A network in range, as a page row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Net {
    /// Its name (SSID).
    pub name: String,
    /// Its signal, in bars of 4.
    pub bars: u8,
    /// Whether iwd knows it (has joined it before).
    pub known: bool,
    /// Whether it is the one joined.
    pub connected: bool,
    /// Joining asks for a passphrase (and it isn't known).
    pub locked: bool,
    /// It is secured (a passphrase, or more): known or not.
    pub secured: bool,
    /// iwd can't join it here (WEP, Enterprise).
    pub unsupported: bool,
}

/// Bluetooth as read.
#[derive(Clone, Debug, Default)]
pub struct BluetoothNow {
    /// Whether the radio is on.
    pub powered: bool,
    /// Whether it is looking for devices to pair.
    pub discovering: bool,
    /// Paired devices first, then those found while looking.
    pub devices: Vec<Gadget>,
}

/// A Bluetooth device, as a page row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gadget {
    /// Its name, or its address where it gives none.
    pub name: String,
    /// Its address.
    pub address: String,
    /// "headphones", "gamepad", ...
    pub kind: &'static str,
    /// Whether it is paired.
    pub paired: bool,
    /// Whether it is connected.
    pub connected: bool,
    /// Its battery, in percent, where it says.
    pub battery: Option<u8>,
}

/// A pairing question for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Type the PIN the device wants.
    Pin,
    /// Type the six-digit passkey the device shows.
    Passkey,
    /// Do both show this passkey? Yes or no.
    Confirm(u32),
    /// Type this on the device (nothing to answer here).
    Show(String),
    /// May it pair without a code? Yes or no.
    Allow,
}

/// The person's answer to an [`Ask`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// Yes: go ahead, or the passkeys match.
    Yes,
    /// No: refuse, or cancel.
    No,
    /// The PIN or passkey typed.
    Code(String),
}

/// What the pages ask the worker to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Do {
    /// Look for Wi-Fi networks.
    Scan,
    /// Join a network, with a passphrase where it needs one.
    Join(String, Option<String>),
    /// Leave the network joined.
    Leave,
    /// Forget a known network.
    Forget(String),
    /// Bluetooth on or off.
    Power(bool),
    /// Look for devices to pair, or stop.
    Look(bool),
    /// Pair the device with this address.
    Pair(String),
    /// Connect the paired device with this address.
    Connect(String),
    /// Disconnect the device with this address.
    Disconnect(String),
    /// Forget a device.
    Remove(String),
}

mod real {
    use super::*;
    use crate::net::bluetooth::{self, Bluetooth, Kind, Request};
    use crate::net::wifi::{Security, Wifi};
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// How often what is shown is read again while nothing is asked.
    const EVERY: Duration = Duration::from_secs(2);
    /// How long a pairing question waits for an answer.
    const ANSWER: Duration = Duration::from_secs(60);

    /// The worker, while the menu is open: dropped, it stops (and Bluetooth stops looking).
    pub struct Radios {
        todo: Sender<Do>,
        replies: Sender<Reply>,
        now: Arc<Mutex<Snapshot>>,
    }

    impl Radios {
        /// Starts the worker, where the system has iwd or BlueZ to talk to.
        pub fn start() -> Option<Radios> {
            let wifi = Wifi::open();
            let bt = Bluetooth::open();
            let unsupported = |e: &crate::net::Error| *e == crate::net::Error::Unsupported;
            if matches!(&wifi, Err(e) if unsupported(e)) && matches!(&bt, Err(e) if unsupported(e))
            {
                return None;
            }
            let (todo, asked) = channel();
            let (replies, answers) = channel();
            let now = Arc::new(Mutex::new(Snapshot::default()));
            let shared = now.clone();
            std::thread::Builder::new()
                .name("radios".into())
                .spawn(move || work(asked, Arc::new(Mutex::new(answers)), shared))
                .ok()?;
            Some(Radios { todo, replies, now })
        }

        /// How things are, as last read.
        pub fn now(&self) -> Snapshot {
            self.now.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }

        /// Asks the worker to do `what`; [`Snapshot::busy`] and then [`Snapshot::said`] tell how it went.
        pub fn ask(&self, what: Do) {
            let _ = self.todo.send(what);
        }

        /// Answers the pairing question.
        pub fn reply(&self, reply: Reply) {
            self.now.lock().unwrap_or_else(|e| e.into_inner()).asking = None;
            let _ = self.replies.send(reply);
        }
    }

    fn kind(k: Kind) -> &'static str {
        match k {
            Kind::Headphones => "headphones",
            Kind::Speaker => "speaker",
            Kind::Gamepad => "gamepad",
            Kind::Keyboard => "keyboard",
            Kind::Mouse => "mouse",
            Kind::Phone => "phone",
            Kind::Computer => "computer",
            Kind::Other => "device",
        }
    }

    fn work(asked: Receiver<Do>, answers: Arc<Mutex<Receiver<Reply>>>, now: Arc<Mutex<Snapshot>>) {
        // Kept open: Bluetooth's looking lasts as long as its connection.
        let mut bt: Option<Bluetooth> = None;
        let mut looking = false;
        let set = |f: &dyn Fn(&mut Snapshot)| f(&mut now.lock().unwrap_or_else(|e| e.into_inner()));
        loop {
            let what = match asked.recv_timeout(EVERY) {
                Ok(what) => Some(what),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if bt.is_none() {
                bt = Bluetooth::open().ok();
            }
            if let Some(what) = what {
                let doing = describe(&what);
                set(&|s| (s.busy, s.said) = (Some(doing.clone()), None));
                let said = act(&what, bt.as_ref(), &mut looking, &answers, &now);
                if said.is_err() {
                    // Maybe it went away (iwd stopped, BlueZ restarted): open it again next time.
                    bt = None;
                }
                let said = said.unwrap_or_else(|e| e);
                eprintln!("dslot: {doing} {said}");
                set(&|s| (s.busy, s.said) = (None, Some(said.clone())));
            }
            let wifi = Wifi::open().ok().and_then(|w| read_wifi(&w));
            let bluetooth = bt.as_ref().and_then(|b| read_bt(b, looking));
            set(&|s| (s.wifi, s.bluetooth) = (wifi.clone(), bluetooth.clone()));
        }
    }

    /// What doing it is called while it is done.
    fn describe(what: &Do) -> String {
        match what {
            Do::Scan => "Looking for networks\u{2026}".into(),
            Do::Join(n, _) => format!("Joining {n}\u{2026}"),
            Do::Leave => "Leaving\u{2026}".into(),
            Do::Forget(n) => format!("Forgetting {n}\u{2026}"),
            Do::Power(on) => format!(
                "Turning Bluetooth {}\u{2026}",
                if *on { "on" } else { "off" }
            ),
            Do::Look(on) => (if *on {
                "Looking for devices\u{2026}"
            } else {
                "Stopping\u{2026}"
            })
            .into(),
            Do::Pair(_) => "Pairing\u{2026}".into(),
            Do::Connect(_) => "Connecting\u{2026}".into(),
            Do::Disconnect(_) => "Disconnecting\u{2026}".into(),
            Do::Remove(_) => "Forgetting\u{2026}".into(),
        }
    }

    /// Does `what`; what it came to in words, `Err` when it failed.
    fn act(
        what: &Do,
        bt: Option<&Bluetooth>,
        looking: &mut bool,
        answers: &Arc<Mutex<Receiver<Reply>>>,
        now: &Arc<Mutex<Snapshot>>,
    ) -> Result<String, String> {
        let err = |e: crate::net::Error| e.to_string();
        let device = |address: &str| -> Result<bluetooth::Device, String> {
            let bt = bt.ok_or("Bluetooth isn't there")?;
            bt.devices()
                .map_err(err)?
                .into_iter()
                .find(|d| d.address == address)
                .ok_or_else(|| "It has gone".to_owned())
        };
        match what {
            Do::Scan => Wifi::open()
                .and_then(|w| w.scan())
                .map(|_| String::new())
                .map_err(err),
            Do::Join(name, passphrase) => {
                let wifi = Wifi::open().map_err(err)?;
                let network = wifi
                    .networks()
                    .map_err(err)?
                    .into_iter()
                    .find(|n| &n.name == name)
                    .ok_or("It is out of range now")?;
                wifi.connect(&network, passphrase.as_deref()).map_err(err)?;
                Ok(format!("Joined {name}"))
            }
            Do::Leave => Wifi::open()
                .and_then(|w| w.disconnect())
                .map(|_| "Left".into())
                .map_err(err),
            Do::Forget(name) => {
                let wifi = Wifi::open().map_err(err)?;
                let known = wifi
                    .known()
                    .map_err(err)?
                    .into_iter()
                    .find(|k| &k.name == name)
                    .ok_or("It isn't known")?;
                wifi.forget(&known).map_err(err)?;
                Ok(format!("Forgot {name}"))
            }
            Do::Power(on) => {
                let bt = bt.ok_or("Bluetooth isn't there")?;
                bt.set_powered(*on).map_err(err)?;
                Ok(String::new())
            }
            Do::Look(on) => {
                let bt = bt.ok_or("Bluetooth isn't there")?;
                bt.discover(*on).map_err(err)?;
                *looking = *on;
                Ok(String::new())
            }
            Do::Pair(address) => {
                let d = device(address)?;
                let bt = bt.ok_or("Bluetooth isn't there")?;
                let (answers, asking) = (answers.clone(), now.clone());
                let result = bt.pair(&d, move |request| {
                    let ask = match request {
                        Request::Pin => Ask::Pin,
                        Request::Passkey => Ask::Passkey,
                        Request::Confirm(k) => Ask::Confirm(k),
                        Request::ShowPin(p) => Ask::Show(p),
                        Request::ShowPasskey(k) => Ask::Show(format!("{k:06}")),
                        Request::Allow => Ask::Allow,
                    };
                    let shown_only = matches!(ask, Ask::Show(_));
                    // Anything left from an earlier question.
                    let answers = answers.lock().unwrap_or_else(|e| e.into_inner());
                    while answers.try_recv().is_ok() {}
                    asking.lock().unwrap_or_else(|e| e.into_inner()).asking = Some(ask.clone());
                    if shown_only {
                        return bluetooth::Answer::Yes;
                    }
                    match answers.recv_timeout(ANSWER) {
                        Ok(Reply::Yes) => bluetooth::Answer::Yes,
                        Ok(Reply::Code(code)) if ask == Ask::Passkey => code
                            .parse()
                            .map_or(bluetooth::Answer::No, bluetooth::Answer::Passkey),
                        Ok(Reply::Code(code)) => bluetooth::Answer::Pin(code),
                        _ => bluetooth::Answer::No,
                    }
                });
                now.lock().unwrap_or_else(|e| e.into_inner()).asking = None;
                result.map_err(err)?;
                // Paired: stop looking, and connect it.
                let _ = bt.discover(false);
                *looking = false;
                bt.connect(&d).map_err(err)?;
                Ok(format!("Paired with {}", d.name))
            }
            Do::Connect(address) => {
                let d = device(address)?;
                bt.ok_or("Bluetooth isn't there")?
                    .connect(&d)
                    .map_err(err)?;
                Ok(format!("Connected {}", d.name))
            }
            Do::Disconnect(address) => {
                let d = device(address)?;
                bt.ok_or("Bluetooth isn't there")?
                    .disconnect(&d)
                    .map_err(err)?;
                Ok("Disconnected".into())
            }
            Do::Remove(address) => {
                let d = device(address)?;
                bt.ok_or("Bluetooth isn't there")?.remove(&d).map_err(err)?;
                Ok(format!("Forgot {}", d.name))
            }
        }
    }

    fn read_wifi(wifi: &Wifi) -> Option<WifiNow> {
        let status = wifi.status().ok()?;
        let networks: Vec<Net> = wifi
            .networks()
            .unwrap_or_default()
            .into_iter()
            .map(|n| Net {
                locked: n.security.needs_passphrase() && !n.known,
                secured: n.security != Security::Open,
                unsupported: matches!(n.security, Security::Wep | Security::Enterprise) && !n.known,
                name: n.name,
                bars: n.bars,
                known: n.known,
                connected: n.connected,
            })
            .collect();
        let joined = networks
            .iter()
            .find(|n| n.connected)
            .map(|n| (n.name.clone(), n.bars))
            .or_else(|| status.network.clone().map(|n| (n, 0)));
        Some(WifiNow {
            joined,
            state: status.state,
            scanning: status.scanning,
            networks,
        })
    }

    fn read_bt(bt: &Bluetooth, looking: bool) -> Option<BluetoothNow> {
        let powered = bt.powered().ok()?;
        let devices = bt
            .devices()
            .unwrap_or_default()
            .into_iter()
            // Found while looking, but not named: no use to anyone picking from a list.
            .filter(|d| d.paired || d.name != d.address)
            .map(|d| Gadget {
                kind: kind(d.kind),
                name: d.name,
                address: d.address,
                paired: d.paired,
                connected: d.connected,
                battery: d.battery,
            })
            .collect();
        Some(BluetoothNow {
            powered,
            discovering: looking,
            devices,
        })
    }
}
