//! What an emulator (or an app) tells DSlot about itself, in a description file of its own, and
//! the conventions DSlot starts it by (docs/emulators.md in DSlot). On nibOS a package installs
//! its description in `/usr/share/dslot/emulators/<id>.toml`:
//!
//! ```toml
//! name = "inkdot"
//! priority = 100
//! [systems]
//! GB = ["/usr/bin/inkdot", "{options}", "--saves", "{saves}", "{rom}"]
//! GBC = ["/usr/bin/inkdot", "{options}", "--saves", "{saves}", "{rom}"]
//! ```
//!
//! An emulator counts as installed for a system when the program its command for that system
//! starts is there, and so is every file under `requires`. Each system's default is the
//! installed one with the highest priority; DSlot's settings can pick another, for a console or
//! a game. A description also declares the emulator's options, which DSlot shows as settings and
//! turns into words of its command line, and actions on a game's files (clear its resume state).
//!
//! Systems are named as the `Roms/` folders are: `NDS`, `GBA`, `GB`, `GBC`.
//!
//! An emulator with a menu of its own can keep the options it declares in DSlot's settings
//! ([`settings_file`]), so the two menus show the same values: under its id, at the console's
//! or the game's level (`[console.GB.inkdot]`, `[game."GB/Tetris (World)".inkdot]`), with
//! [`crate::settings::Settings`]; never at the system level, which DSlot doesn't read options
//! at. A reset removes the key (`Settings::reset`): unset everywhere, the option isn't passed
//! and the emulator's `default` holds. DSlot reads the file again when a game exits. Read it when
//! the game starts and write it when an option changes; DSlot does not touch it meanwhile.
//! What an emulator doesn't declare stays in its own files.

mod apps;
mod launch;

pub use apps::{apps, App, Found};

pub use launch::{description_dirs, fill, matches, saves_dir, Placeholders};

use crate::settings::{Def, Kind, Levels};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use toml::Value;

/// DSlot's settings file: on nibOS `~/.config/dslot/settings.toml` (`$XDG_CONFIG_HOME`), on a
/// desktop in DSlot's config folder there. On the stock OS DSlot keeps it beside its program,
/// and there is none for an emulator to use.
pub fn settings_file() -> Option<PathBuf> {
    cfg!(not(nibkit_stock)).then(|| {
        crate::paths::AppDirs::new("dslot")
            .config
            .join("settings.toml")
    })
}

/// A description file as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Description {
    name: String,
    #[serde(default)]
    priority: i32,
    /// The command for each system it plays, by the system's folder name (NDS, GBA, GB, GBC).
    systems: BTreeMap<String, Vec<String>>,
    /// Files that must be there as well as the programs (a RetroArch core, say).
    #[serde(default)]
    requires: Vec<PathBuf>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    seed: Vec<Seed>,
    /// For an emulator that goes back to a menu of its own when a game ends: a line of its
    /// output that says so, at which DSlot asks it to quit (SIGTERM) instead.
    #[serde(default)]
    quit_on_output: Option<String>,
    #[serde(default)]
    options: Vec<OptionDecl>,
    #[serde(default)]
    actions: Vec<ActionDecl>,
}

/// An option as declared:
///
/// ```toml
/// [[options]]
/// key = "hires3d"
/// label = "3D resolution"
/// systems = ["NDS"]              # all the emulator's, if left out
/// values = [1, 2, 4]             # a choice ...
/// labels = ["1x", "2x", "4x"]
/// args = ["--hires3d", "{value}"]
/// default = 2                    # what the emulator does unless told; never passed
/// ```
///
/// ... or `kind = "toggle"` with `args` when on and `off` when off, or `kind = "number"` with
/// `min`, `max`, `step` and `unit`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionDecl {
    key: String,
    label: String,
    #[serde(default)]
    systems: Vec<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    values: Vec<Value>,
    #[serde(default)]
    labels: Vec<String>,
    min: Option<i64>,
    max: Option<i64>,
    step: Option<i64>,
    #[serde(default)]
    unit: String,
    default: Option<Value>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    off: Vec<String>,
}

