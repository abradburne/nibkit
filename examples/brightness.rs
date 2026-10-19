//! Prints each screen's brightness whenever it changes, however it was changed (the Menu +
//! volume shortcut, `nib brightness`, another app). Runs on nibOS without the screens.
//!
//! ```text
//! cargo run --example brightness
//! ```

use nibkit::{backlight, screens::Screen};
use std::time::Duration;

fn main() {
    let Some(mut changes) = backlight::watch() else {
        eprintln!("this system keeps brightness to itself");
        return;
    };
    let show = || {
        let level = |s| backlight::get(s).map_or("-".into(), |p| format!("{p}%"));
        println!(
            "top {}  bottom {}",
            level(Screen::Top),
            level(Screen::Bottom)
        );
    };
    show();
    loop {
        // A game would check once a frame; nothing here draws, so a slower poll will do.
        std::thread::sleep(Duration::from_millis(50));
        if changes.changed() {
            show();
        }
    }
}
