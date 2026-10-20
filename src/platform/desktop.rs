//! A desktop computer (macOS, Linux, Windows), where an app made for the handheld is tried out
//! or played: both screens in windows, the keyboard and any gamepad, the mouse for touch. The
//! handheld's own controls (each screen's brightness, the battery, the speakers' tuning, the
//! network, powering off) are none of an app's business here, so their calls say `None` or do
//! nothing; powering off or restarting never touches the computer.

use crate::input::Face;
use crate::screens::Layout;
use std::path::PathBuf;

pub const NAME: &str = "desktop";
/// As apps name it in `runs_on`.
#[cfg(feature = "dslot")]
pub const KEY: &str = "desktop";

pub fn on_device() -> bool {
    false
}

/// Not a panel in sight: [`crate::screens::place`] finds none, and an app opens windows of
/// its own ([`crate::screens::desktop_windows`]).
pub const LAYOUT: Layout = Layout::PerPanel;
/// A typical monitor; an app pacing itself to the screen leaves it to its renderer's vsync.
pub const REFRESH_HZ: f64 = 60.0;
/// No panels' CRTCs to wait on ([`crate::vblank`] says so).
pub const CRTC_TOP: u32 = 0;
pub const CRTC_BOTTOM: u32 = 0;

/// Any gamepad SDL knows has its face buttons named by position on an Xbox pad; taken as the
/// DS+'s by the same place, as on nibOS (the right-hand one is A), so a game asks for the same
/// button wherever it runs.
pub fn printed(sdl: Face) -> Face {
    match sdl {
        Face::A => Face::B,
        Face::B => Face::A,
        other => other,
    }
}

pub const GAMEPAD_NAME: Option<&str> = None;
pub const CONTROLLER_MAPPINGS: &[&str] = &[];
pub const MENU_ON_RELEASE: bool = false;

/// CoreAudio, PipeWire and WASAPI all take 48 kHz and small buffers well.
pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_BUFFER: u32 = 1024;

/// The games: the folders `NIBKIT_ROMS` names (separated as `PATH` is), else `Roms` in nibOS's
/// data folder (`~/Library/Application Support/nibos/Roms` on macOS, `~/.local/share/nibos/Roms`
/// on Linux).
pub fn rom_roots() -> Vec<PathBuf> {
    match std::env::var_os("NIBKIT_ROMS") {
        Some(list) => std::env::split_paths(&list).collect(),
        None => vec![crate::paths::AppDirs::new("nibos").data.join("Roms")],
    }
}

/// Saves beside them: `Saves` in nibOS's data folder.
pub fn saves_root() -> PathBuf {
    crate::paths::AppDirs::new("nibos").data.join("Saves")
}

pub const BACKLIGHTS: Option<(&str, &str)> = None;
pub const BOARD_CONF: &str = "";
pub const BACKLIGHT_SAVE: Option<&[&str]> = None;
pub const BATTERY: Option<(&str, &str)> = None;
pub const BATTERY_PROTECT: Option<&[&str]> = None;
pub const AMP_TUNING: Option<&str> = None;
pub const VOLUME: bool = false;
pub const NETWORK: Option<&str> = None;
/// Never run: powering off and restarting are only done on the device itself.
pub const POWER_OFF: &[&str] = &["true"];
pub const RESTART: &[&str] = &["true"];
pub const SLEEP: Option<&[&str]> = None;