/// An action on a game's files, as declared:
///
/// ```toml
/// [[actions]]
/// label = "Clear resume state"
/// delete = ["{saves}/*.auto.dss"]
/// ```
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionDecl {
    label: String,
    #[serde(default)]
    systems: Vec<String>,
    delete: Vec<String>,
}

/// One of an emulator's options: a setting (its key is `<emulator>.<key>`), the consoles it is
/// for, and the words it adds to the command.
#[derive(Clone, Debug, PartialEq)]
pub struct EmuOption {
    /// The setting it is, keyed `<emulator>.<key>`.
    pub def: Def,
    systems: Vec<String>,
    /// With `{value}` for the value; for a toggle, the words when on.
    args: Vec<String>,
    /// A toggle's words when off.
    off: Vec<String>,
}

impl EmuOption {
    /// Whether it is for `system`'s games.
    pub fn applies(&self, system: &str) -> bool {
        self.systems.iter().any(|s| s == system)
    }

    /// The words `value` adds to the command line.
    pub fn words(&self, value: &Value) -> Vec<String> {
        if self.def.kind == Kind::Toggle {
            return if value.as_bool() == Some(true) {
                self.args.clone()
            } else {
                self.off.clone()
            };
        }
        let value = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        self.args
            .iter()
            .map(|w| w.replace("{value}", &value))
            .collect()
    }

    /// The words of the option's form `words[at..]` starts with, if any, and its value:
    /// `--hires3d 2` gives 2. For reading old launch.conf lines.
    pub fn recognise(&self, words: &[String]) -> Option<(usize, Value)> {
        if self.def.kind == Kind::Toggle {
            for (form, on) in [(&self.args, true), (&self.off, false)] {
                if !form.is_empty() && words.starts_with(form) {
                    return Some((form.len(), Value::Boolean(on)));
                }
            }
            return None;
        }
        if self.args.len() > words.len() {
            return None;
        }
        let mut value = None;
        for (pattern, word) in self.args.iter().zip(words) {
            match pattern.split_once("{value}") {
                Some((before, after)) => {
                    let v = word.strip_prefix(before)?.strip_suffix(after)?;
                    value = Some(v.to_owned());
                }
                None if pattern == word => {}
                None => return None,
            }
        }
        let value = value?;
        let parsed = value
            .parse::<i64>()
            .map(Value::Integer)
            .unwrap_or(Value::String(value));
        self.def
            .accepts(&parsed)
            .then_some((self.args.len(), parsed))
    }

    fn parse(emulator: &str, decl: OptionDecl, plays: &[String]) -> Result<EmuOption, String> {
        let kind = match decl.kind.as_deref() {
            Some("toggle") => Kind::Toggle,
            Some("number") => Kind::Number {
                min: decl.min.ok_or("a number needs a min")?,
                max: decl.max.ok_or("a number needs a max")?,
                step: decl.step.unwrap_or(1).max(1),
                unit: decl.unit,
            },
            None | Some("choice") if !decl.values.is_empty() => {
                let label = |i: usize, v: &Value| {
                    decl.labels.get(i).cloned().unwrap_or_else(|| match v {
                        Value::String(s) => s.clone(),
                        v => v.to_string(),
                    })
                };
                Kind::Choice(
                    decl.values
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (v.clone(), label(i, v)))
                        .collect(),
                )
            }
            other => {
                return Err(format!(
                    "option {}: kind {other:?} (choice with values, toggle or number)",
                    decl.key
                ))
            }
        };
        if decl.key.contains('.') {
            return Err(format!("option {}: no dots in a key", decl.key));
        }
        let systems = if decl.systems.is_empty() {
            plays.to_vec()
        } else {
            decl.systems
        };
        let def = Def {
            key: format!("{emulator}.{}", decl.key),
            label: decl.label,
            kind,
            levels: Levels::CONSOLE_AND_GAME,
            default: decl.default,
        };
        Ok(EmuOption {
            def,
            systems,
            args: decl.args,
            off: decl.off,
        })
    }
}

