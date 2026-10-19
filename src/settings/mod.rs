//! Settings at three levels: the **system** (the app, or the device, as a whole), a **console**
//! (every game of it: `NDS`, `GBA`, `GB`, `GBC`, as the `Roms/` folders are named) and a
//! **game** (one ROM). A value set for a game wins over its console's, which wins over the
//! system's; one set nowhere is the setting's default ([`Def`]). An app that has no consoles
//! or games uses the system level alone.
//!
//! They are kept in one TOML file, with a table for the app's own state:
//!
//! ```toml
//! [state]
//! last = "GBC/Tetris DX (World)"
//!
//! [system]
//! palette = "green"
//!
//! [console.GB]
//! palette = "grey"
//!
//! [game."GB/Tetris (World)"]
//! palette = "green"
//! frameskip = 1
//! ```
//!
//! ```
//! use nibkit::settings::{Scope, Settings};
//! # let dir = std::env::temp_dir().join("nibkit-doc-settings");
//! let mut settings = Settings::load(dir.join("settings.toml"));
//! settings.set(Scope::Console("GB".into()), "palette", "grey".into());
//! let chain = Scope::chain("GB", "Tetris (World)");
//! let (value, from) = settings.resolve(&chain, "palette").unwrap();
//! assert_eq!((value.as_str(), from), (Some("grey"), &Scope::Console("GB".into())));
//! # let _ = std::fs::remove_dir_all(dir);
//! ```

pub mod def;

pub use def::{Def, Kind, Levels};
pub use toml::Value;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Table;

/// Where a value is set.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Scope {
    System,
    /// A console, by its `Roms/` folder name: `GBC`.
    Console(String),
    /// A game, by its console and its ROM's file name without the extension: `GBC/Tetris DX
    /// (World)`, which tells it from a game of another console of the same name.
    Game(String),
}

impl Scope {
    /// A game's key: `<console>/<ROM name>`.
    pub fn game_key(console: &str, rom_name: &str) -> String {
        format!("{console}/{rom_name}")
    }

    /// The levels a game's settings are looked up at, most particular first.
    pub fn chain(console: &str, rom_name: &str) -> [Scope; 3] {
        [
            Scope::Game(Scope::game_key(console, rom_name)),
            Scope::Console(console.to_owned()),
            Scope::System,
        ]
    }

    /// The levels a console's settings are looked up at.
    pub fn console_chain(console: &str) -> [Scope; 2] {
        [Scope::Console(console.to_owned()), Scope::System]
    }
}

/// The settings, as read from their file.
pub struct Settings {
    file: PathBuf,
    values: BTreeMap<Scope, BTreeMap<String, Value>>,
    /// The app's own state: what it was showing, and the like. Kept as it is.
    pub state: Table,
    /// Comment lines written at the top of the file.
    pub header: String,
}

impl Settings {
    /// Reads `file`. A file that is missing, or isn't TOML, gives no settings (the defaults);
    /// tables and values it doesn't understand are skipped.
    pub fn load(file: impl Into<PathBuf>) -> Settings {
        let header = "# Settings: [system], then [console.<NAME>], then [game.\"<console>/<ROM name>\"], each\n\
                      # winning over the one before. Edit while the app is not running.\n";
        let mut settings = Settings {
            file: file.into(),
            values: BTreeMap::new(),
            state: Table::new(),
            header: header.into(),
        };
        if let Ok(text) = fs::read_to_string(&settings.file) {
            settings.read(&text);
        }
        settings
    }

    /// The file the settings are kept in.
    pub fn file(&self) -> &Path {
        &self.file
    }

    fn read(&mut self, text: &str) {
        let table: Table = match text.parse() {
            Ok(table) => table,
            Err(e) => {
                eprintln!("nibkit: {}: {e}", self.file.display());
                return;
            }
        };
        for (section, value) in table {
            let Value::Table(entries) = value else {
                continue;
            };
            match section.as_str() {
                "state" => self.state = entries,
                "system" => self.read_scope(Scope::System, entries),
                "console" | "game" => {
                    for (name, entries) in entries {
                        let Value::Table(entries) = entries else {
                            continue;
                        };
                        let scope = if section == "console" {
                            Scope::Console(name)
                        } else {
                            Scope::Game(name)
                        };
                        self.read_scope(scope, entries);
                    }
                }
                _ => eprintln!("nibkit: {}: [{section}] skipped", self.file.display()),
            }
        }
    }

    /// One level's values. A table in it holds keys with a dot: `dsperate.hires3d`.
    fn read_scope(&mut self, scope: Scope, entries: Table) {
        let values = self.values.entry(scope).or_default();
        for (key, value) in entries {
            match value {
                Value::Table(sub) => {
                    for (k, v) in sub {
                        values.insert(format!("{key}.{k}"), v);
                    }
                }
                value => {
                    values.insert(key, value);
                }
            }
        }
    }

    /// The settings as their file has them.
    pub fn to_toml(&self) -> String {
        let mut out = Table::new();
        if !self.state.is_empty() {
            out.insert("state".into(), Value::Table(self.state.clone()));
        }
        let (mut consoles, mut games) = (Table::new(), Table::new());
        for (scope, values) in self.values.iter().filter(|(_, v)| !v.is_empty()) {
            let mut table = Table::new();
            for (key, value) in values {
                match key.split_once('.') {
                    Some((group, k)) => {
                        let sub = table
                            .entry(group)
                            .or_insert_with(|| Value::Table(Table::new()));
                        if let Value::Table(sub) = sub {
                            sub.insert(k.to_owned(), value.clone());
                        }
                    }
                    None => {
                        table.insert(key.clone(), value.clone());
                    }
                }
            }
            let table = Value::Table(table);
            match scope {
                Scope::System => drop(out.insert("system".into(), table)),
                Scope::Console(c) => drop(consoles.insert(c.clone(), table)),
                Scope::Game(g) => drop(games.insert(g.clone(), table)),
            }
        }
        for (name, table) in [("console", consoles), ("game", games)] {
            if !table.is_empty() {
                out.insert(name.into(), Value::Table(table));
            }
        }
        format!(
            "{}\n{}",
            self.header,
            toml::to_string(&out).unwrap_or_default()
        )
    }

