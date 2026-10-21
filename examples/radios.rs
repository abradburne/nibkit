//! The radios as nibkit's `net` sees them (feature `net-control`), without changing anything:
//! the Wi-Fi network joined and the ones in range, the known ones, and the Bluetooth devices.
//! `radios scan` asks for a Wi-Fi scan first. `radios bt-cycle` turns Bluetooth off and on four
//! times, then off straight after BlueZ restarts (needs sudo for the restart), and on again:
//! each step says whether it worked.
//!
//! ```sh
//! cargo run --example radios --features net-control -- scan
//! ```

use nibkit::net::{bluetooth::Bluetooth, wifi::Wifi};

fn bt_cycle() {
    let bt = match Bluetooth::open() {
        Ok(bt) => bt,
        Err(e) => return println!("bluetooth: {e}"),
    };
    for on in [false, true, false, true, false, true, false, true] {
        let r = bt.set_powered(on);
        println!("powered {on}: {r:?} -> now {:?}", bt.powered());
    }
    let _ = std::process::Command::new("sudo")
        .args(["systemctl", "restart", "bluetooth"])
        .status();
    let r = bt.set_powered(false);
    println!("off right after BlueZ restarted: {r:?}");
    std::thread::sleep(std::time::Duration::from_secs(2));
    let r = bt.set_powered(true);
    println!("on again: {r:?} -> now {:?}", bt.powered());
}

fn main() {
    if std::env::args().any(|a| a == "bt-cycle") {
        return bt_cycle();
    }
    match Wifi::open() {
        Ok(wifi) => {
            if std::env::args().any(|a| a == "scan") {
                let _ = wifi.scan();
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            println!("wifi: {:?}", wifi.status());
            for n in wifi.networks().unwrap_or_default() {
                println!(
                    "  {:32} {:?} {} bars ({} dBm){}{}",
                    n.name,
                    n.security,
                    n.bars,
                    n.dbm,
                    if n.known { " known" } else { "" },
                    if n.connected { " connected" } else { "" }
                );
            }
            for k in wifi.known().unwrap_or_default() {
                println!(
                    "  known: {} {:?} auto={}",
                    k.name, k.security, k.autoconnect
                );
            }
        }
        Err(e) => println!("wifi: {e}"),
    }
    match Bluetooth::open() {
        Ok(bt) => {
            println!("bluetooth: powered {:?}", bt.powered());
            for d in bt.devices().unwrap_or_default() {
                println!(
                    "  {:24} {} {:?} paired={} connected={} battery={:?}",
                    d.name, d.address, d.kind, d.paired, d.connected, d.battery
                );
            }
        }
        Err(e) => println!("bluetooth: {e}"),
    }
}