/// An action on a game's files.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// What DSlot's menu shows for it.
    pub label: String,
    systems: Vec<String>,
    /// Files to delete: paths with placeholders, `*` allowed in the last part.
    pub delete: Vec<String>,
}

impl Action {
    /// Whether it is for `system`'s games.
    pub fn applies(&self, system: &str) -> bool {
        self.systems.iter().any(|s| s == system)
    }
}

/// A file put in place before a game starts, when there is none there yet (an emulator's
/// settings for this handheld, say). Never overwritten.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Seed {
    /// The file to copy.
    pub from: PathBuf,
    /// `~/` at the start is the home folder.
    pub to: PathBuf,
}

/// An installed emulator, with the commands for the consoles it is installed for.
#[derive(Debug)]
pub struct Emulator {
    /// The description's file name without `.toml`: what settings.ini and launch.conf call it.
    pub id: String,
    /// The name DSlot shows.
    pub name: String,
    /// Each system's default is the installed emulator with the highest.
    pub priority: i32,
    commands: Vec<(String, Vec<String>)>,
    /// Environment variables set for it.
    pub env: Vec<(String, String)>,
    /// Files put in place before a game starts.
    pub seed: Vec<Seed>,
    /// A line of its output that says it went back to a menu of its own, at which DSlot asks it
    /// to quit (SIGTERM).
    pub quit_on_output: Option<String>,
    /// Its options, which DSlot shows as settings.
    pub options: Vec<EmuOption>,
    /// Its actions on a game's files.
    pub actions: Vec<Action>,
}

impl Emulator {
    /// The options that apply to `system`'s games.
    pub fn options_for<'a>(&'a self, system: &'a str) -> impl Iterator<Item = &'a EmuOption> {
        self.options.iter().filter(move |o| o.applies(system))
    }

    /// The command template for `system`, if it plays it here.
    pub fn command(&self, system: &str) -> Option<&[String]> {
        self.commands
            .iter()
            .find(|(s, _)| s == system)
            .map(|(_, c)| c.as_slice())
    }

    /// Whether it is installed for `system`.
    pub fn plays(&self, system: &str) -> bool {
        self.command(system).is_some()
    }

    /// Reads a description: `None` for one that is not installed. `there` says whether a
    /// program (or a required file) is there.
    fn parse(id: &str, text: &str, there: &Probe) -> Result<Option<Emulator>, String> {
        let d: Description = toml::from_str(text).map_err(|e| e.to_string())?;
        if !d.requires.iter().all(|f| (there.file)(f)) {
            return Ok(None);
        }
        let mut commands = Vec::new();
        for (system, command) in d.systems {
            match command.first() {
                None => return Err(format!("{system}: an empty command")),
                Some(program) if (there.program)(Path::new(program)) => {
                    commands.push((system, command))
                }
                Some(_) => {}
            }
        }
        if commands.is_empty() {
            return Ok(None);
        }
        let plays: Vec<String> = commands.iter().map(|(s, _)| s.clone()).collect();
        let options = d
            .options
            .into_iter()
            .map(|o| EmuOption::parse(id, o, &plays))
            .collect::<Result<_, _>>()?;
        let systems = |given: Vec<String>| {
            if given.is_empty() {
                plays.clone()
            } else {
                given
            }
        };
        let actions = d
            .actions
            .into_iter()
            .map(|a| Action {
                systems: systems(a.systems),
                label: a.label,
                delete: a.delete,
            })
            .collect();
        Ok(Some(Emulator {
            id: id.to_owned(),
            name: d.name,
            priority: d.priority,
            commands,
            env: d.env.into_iter().collect(),
            seed: d.seed,
            quit_on_output: d.quit_on_output.filter(|s| !s.is_empty()),
            options,
            actions,
        }))
    }
}

/// The installed emulators, highest priority first.
#[derive(Default, Debug)]
pub struct Emulators {
    all: Vec<Emulator>,
}

