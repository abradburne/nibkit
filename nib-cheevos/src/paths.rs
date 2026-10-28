//! Where RetroAchievements' things are, the same for every app on the device: the service and
//! its media host, the badge pictures' cache every app shares, and the service's "warning"
//! entries, which are not achievements.

use std::path::{Path, PathBuf};

/// The service: its `dorequest.php` takes rcheevos' requests.
pub const HOST: &str = "https://retroachievements.org";
/// Where the service keeps pictures: badges under `Badge/`, game icons under `Images/`.
pub const MEDIA_HOST: &str = "https://media.retroachievements.org";
/// Where an account is made, on a phone or a computer: the service has no way to make one from
/// a device.
pub const SIGN_UP: &str = "https://retroachievements.org/createaccount.php";

/// The first id of the service's own "Warning: Unknown Emulator" entries ("Hardcore unlocks
/// cannot be earned using this emulator"), which it puts in the set of an emulator it doesn't
/// know, earned already. Not achievements: never shown, counted or announced.
pub const WARNING_ID: u32 = 101_000_001;

/// Whether the achievement with this id is one of the service's warnings ([`WARNING_ID`]).
pub const fn is_warning(id: u32) -> bool {
    id >= WARNING_ID
}

/// The badge pictures' folder, which DSlot and every emulator share:
/// `$XDG_CACHE_HOME/retroachievements/badges` for an absolute `XDG_CACHE_HOME` (a relative one
/// is ignored, as the XDG rules say), else `~/.cache/retroachievements/badges`. `None` without
/// either.
pub fn badges_dir() -> Option<PathBuf> {
    badges_dir_from(
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// [`badges_dir`] from the two variables.
fn badges_dir_from(xdg_cache: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let cache = xdg_cache
        .filter(|p| p.is_absolute())
        .or_else(|| home.filter(|p| p.is_absolute()).map(|h| h.join(".cache")))?;
    Some(cache.join("retroachievements/badges"))
}

/// A badge's file in `dir`: `<name>.png`. `None` for a name that could reach outside it (only
/// letters, digits, `_` and `-`: the service's names are digits, `_lock` for a locked one).
pub fn badge_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    plain.then(|| dir.join(format!("{name}.png")))
}

/// Where the badge `name` is on the media host.
pub fn badge_url(name: &str) -> String {
    format!("{MEDIA_HOST}/Badge/{name}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badges_are_under_an_absolute_cache_home_or_the_home_cache() {
        let p = |s: &str| Some(PathBuf::from(s));
        assert_eq!(
            badges_dir_from(p("/x/cache"), p("/home/n")),
            p("/x/cache/retroachievements/badges")
        );
        // A relative XDG_CACHE_HOME is ignored.
        assert_eq!(
            badges_dir_from(p("cache"), p("/home/n")),
            p("/home/n/.cache/retroachievements/badges")
        );
        assert_eq!(badges_dir_from(None, p("rel")), None);
        assert_eq!(badges_dir_from(None, None), None);
    }

    #[test]
    fn names_stay_in_the_folder() {
        let d = Path::new("/c");
        assert_eq!(badge_file(d, "12345"), Some(PathBuf::from("/c/12345.png")));
        assert_eq!(
            badge_file(d, "12345_lock"),
            Some(PathBuf::from("/c/12345_lock.png"))
        );
        assert_eq!(badge_file(d, ""), None);
        assert_eq!(badge_file(d, "../x"), None);
        assert_eq!(badge_file(d, "a/b"), None);
        assert_eq!(badge_file(d, "a.b"), None);
        assert_eq!(
            badge_url("00001"),
            "https://media.retroachievements.org/Badge/00001.png"
        );
    }

    #[test]
    fn warnings_are_not_achievements() {
        assert!(!is_warning(1));
        assert!(!is_warning(101_000_000));
        assert!(is_warning(WARNING_ID));
        assert!(is_warning(101_000_002));
    }
}
