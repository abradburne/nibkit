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
    #[cfg(feature = "nibos")]
    pub fn new(app: &str) -> AppDirs {
        AppDirs {
            config: xdg("XDG_CONFIG_HOME", ".config").join(app),
            data: xdg("XDG_DATA_HOME", ".local/share").join(app),
            cache: xdg("XDG_CACHE_HOME", ".cache").join(app),
        }
    }

    #[cfg(not(feature = "nibos"))]
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
#[cfg_attr(not(feature = "nibos"), allow(dead_code))]
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
/// starts from and the card in TF2.
pub fn rom_roots() -> Vec<PathBuf> {
    platform::ROM_ROOTS.iter().map(PathBuf::from).collect()
}

/// Where games' saves go, a folder per console and one per game in it: `<root>/GBC/<ROM
/// name>/`. On the ROMS partition, whichever card the game is on.
pub fn saves_root() -> PathBuf {
    PathBuf::from(platform::SAVES_ROOT)
}

/// A game's saves folder: `<saves root>/<console>/<ROM file name without the extension>`.
pub fn saves_dir(root: &Path, console: &str, rom_name: &str) -> PathBuf {
    root.join(console).join(rom_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "nibos")]
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

    #[test]
    fn saves() {
        let dir = saves_dir(&saves_root(), "GBC", "Pokemon - Crystal Version (USA)");
        assert!(dir.ends_with("Saves/GBC/Pokemon - Crystal Version (USA)"));
    }
}
