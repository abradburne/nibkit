//! nibOS: mainline Linux and Arch Linux ARM. Each panel is its own KMS output; PipeWire;
//! nibos-hw's gamepad, backlight links and `nib` command.

use crate::input::Face;
use crate::screens::Layout;

pub const NAME: &str = "nibOS";
/// As apps name it in `runs_on`.
#[cfg(feature = "dslot")]
pub const KEY: &str = "nibos";

pub fn on_device() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
        && !std::env::var("SDL_VIDEODRIVER").is_ok_and(|v| v == "dummy")
}

pub const LAYOUT: Layout = Layout::PerPanel;
/// Measured: 59.94 Hz on both panels, their vblanks 8.2 ms apart.
pub const REFRESH_HZ: f64 = 59.94;
/// The CRTC lighting each panel (DSI-1, the bottom, is the first).
pub const CRTC_TOP: u32 = 1;
pub const CRTC_BOTTOM: u32 = 0;

/// nibos-input names the face buttons by position (the kernel's gamepad spec): the DS+'s A, on
/// the right, is BTN_EAST, SDL's "B"; its B, at the bottom, BTN_SOUTH, SDL's "A". X and Y keep
/// their old evdev names (BTN_NORTH is BTN_X), which SDL reports as X and Y.
pub fn printed(sdl: Face) -> Face {
    match sdl {
        Face::A => Face::B,
        Face::B => Face::A,
        other => other,
    }
}

pub const GAMEPAD_NAME: Option<&str> = Some("Anbernic RG DS+ Gamepad");
/// SDL's own mapping database knows nibos-input's gamepad.
pub const CONTROLLER_MAPPINGS: &[&str] = &[];
/// nibos-input holds Menu back until it is let go of, in case it starts a shortcut.
pub const MENU_ON_RELEASE: bool = true;

/// PipeWire's rate; it takes small buffers well.
pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_BUFFER: u32 = 1024;

pub const ROM_ROOTS: &[&str] = &["/mnt/roms/Roms", "/mnt/ds/Roms"];
pub const SAVES_ROOT: &str = "/mnt/roms/Saves";

/// The backlights as nibos-hw names them: links to the kernel's backlight devices.
pub const BACKLIGHTS: Option<(&str, &str)> =
    Some(("/run/nibos/backlight/top", "/run/nibos/backlight/bottom"));
/// Where nibOS says how dark a screen may go (NIBOS_BACKLIGHT_MIN_PERCENT).
pub const BOARD_CONF: &str = "/usr/lib/nibos/board.conf";
/// Saves the screens' brightness for the next boot.
pub const BACKLIGHT_SAVE: Option<&[&str]> = Some(&["nibos-backlight", "save"]);
/// The battery and charger as nibos-hw names them: links to their power supplies.
pub const BATTERY: Option<(&str, &str)> = Some(("/run/nibos/battery", "/run/nibos/charger"));
/// Turns battery protect on or off and keeps it (`nib battery protect on|off`).
pub const BATTERY_PROTECT: Option<&[&str]> = Some(&["nib", "battery", "protect"]);
/// The volume: PipeWire's PulseAudio server, through pactl.
pub const VOLUME: bool = true;
/// `nib net up|down|status`.
pub const NETWORK: Option<&str> = Some("nib");
pub const POWER_OFF: &[&str] = &["systemctl", "poweroff"];
pub const RESTART: &[&str] = &["systemctl", "reboot"];
/// logind's suspend: returns once the device is awake again.
pub const SLEEP: Option<&[&str]> = Some(&["systemctl", "suspend"]);
