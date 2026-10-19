//! The RG DS+'s stock OS (Buildroot, kernel 6.1, Weston, PulseAudio). Its own helper,
//! portsCtrl.dge, answers the volume and brightness keys and keeps their levels, so nibkit
//! leaves those alone there.

use crate::input::Face;
use crate::screens::Layout;
use std::path::Path;

pub const NAME: &str = "stock";

pub fn on_device() -> bool {
    Path::new("/mnt/vendor").is_dir()
}

/// Weston shows both panels as one display, side by side.
pub const LAYOUT: Layout = Layout::Spanned;
pub const REFRESH_HZ: f64 = 60.0;
/// Not measured on the stock kernel; assumed as on nibOS.
pub const CRTC_TOP: u32 = 1;
pub const CRTC_BOTTOM: u32 = 0;

/// With the stock mappings SDL names the face buttons by position on an Xbox pad: the DS+'s A
/// (on the right) is SDL's B, its X (at the top) SDL's Y, and so on round.
pub fn printed(sdl: Face) -> Face {
    match sdl {
        Face::A => Face::B,
        Face::B => Face::A,
        Face::X => Face::Y,
        Face::Y => Face::X,
    }
}

/// Not recorded here.
pub const GAMEPAD_NAME: Option<&str> = None;
/// The stock emulators' mapping databases, which name the DS+'s buttons.
pub const CONTROLLER_MAPPINGS: &[&str] = &[
    "/mnt/vendor/sbin/gamecontrollerdb.txt",
    "/mnt/vendor/deep/nnddss/assets/gamecontrollerdb.txt",
];
pub const MENU_ON_RELEASE: bool = false;

/// The stock SDL reaches PulseAudio through an ALSA shim whose small buffers underrun on every
/// period: big ones.
pub const AUDIO_RATE: u32 = 44_100;
pub const AUDIO_BUFFER: u32 = 4096;

/// The card in the TF2 slot, then the one the OS starts from.
pub const ROM_ROOTS: &[&str] = &["/mnt/sdcard/Roms", "/mnt/mmc/Roms"];
/// Beside Roms on the card the OS starts from, as on nibOS's ROMS partition.
pub const SAVES_ROOT: &str = "/mnt/mmc/Saves";

pub const BACKLIGHTS: Option<(&str, &str)> = None;
pub const BOARD_CONF: &str = "";
pub const BACKLIGHT_SAVE: Option<&[&str]> = None;
pub const VOLUME: bool = false;
pub const NETWORK: Option<&str> = None;
/// As the stock menu does it.
pub const POWER_OFF: &[&str] = &["poweroff"];
pub const RESTART: &[&str] = &["reboot"];
/// The stock OS sleeps through its own helper, on the power key.
pub const SLEEP: Option<&[&str]> = None;
