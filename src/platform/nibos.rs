//! nibOS: mainline Linux and Arch Linux ARM. Each panel is its own KMS output; PipeWire;
//! nibos-hw's gamepad, backlight links and `nib` command.

use crate::input::Face;
use crate::screens::Layout;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// Each card's `Roms`, then its `Games`: a card set up for slot CFW keeps its games in
/// `Games/<system>/`, and both layouts are read at once.
pub const ROM_ROOTS: &[&str] = &[
    "/mnt/roms/Roms",
    "/mnt/ds/Roms",
    "/mnt/roms/Games",
    "/mnt/ds/Games",
];
pub const SAVES_ROOT: &str = "/mnt/roms/Saves";

/// The card in TF2: its mount point, and the slot's mmc host (the DS+'s sdmmc1 is `mmc1`), a
/// `mmc1:XXXX` folder under which says the kernel has a card, before udev and the mount catch
/// up. Its fstab entry is `noauto`; a udev rule starts the mount when the card appears
/// (rootfs/dslot/70-nibos-games-card.rules), and systemd drops it when the card goes.
const GAMES_CARD: (&str, &str) = ("/mnt/ds", "/sys/class/mmc_host/mmc1");

/// `ROM_ROOTS`, less the TF2 card's when there is no card in the slot. A path under an
/// unmounted mount point is an empty folder at best, and when `/mnt/ds` was an automount each
/// touch of it waited the device timeout (5 s) out with the slot empty, again and again.
pub fn rom_roots() -> Vec<PathBuf> {
    let (mount, host) = GAMES_CARD;
    let card = mounted(Path::new(mount)) || card_in(Path::new(host));
    ROM_ROOTS
        .iter()
        .map(PathBuf::from)
        .filter(|r| card || !r.starts_with(mount))
        .collect()
}

/// Waits, up to `timeout`, for the TF2 card's mount while a card is in the slot and not mounted
/// yet (udev and systemd mount it moments after the kernel sees it, which at power-on may be
/// after a launcher has started). Returns at once with no card in, or a card already mounted.
pub fn wait_for_cards(timeout: Duration) {
    let (mount, host) = (Path::new(GAMES_CARD.0), Path::new(GAMES_CARD.1));
    let start = Instant::now();
    while start.elapsed() < timeout && card_in(host) && !mounted(mount) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether a file system is mounted at `dir`. An automount's autofs counts: its first use
/// mounts the file system.
fn mounted(dir: &Path) -> bool {
    std::fs::read_to_string("/proc/self/mounts")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split(' ').nth(1))
        .any(|m| Path::new(m) == dir)
}

/// Whether the kernel has a card on the mmc host at `host`: a `mmcN:XXXX` folder under it.
fn card_in(host: &Path) -> bool {
    let Some(name) = host.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let prefix = format!("{name}:");
    std::fs::read_dir(host)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with(&prefix))
}

/// The backlights as nibos-hw names them: links to the kernel's backlight devices.
pub const BACKLIGHTS: Option<(&str, &str)> =
    Some(("/run/nibos/backlight/top", "/run/nibos/backlight/bottom"));
/// Where nibOS says how dark a screen may go (NIBOS_BACKLIGHT_MIN_PERCENT).
pub const BOARD_CONF: &str = "/usr/lib/nibos/board.conf";
/// Folders of apps installed with the system (each app a folder, as in `Roms/APPS`).
#[cfg_attr(not(feature = "dslot"), allow(dead_code))]
pub const SYSTEM_APPS: &[&str] = &["/usr/share/dslot/apps"];
/// Saves the screens' brightness for the next boot.
pub const BACKLIGHT_SAVE: Option<&[&str]> = Some(&["nibos-backlight", "save"]);
/// The battery and charger as nibos-hw names them: links to their power supplies.
pub const BATTERY: Option<(&str, &str)> = Some(("/run/nibos/battery", "/run/nibos/charger"));
/// Turns battery protect on or off and keeps it (`nib battery protect on|off`).
pub const BATTERY_PROTECT: Option<&[&str]> = Some(&["nib", "battery", "protect"]);
/// The speaker amps' tuning: `nib audio` (sudo -n for changes; the wheel group may).
pub const AMP_TUNING: Option<&str> = Some("/usr/lib/nibos/nib-audio");
/// The volume: PipeWire's PulseAudio server, through pactl.
pub const VOLUME: bool = true;
/// `nib wifi on|off|status` (the Wi-Fi switch), `nib net up` (the network).
pub const NETWORK: Option<&str> = Some("nib");
/// Wi-Fi (iwd) and Bluetooth (BlueZ), whose state `net` reads.
pub const RADIOS: bool = true;
pub const POWER_OFF: &[&str] = &["systemctl", "poweroff"];
pub const RESTART: &[&str] = &["systemctl", "reboot"];
/// logind's suspend: returns once the device is awake again.
pub const SLEEP: Option<&[&str]> = Some(&["systemctl", "suspend"]);