impl Emulators {
    /// Reads every `*.toml` in `dirs`, in order: a file replaces one of the same name in an
    /// earlier folder (so a user's own `dsperate.toml` wins over the package's).
    pub fn load(dirs: &[PathBuf]) -> Emulators {
        let mut files: BTreeMap<String, PathBuf> = BTreeMap::new();
        for dir in dirs {
            for path in std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
            {
                if path.extension().is_some_and(|e| e == "toml") {
                    if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                        files.insert(id.to_owned(), path);
                    }
                }
            }
        }
        let texts =
            files
                .into_iter()
                .filter_map(|(id, path)| match std::fs::read_to_string(&path) {
                    Ok(text) => Some((id, text)),
                    Err(e) => {
                        eprintln!("nibkit: {}: {e}", path.display());
                        None
                    }
                });
        Emulators::from_texts(
            texts,
            &Probe {
                program: runnable,
                file: Path::is_file,
            },
        )
    }

    /// Descriptions given as `(id, text)`, as if everything they name were installed: to see
    /// how DSlot would choose among them.
    pub fn assume_installed(texts: impl IntoIterator<Item = (String, String)>) -> Emulators {
        Emulators::from_texts(
            texts,
            &Probe {
                program: |_| true,
                file: |_| true,
            },
        )
    }

    fn from_texts(texts: impl IntoIterator<Item = (String, String)>, there: &Probe) -> Emulators {
        let mut all = Vec::new();
        for (id, text) in texts {
            match Emulator::parse(&id, &text, there) {
                Ok(Some(emulator)) => all.push(emulator),
                Ok(None) => eprintln!("nibkit: emulator {id}: not installed"),
                Err(e) => eprintln!("nibkit: emulator {id}: {e}"),
            }
        }
        // Stable: equal priorities stay in file name order.
        all.sort_by_key(|e| std::cmp::Reverse(e.priority));
        Emulators { all }
    }

    /// The emulator with this id, if it is installed.
    pub fn get(&self, id: &str) -> Option<&Emulator> {
        self.all.iter().find(|e| e.id == id)
    }

    /// The emulators installed for `system`, the default first.
    pub fn for_system<'a>(&'a self, system: &'a str) -> impl Iterator<Item = &'a Emulator> {
        self.all.iter().filter(move |e| e.plays(system))
    }

    /// `wanted` if it is installed for `system`, else the system's default.
    pub fn choose(&self, system: &str, wanted: Option<&str>) -> Option<&Emulator> {
        wanted
            .and_then(|id| self.get(id))
            .filter(|e| e.plays(system))
            .or_else(|| self.all.iter().find(|e| e.plays(system)))
    }
}

/// Checks a description as DSlot reads it, as if everything it names were installed: for an
/// emulator's package, before it ships it.
///
/// ```
/// let text = "name = \"inkdot\"\n[systems]\nGB = [\"/usr/bin/inkdot\", \"{rom}\"]\n";
/// let inkdot = nibkit::dslot::check("inkdot", text).unwrap();
/// assert!(inkdot.plays("GB") && !inkdot.plays("GBA"));
/// ```
pub fn check(id: &str, text: &str) -> Result<Emulator, String> {
    let all_there = Probe {
        program: |_| true,
        file: |_| true,
    };
    Emulator::parse(id, text, &all_there)?.ok_or_else(|| format!("{id}: plays no system"))
}

/// How to tell whether what a description names is there; tests stand in their own.
struct Probe {
    program: fn(&Path) -> bool,
    file: fn(&Path) -> bool,
}

