#![warn(missing_docs)]
//! nibkit: what an app on a nibOS handheld (the Anbernic RG DS+ today) needs to know and do,
//! without depending on any one graphics or input library. Use it with SDL (as the template
//! in `sdk/template` does), or anything else.
//!
//! | Module | |
//! |---|---|
//! | [`screens`] | The two panels: which SDL display (or which half of one) is the top screen and which the bottom, their size and refresh. |
//! | [`vblank`] | Waiting for a panel's vertical blank, and when it was. |
//! | [`input`] | The buttons by what is printed on them, the gamepad's name, the Menu and Back keys. |
//! | [`touch`] | The touch panel, read straight from evdev. |
//! | [`backlight`] | Each screen's brightness, in percent. |
//! | [`battery`] | The battery's charge, time to full, and battery protect (stop at about 80%). |
//! | [`device`] | Volume, Wi-Fi, sleeping, restarting, powering off. |
//! | [`net`] | The radios for a status bar: Wi-Fi off, connecting or connected (and its signal), Bluetooth off, on or connected. |
//! | [`power`] | The power menu's request (a long press of power on nibOS), and what a game does when it is put away for it. |
//! | [`audio`] | The sample rate and buffer size that suit the system's sound server. |
//! | [`names`] | A game's name for showing, from its file's name (No-Intro's tags dropped). |
//! | [`paths`] | Where an app keeps its settings, data and cache; where games and saves are. |
//! | [`settings`] | Settings at three levels, system, console and game, each overriding the one before. |
//! | [`watch`] | Noticing that a setting changed elsewhere, without waiting. |
//! | `dslot` (feature `dslot`) | Description files, for an emulator or app that DSlot starts. |
//! | `menu` (feature `menu`) | The menu on the touch screen, as DSlot draws it: what goes where, the buttons and touches; with `menu-raster`, drawn into a picture. |
//!
//! The same code runs on two systems, chosen when the app is built: **nibOS** (feature
//! `nibos`, the default) and the RG DS+'s **stock OS** (feature `stock`). Where the stock OS
//! lacks something nibOS has (each screen's brightness, a volume DSlot can set), the call says
//! so (`None`) rather than guessing.

pub mod audio;
pub mod backlight;
pub mod battery;
pub mod clock;
pub mod device;
#[cfg(feature = "dslot")]
pub mod dslot;
pub mod input;
#[cfg(feature = "menu")]
pub mod menu;
pub mod names;
pub mod net;
pub mod paths;
mod platform;
pub mod power;
pub mod screens;
pub mod settings;
pub mod touch;
pub mod vblank;
pub mod watch;

#[cfg(test)]
mod testing;

/// The system nibkit was built for: "nibOS" or "stock".
pub const SYSTEM: &str = platform::NAME;

/// On the handheld itself, as against a desktop where an app is tried out: no X or Wayland
/// session on nibOS, the stock OS's vendor partition there.
pub fn on_device() -> bool {
    platform::on_device()
}
