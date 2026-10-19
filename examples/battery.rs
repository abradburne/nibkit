//! Prints the battery's charge, what it is doing, the time to full and battery protect, every
//! 10 s, and at once when battery protect changes (`nib battery protect on|off`). Runs on
//! nibOS without the screens.
//!
//! ```text
//! cargo run --example battery
//! ```

use nibkit::battery;
use std::time::{Duration, Instant};

fn main() {
    let Some(mut changes) = battery::watch() else {
        eprintln!("this system keeps the battery to itself");
        return;
    };
    let show = || {
        let Some(b) = battery::read() else { return };
        let to_full = b.time_to_full().map_or(String::new(), |t| {
            format!(", full in ~{} min", t.as_secs() / 60)
        });
        let protect = match battery::protect() {
            Some(true) => "on",
            _ => "off",
        };
        println!(
            "{}% {:?}{}, {} mA, plugged in: {}, protect {protect}",
            b.percent,
            b.state,
            to_full,
            b.current_ua / 1000,
            b.plugged_in
        );
    };
    show();
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(200));
        if changes.changed() || last.elapsed() >= Duration::from_secs(10) {
            show();
            last = Instant::now();
        }
    }
}