/// A program is there: a path to an executable file, or a name found on PATH.
fn runnable(program: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let executable = |p: &Path| {
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if program.components().count() > 1 {
        return executable(program);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|dir| executable(&dir.join(program)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DSPERATE: &str = r#"
        name = "DSperate"
        priority = 100
        [systems]
        NDS = ["/usr/bin/dsperate", "{options}", "{rom}"]
        GBA = ["/usr/bin/dsperate", "--gba", "{options}", "{rom}"]
        [env]
        DS_DEBUG = "marks"
        [[seed]]
        from = "/usr/share/dsperate/dslot/dsperate.ini"
        to = "~/.config/dsperate/dsperate.ini"
    "#;
    const MGBA: &str = r#"
        name = "mGBA"
        priority = 50
        requires = ["/usr/lib/libretro/mgba_libretro.so"]
        [systems]
        GBA = ["/usr/bin/retroarch", "-L", "/usr/lib/libretro/mgba_libretro.so", "{rom}"]
        GB = ["/usr/bin/retroarch", "-L", "/usr/lib/libretro/mgba_libretro.so", "{rom}"]
    "#;

    // Everything is there except what a test says is missing (in `MISSING`, per thread).
    thread_local!(static MISSING: std::cell::RefCell<Vec<PathBuf>> = Default::default());
    const PROBE: Probe = Probe {
        program: there,
        file: there,
    };
    fn there(p: &Path) -> bool {
        MISSING.with(|m| !m.borrow().iter().any(|f| f == p))
    }

    fn load(files: &[(&str, &str)], missing: &[&str]) -> Emulators {
        MISSING.with(|m| *m.borrow_mut() = missing.iter().map(PathBuf::from).collect());
        let texts = files
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()));
        Emulators::from_texts(texts, &PROBE)
    }

    #[test]
    fn reads_a_description() {
        let emulators = load(&[("dsperate", DSPERATE)], &[]);
        let e = emulators.get("dsperate").unwrap();
        assert_eq!((e.name.as_str(), e.priority), ("DSperate", 100));
        assert_eq!(e.command("GBA").unwrap()[1], "--gba");
        assert!(!e.plays("GB"));
        assert_eq!(e.env, [("DS_DEBUG".to_owned(), "marks".to_owned())]);
        assert_eq!(e.seed[0].to, Path::new("~/.config/dsperate/dsperate.ini"));
    }

    #[test]
    fn installed_only() {
        // No program: not listed at all.
        assert!(load(&[("dsperate", DSPERATE)], &["/usr/bin/dsperate"])
            .get("dsperate")
            .is_none());
        // The program but not the core it requires.
        assert!(
            load(&[("mgba", MGBA)], &["/usr/lib/libretro/mgba_libretro.so"])
                .get("mgba")
                .is_none()
        );
        assert!(load(&[("mgba", MGBA)], &[])
            .get("mgba")
            .is_some_and(|e| e.plays("GB")));
    }

    #[test]
    fn default_and_choice() {
        let emulators = load(&[("mgba", MGBA), ("dsperate", DSPERATE)], &[]);
        let ids = |s| {
            emulators
                .for_system(s)
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("GBA"), ["dsperate", "mgba"]);
        assert_eq!(ids("NDS"), ["dsperate"]);
        assert_eq!(ids("GBC"), Vec::<&str>::new());

        let chosen = |s, wanted| emulators.choose(s, wanted).map(|e| e.id.as_str());
        assert_eq!(chosen("GBA", None), Some("dsperate"));
        assert_eq!(chosen("GBA", Some("mgba")), Some("mgba"));
        // One that does not play the console, or is not there, falls back to the default.
        assert_eq!(chosen("NDS", Some("mgba")), Some("dsperate"));
        assert_eq!(chosen("NDS", Some("gone")), Some("dsperate"));
        assert_eq!(chosen("GBC", None), None);
    }

    #[test]
    fn bad_descriptions() {
        // A misspelt key is an error, not silently ignored; a system is any folder name.
        let parse = |text| Emulator::parse("x", text, &PROBE);
        assert!(parse("name = \"X\"\nprio = 1\n[systems]\nNDS = [\"/x\"]").is_err());
        assert!(parse("name = \"X\"\n[systems]\nNDS = []").is_err());
        let snes = parse("name = \"X\"\n[systems]\nSFC = [\"/x\"]\nGB = [\"/x\"]");
        assert!(snes
            .unwrap()
            .is_some_and(|e| e.plays("GB") && e.plays("SFC")));
    }
}
