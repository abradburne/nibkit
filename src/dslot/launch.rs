//! How DSlot starts what a description describes: the placeholders in a command, where the
//! descriptions are, where a game's saves go. What DSlot does around it (closing its windows so
//! the program can have both screens, waiting for it, taking them back) is in docs/emulators.md.

use std::path::{Path, PathBuf};

/// The folders DSlot reads descriptions from, in order: a file replaces one of the same name in
/// an earlier folder. On nibOS: packages', the administrator's, the user's. On the stock OS:
/// DSlot's own folder on either card. On a desktop: the user's (DSlot's config folder).
pub fn description_dirs() -> Vec<PathBuf> {
    #[cfg(nibkit_desktop)]
    {
        vec![crate::paths::AppDirs::new("dslot").config.join("emulators")]
    }
    #[cfg(nibkit_nibos)]
    {
        let user = crate::paths::AppDirs::new("dslot").config.join("emulators");
        vec![
            PathBuf::from("/usr/share/dslot/emulators"),
            PathBuf::from("/etc/dslot/emulators"),
            user,
        ]
    }
    #[cfg(nibkit_stock)]
    {
        ["/mnt/mmc", "/mnt/sdcard"]
            .iter()
            .map(|card| Path::new(card).join("Roms/APPS/dslot/emulators"))
            .collect()
    }
}

/// A game's saves folder, as DSlot makes it for `{saves}`: `<saves root>/<system>/<ROM name>`.
pub fn saves_dir(root: &Path, system: &str, rom_name: &str) -> PathBuf {
    crate::paths::saves_dir(root, system, rom_name)
}

/// What a command's placeholders stand for.
pub struct Placeholders<'a> {
    /// `{rom}`: the ROM file, as found (a .zip too).
    pub rom: &'a Path,
    /// `{name}`: its file name without the extension.
    pub name: &'a str,
    /// `{system}`: its system's folder name.
    pub system: &'a str,
    /// `{saves}`: the game's saves folder, where there is one.
    pub saves: Option<&'a Path>,
}

/// A word of a command (or an action's path) with its placeholders filled in. `{options}`,
/// which stands for any number of words, is the caller's to expand.
pub fn fill(word: &str, p: &Placeholders) -> String {
    word.replace("{rom}", &p.rom.to_string_lossy())
        .replace("{name}", p.name)
        .replace("{system}", p.system)
        .replace(
            "{saves}",
            &p.saves.unwrap_or(Path::new("")).to_string_lossy(),
        )
}

/// A file name against a pattern where `*` stands for any run of characters (an action's
/// `delete`).
pub fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, tail)) => {
            let Some(rest) = name.strip_prefix(head) else {
                return false;
            };
            (0..=rest.len())
                .filter(|&i| rest.is_char_boundary(i))
                .any(|i| matches(tail, &rest[i..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        let saves = Path::new("/mnt/roms/Saves/GB/Tetris (World)");
        let p = Placeholders {
            rom: Path::new("/r/GB/Tetris (World).zip"),
            name: "Tetris (World)",
            system: "GB",
            saves: Some(saves),
        };
        assert_eq!(
            fill("--saves={saves}", &p),
            "--saves=/mnt/roms/Saves/GB/Tetris (World)"
        );
        assert_eq!(fill("{system}:{name}", &p), "GB:Tetris (World)");
        assert_eq!(
            saves_dir(Path::new("/mnt/roms/Saves"), "GB", "Tetris (World)"),
            saves
        );
    }

    #[test]
    fn wildcards() {
        assert!(matches("*.auto.dss", "CEYE.auto.dss"));
        assert!(!matches("*.auto.dss", "CEYE.1.dss"));
        assert!(matches("save*", "save") && matches("a*b*c", "a-b-c") && !matches("a*b", "a-c"));
    }

    #[test]
    fn dirs() {
        assert!(description_dirs().iter().all(|d| d.ends_with("emulators")));
    }
}