    /// Writes the settings to their file, making its folder if need be.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&self.file, self.to_toml())
    }

    /// The value set at exactly this level.
    pub fn get(&self, scope: &Scope, key: &str) -> Option<&Value> {
        self.values.get(scope)?.get(key)
    }

    pub fn set(&mut self, scope: Scope, key: &str, value: Value) {
        self.values
            .entry(scope)
            .or_default()
            .insert(key.to_owned(), value);
    }

    /// Lets this level inherit `key` again.
    pub fn reset(&mut self, scope: &Scope, key: &str) {
        if let Some(values) = self.values.get_mut(scope) {
            values.remove(key);
        }
    }

    /// The value of `key` seen from the first level of `chain`: the first level along it that
    /// sets it, and which level that is.
    pub fn resolve<'a, 's>(
        &'a self,
        chain: &'s [Scope],
        key: &str,
    ) -> Option<(&'a Value, &'s Scope)> {
        chain
            .iter()
            .find_map(|scope| Some((self.get(scope, key)?, scope)))
    }

    /// `def`'s value seen from `chain`: as resolved, at a level `def` can be set at and if it is
    /// a value `def` takes; else its default.
    pub fn value(&self, chain: &[Scope], def: &Def) -> Option<Value> {
        let set = chain
            .iter()
            .filter(|scope| def.levels.has(scope))
            .find_map(|scope| self.get(scope, &def.key))
            .filter(|v| def.accepts(v));
        set.or(def.default.as_ref()).cloned()
    }

    /// Where `def`'s value seen from `chain` comes from: a level, or `None` for its default.
    pub fn source<'s>(&self, chain: &'s [Scope], def: &Def) -> Option<&'s Scope> {
        chain
            .iter()
            .filter(|scope| def.levels.has(scope))
            .find(|scope| self.get(scope, &def.key).is_some_and(|v| def.accepts(v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    fn palette() -> Def {
        Def {
            key: "palette".into(),
            label: "Palette".into(),
            kind: Kind::choice(&[("green", "Green"), ("grey", "Grey")]),
            levels: Levels::ALL,
            default: Some("green".into()),
        }
    }

    #[test]
    fn levels() {
        let dir = TempDir::new("settings-levels");
        let mut s = Settings::load(dir.path().join("settings.toml"));
        let chain = Scope::chain("GB", "Tetris (World)");
        let def = palette();
        assert_eq!(s.value(&chain, &def), Some("green".into()));
        assert_eq!(s.source(&chain, &def), None);

        s.set(Scope::System, "palette", "grey".into());
        assert_eq!(s.source(&chain, &def), Some(&Scope::System));
        s.set(Scope::Console("GB".into()), "palette", "green".into());
        s.set(
            Scope::Game("GB/Tetris (World)".into()),
            "palette",
            "grey".into(),
        );
        assert_eq!(s.value(&chain, &def), Some("grey".into()));
        s.reset(&Scope::Game("GB/Tetris (World)".into()), "palette");
        assert_eq!(s.source(&chain, &def), Some(&Scope::Console("GB".into())));

        // A value the setting doesn't take is passed over; so is a level it can't be set at.
        s.set(Scope::Console("GB".into()), "palette", "purple".into());
        assert_eq!(s.source(&chain, &def), Some(&Scope::System));
        let system_only = Def {
            levels: Levels::SYSTEM,
            ..def
        };
        s.set(Scope::Console("GB".into()), "palette", "green".into());
        assert_eq!(s.value(&chain, &system_only), Some("grey".into()));
    }

    #[test]
    fn round_trip() {
        let dir = TempDir::new("settings-file");
        let file = dir.path().join("config/app/settings.toml");
        let mut s = Settings::load(&file);
        s.state.insert("page".into(), "gba".into());
        s.set(Scope::System, "sounds", false.into());
        s.set(Scope::Console("NDS".into()), "dsperate.hires3d", 2.into());
        s.set(
            Scope::Game("NDS/Golden Sun".into()),
            "dsperate.args",
            "--speed 150".into(),
        );
        s.save().unwrap();
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# Settings"));
        assert!(text.contains("[console.NDS.dsperate]"), "{text}");

        let s = Settings::load(&file);
        assert_eq!(s.state.get("page").and_then(Value::as_str), Some("gba"));
        assert_eq!(
            s.get(&Scope::System, "sounds"),
            Some(&Value::Boolean(false))
        );
        assert_eq!(
            s.get(&Scope::Console("NDS".into()), "dsperate.hires3d"),
            Some(&Value::Integer(2))
        );
        let args = s.get(&Scope::Game("NDS/Golden Sun".into()), "dsperate.args");
        assert_eq!(args.and_then(Value::as_str), Some("--speed 150"));
    }

    #[test]
    fn not_toml() {
        let dir = TempDir::new("settings-bad");
        let file = dir.path().join("settings.toml");
        fs::write(&file, "palette = = grey").unwrap();
        let s = Settings::load(&file);
        assert_eq!(s.value(&[Scope::System], &palette()), Some("green".into()));
    }
}
