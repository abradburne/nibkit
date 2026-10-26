//! Where an app keeps its files, and where the games and their saves are.
//!
//! On nibOS an app's files are the user's, in the XDG folders: `~/.config/<app>/` for settings,
//! `~/.local/share/<app>/` for data, `~/.cache/<app>/` for what can be made again. On the stock
//! OS, where an app lives in a folder on a card (`Roms/APPS/<app>/`) and runs as root, they are
//! in that folder, beside the program.
//!
//! ```
//! let dirs = nibkit::paths::AppDirs::new("inkdot");
//! let settings = dirs.config.join("settings.toml");
//! # let _ = settings;
//! ```

use crate::platform;
use std::path::{Path, PathBuf};

/// An app's folders. Nothing is created: make a folder before writing into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDirs {
    /// Settings, small and hand-editable.
    pub config: PathBuf,
    /// What the app keeps: records, its own saves.
    pub data: PathBuf,
    /// What can be thrown away and made again.
    pub cache: PathBuf,
}

impl AppDirs {
    /// The XDG folders for `app`: `~/.config/<app>`, `~/.local/share/<app>`, `~/.cache/<app>`.
    #[cfg(nibkit_nibos)]
    pub fn new(app: &str) -> AppDirs {
        AppDirs {
            config: xdg("XDG_CONFIG_HOME", ".config").join(app),
            data: xdg("XDG_DATA_HOME", ".local/share").join(app),
            cache: xdg("XDG_CACHE_HOME", ".cache").join(app),
        }
    }

    /// The platform's own folders for apps: `~/Library/Application Support/<app>` (and
    /// `~/Library/Caches/<app>`) on macOS, `%APPDATA%\\<app>` on Windows, the XDG folders
    /// elsewhere.
    #[cfg(nibkit_desktop)]
    pub fn new(app: &str) -> AppDirs {
        let home = || PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        if cfg!(target_os = "macos") {
            let library = home().join("Library");
            AppDirs {
                config: library.join("Application Support").join(app),
                data: library.join("Application Support").join(app),
                cache: library.join("Caches").join(app),
            }
        } else if cfg!(windows) {
            let roaming = std::env::var_os("APPDATA").map_or_else(home, PathBuf::from);
            let local =
                std::env::var_os("LOCALAPPDATA").map_or_else(|| roaming.clone(), PathBuf::from);
            AppDirs {
                config: roaming.join(app),
                data: roaming.join(app),
                cache: local.join(app).join("cache"),
            }
        } else {
            AppDirs {
                config: xdg("XDG_CONFIG_HOME", ".config").join(app),
                data: xdg("XDG_DATA_HOME", ".local/share").join(app),
                cache: xdg("XDG_CACHE_HOME", ".cache").join(app),
            }
        }
    }

    /// The folder the program is in, for settings and data, and a `cache` folder in it.
    #[cfg(nibkit_stock)]
    pub fn new(_app: &str) -> AppDirs {
        let here = exe_dir();
        AppDirs {
            config: here.clone(),
            data: here.clone(),
            cache: here.join("cache"),
        }
    }
}

/// An XDG base directory: the variable, if it is an absolute path, or its default in the home
/// folder.
#[cfg_attr(nibkit_stock, allow(dead_code))]
fn xdg(var: &str, default: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(dir) if Path::new(&dir).is_absolute() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(default),
    }
}

/// The folder the running program is in.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_default()
}

/// The `Roms` folders games are in, each with a folder per console in it (`NDS`, `GBA`, `GB`,
/// `GBC`, ...), as the stock OS lays the cards out: on nibOS the ROMS partition of the card it
/// starts from and the card in TF2. After them, each card's `Games` folder, where a card set
/// up for slot CFW keeps its games (`Games/GBA/...`): both layouts work at once. A ROM in both
/// is the one in `Roms` ([`slot_copy`]).
pub fn rom_roots() -> Vec<PathBuf> {
    platform::rom_roots()
}

/// Whether `rom` is in a slot CFW `Games/<system>/` folder and the same file is also in that
/// card's `Roms/<system>/`, or the other card's: a copy to leave out of a list of games, the one
/// in `Roms` standing for both.
pub fn slot_copy(rom: &Path, roots: &[PathBuf]) -> bool {
    let (Some(dir), Some(file)) = (rom.parent(), rom.file_name()) else {
        return false;
    };
    let (Some(games), Some(system)) = (dir.parent(), dir.file_name()) else {
        return false;
    };
    if games.file_name() != Some("Games".as_ref()) {
        return false;
    }
    roots
        .iter()
        .filter(|r| r.file_name() == Some("Roms".as_ref()))
        .any(|r| r.join(system).join(file).is_file())
}

/// Waits, up to `timeout`, for a card that is in a slot but not mounted yet (on nibOS the TF2
/// card, mounted by udev moments after the kernel sees it). Returns at once when there is
/// nothing to wait for. For a launcher listing games at power-on, before it reads
/// [`rom_roots`]'s folders.
pub fn wait_for_cards(timeout: std::time::Duration) {
    platform::wait_for_cards(timeout)
}

/// Where games' saves go, a folder per console and one per game in it: `<root>/GBC/<ROM
/// name>/`. On the ROMS partition, whichever card the game is on.
pub fn saves_root() -> PathBuf {
    platform::saves_root()
}

/// A game's saves folder: `<saves root>/<console>/<ROM file name without the extension>`.
pub fn saves_dir(root: &Path, console: &str, rom_name: &str) -> PathBuf {
    root.join(console).join(rom_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(nibkit_nibos)]
    #[test]
    fn xdg_dirs() {
        // The variables are the process's: this test is the only one that sets them.
        std::env::set_var("HOME", "/home/u");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CACHE_HOME", "relative/is/ignored");
        std::env::set_var("XDG_DATA_HOME", "/data");
        let dirs = AppDirs::new("inkdot");
        assert_eq!(dirs.config, Path::new("/home/u/.config/inkdot"));
        assert_eq!(dirs.cache, Path::new("/home/u/.cache/inkdot"));
        assert_eq!(dirs.data, Path::new("/data/inkdot"));
    }

    /// Two cards, one laid out as the stock OS does (`Roms/`), the other set up for slot CFW
    /// (`Games/`) with a game that is in both.
    #[test]
    fn slot_layout() {
        let dir = crate::testing::TempDir::new("slot-layout");
        let (one, two) = (dir.path().join("one"), dir.path().join("two"));
        let file = |p: PathBuf| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"rom").unwrap();
            p
        };
        let ours = file(one.join("Roms/GBA/Metroid Fusion.gba"));
        let both = file(two.join("Games/GBA/Metroid Fusion.gba"));
        let only = file(two.join("Games/GB/Tetris.gb"));
        let roots = [
            one.join("Roms"),
            two.join("Roms"),
            one.join("Games"),
            two.join("Games"),
        ];
        assert!(slot_copy(&both, &roots));
        assert!(!slot_copy(&only, &roots));
        assert!(!slot_copy(&ours, &roots));
        // Only a Games/ copy gives way, never one in Roms/.
        let again = file(two.join("Roms/GBA/Metroid Fusion.gba"));
        assert!(!slot_copy(&again, &roots));
    }

    #[test]
    fn saves() {
        let dir = saves_dir(&saves_root(), "GBC", "Pokemon - Crystal Version (USA)");
        assert!(dir.ends_with("Saves/GBC/Pokemon - Crystal Version (USA)"));
    }
}
