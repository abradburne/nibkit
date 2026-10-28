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
//! An emulator that plays only some of a system's games lists them, by file name, in a file of
//! its own, named under `[games]`:
//!
//! ```toml
//! [games]
//! GW = "/usr/share/inktick/units.tsv"
//! ```
//!
//! One line a game, tab-separated: its ROM's name without the extension (a MAME set's name,
//! `gnw_dkjr`), then its title, model, year and series (`Donkey Kong Jr.`, `DJ-101`, `1982`,
//! `New Wide Screen`); the last three may be empty, and `#` lines are comments. It is then the
//! default only for the games listed ([`Emulators::choose_for`]), the others going to the next
//! that takes them, and the titles name the games on the shelf ([`Emulators::unit`]).
//!
//! An emulator with a menu of its own can keep the options it declares in DSlot's settings
//! ([`settings_file`]), so the two menus show the same values: under its id, at the console's
//! or the game's level (`[console.GB.inkdot]`, `[game."GB/Tetris (World)".inkdot]`), with
//! [`crate::settings::Settings`]; never at the system level, which DSlot doesn't read options
//! at. A reset removes the key (`Settings::reset`): unset everywhere, the option isn't passed
//! and the emulator's `default` holds. DSlot reads the file again when a game exits. Read it when
//! the game starts and write it when an option changes; DSlot does not touch it meanwhile.
//! What an emulator doesn't declare stays in its own files.
//!
//! An emulator that earns RetroAchievements says for which of its systems, and whether it may
//! play them in hardcore:
//!
//! ```toml
//! [achievements]
//! systems = ["GB", "GBC"]   # where playing earns achievements (some of [systems])
//! hardcore = false          # the default: Casual (softcore) only
//! ```
//!
//! DSlot then hands a game of those systems the player's sign-in in its environment
//! (`RA_TOKEN_FILE`, `RA_USERNAME`, `RA_HARDCORE`: nib-cheevos' `launch`), `RA_HARDCORE=1` only
//! with `hardcore = true` (and only once DSlot offers hardcore at all), and none to a game of
//! any other system; its Achievements page says which systems earn them
//! ([`Emulators::earning`]). Without the table an emulator gets no sign-in.

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
    /// The only games it plays of a system, by the system: a file listing them.
    #[serde(default)]
    games: BTreeMap<String, PathBuf>,
    /// Where it earns RetroAchievements.
    #[serde(default)]
    achievements: Option<AchievementsDecl>,
}

/// `[achievements]` as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AchievementsDecl {
    #[serde(default)]
    systems: Vec<String>,
    #[serde(default)]
    hardcore: bool,
}

/// Where an emulator earns RetroAchievements (`[achievements]`): the systems it plays that
/// earn them, and whether it may play them in hardcore.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Achievements {
    /// The systems, of those it is installed for, whose games earn achievements in it: they
    /// get the player's sign-in when it starts them. Empty: none.
    pub systems: Vec<String>,
    /// It may play them in hardcore (`RA_HARDCORE=1` when the player chose hardcore); else
    /// Casual (softcore) only.
    pub hardcore: bool,
}

/// A game an emulator lists (`[games]`): what it is called and what it was.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    /// Its title: "Donkey Kong Jr.".
    pub title: String,
    /// Its model number: "DJ-101".
    pub model: String,
    /// The year it came out.
    pub year: String,
    /// The series it was one of: "New Wide Screen".
    pub series: String,
}

/// A `[games]` list's lines, by the game's name. Comments, blank lines and lines without a
/// title are passed over.
fn units(text: &str) -> BTreeMap<String, Unit> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut f = l.split('\t').map(str::trim);
            let name = f.next().filter(|n| !n.is_empty())?;
            let title = f.next().filter(|t| !t.is_empty())?;
            let mut next = || f.next().unwrap_or("").to_owned();
            Some((
                name.to_owned(),
                Unit {
                    title: title.to_owned(),
                    model: next(),
                    year: next(),
                    series: next(),
                },
            ))
        })
        .collect()
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
    /// The only games it plays of a system, where it lists them (`[games]`).
    games: Vec<(String, BTreeMap<String, Unit>)>,
    /// Where it earns RetroAchievements (`[achievements]`).
    pub achievements: Achievements,
}

impl Emulator {
    /// Whether it plays `game` (a ROM's name without its extension) of `system`: it plays the
    /// system, and lists the game there or lists none.
    pub fn takes(&self, system: &str, game: &str) -> bool {
        self.plays(system)
            && self
                .games
                .iter()
                .find(|(s, _)| s == system)
                .is_none_or(|(_, list)| list.contains_key(game))
    }

    /// What it lists `game` of `system` as, if it lists it.
    pub fn unit(&self, system: &str, game: &str) -> Option<&Unit> {
        self.games
            .iter()
            .find(|(s, _)| s == system)
            .and_then(|(_, list)| list.get(game))
    }

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

    /// Whether playing `system`'s games in it earns RetroAchievements: it is installed for the
    /// system and its `[achievements]` name it.
    pub fn earns(&self, system: &str) -> bool {
        self.plays(system) && self.achievements.systems.iter().any(|s| s == system)
    }

    /// Reads a description: `None` for one that is not installed. `there` says whether a
    /// program (or a required file) is there.
    fn parse(id: &str, text: &str, there: &Probe) -> Result<Option<Emulator>, String> {
        let d: Description = toml::from_str(text).map_err(|e| e.to_string())?;
        let achievements = d.achievements.unwrap_or(AchievementsDecl {
            systems: Vec::new(),
            hardcore: false,
        });
        if let Some(s) = achievements
            .systems
            .iter()
            .find(|s| !d.systems.contains_key(s.as_str()))
        {
            return Err(format!("achievements: {s} is not one of its systems"));
        }
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
        let achievements = Achievements {
            systems: achievements
                .systems
                .into_iter()
                .filter(|s| plays.contains(s))
                .collect(),
            hardcore: achievements.hardcore,
        };
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
        // A list that can't be read lists nothing: the emulator takes none of that system's
        // games until it can.
        let games = d
            .games
            .into_iter()
            .map(|(system, file)| {
                let text = std::fs::read_to_string(&file).unwrap_or_default();
                (system, units(&text))
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
            games,
            achievements,
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

    /// Every system an installed emulator plays, by its folder name (`NDS`, `GBA`, `PORTS`...),
    /// sorted: the `Roms/<system>` folders a card should have. The one list of them, for DSlot
    /// and nibOS alike.
    pub fn systems(&self) -> Vec<String> {
        let set: std::collections::BTreeSet<&str> = self
            .all
            .iter()
            .flat_map(|e| e.commands.iter().map(|(s, _)| s.as_str()))
            .collect();
        set.into_iter().map(str::to_owned).collect()
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

    /// As [`Emulators::choose`], for one game (its ROM's name without the extension): `wanted`
    /// if it takes the game, else the highest-priority one that does ([`Emulator::takes`]).
    pub fn choose_for(&self, system: &str, game: &str, wanted: Option<&str>) -> Option<&Emulator> {
        wanted
            .and_then(|id| self.get(id))
            .filter(|e| e.takes(system, game))
            .or_else(|| self.all.iter().find(|e| e.takes(system, game)))
    }

    /// The systems whose games earn RetroAchievements in an emulator installed here
    /// ([`Emulator::earns`]), sorted: what DSlot's Achievements page says playing earns them
    /// in.
    pub fn earning(&self) -> Vec<String> {
        let set: std::collections::BTreeSet<&str> = self
            .all
            .iter()
            .flat_map(|e| {
                e.achievements
                    .systems
                    .iter()
                    .filter(|s| e.plays(s))
                    .map(String::as_str)
            })
            .collect();
        set.into_iter().map(str::to_owned).collect()
    }

    /// What the first emulator that lists `game` of `system` calls it (`[games]`).
    pub fn unit(&self, system: &str, game: &str) -> Option<&Unit> {
        self.all.iter().find_map(|e| e.unit(system, game))
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

    /// The systems the installed emulators play, each once: the folders a card should have.
    #[test]
    fn systems() {
        let both = load(&[("dsperate", DSPERATE), ("mgba", MGBA)], &[]);
        let systems = both.systems();
        assert!(systems.windows(2).all(|w| w[0] < w[1]), "{systems:?}");
        assert!(systems.iter().any(|s| s == "NDS") && systems.iter().any(|s| s == "GB"));
        // Not installed: its systems aren't there.
        let one = load(
            &[("dsperate", DSPERATE), ("mgba", MGBA)],
            &["/usr/bin/dsperate"],
        );
        assert!(!one.systems().iter().any(|s| s == "NDS"));
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
    fn games_listed_go_to_the_emulator_that_lists_them() {
        let dir = crate::testing::TempDir::new("dslot-games");
        let list = dir.path().join("units.tsv");
        std::fs::write(
            &list,
            "# set\ttitle\tmodel\tyear\tseries\ngnw_dkjr\tDonkey Kong Jr.\tDJ-101\t1982\tNew Wide Screen\ngnw_ball\tBall\n\nno_title\n",
        )
        .unwrap();
        let inktick = format!(
            "name = \"inktick\"\npriority = 100\n[systems]\nGW = [\"/usr/bin/inktick\", \"{{rom}}\"]\n[games]\nGW = \"{}\"\n",
            list.display()
        );
        let gw =
            "name = \"gw\"\npriority = 50\n[systems]\nGW = [\"/usr/bin/nibcase\", \"{rom}\"]\n";
        let emulators = load(&[("inktick", &inktick), ("nibcase-gw", gw)], &[]);
        let chosen = |game, wanted| {
            emulators
                .choose_for("GW", game, wanted)
                .map(|e| e.id.as_str())
        };
        // Listed: inktick, the higher priority. Not listed: the next that takes it, even when
        // inktick is asked for.
        assert_eq!(chosen("gnw_dkjr", None), Some("inktick"));
        assert_eq!(chosen("gnw_mario", None), Some("nibcase-gw"));
        assert_eq!(chosen("gnw_mario", Some("inktick")), Some("nibcase-gw"));
        assert_eq!(chosen("gnw_dkjr", Some("nibcase-gw")), Some("nibcase-gw"));
        let unit = emulators.unit("GW", "gnw_dkjr").unwrap();
        assert_eq!(
            (unit.title.as_str(), unit.model.as_str(), unit.year.as_str()),
            ("Donkey Kong Jr.", "DJ-101", "1982")
        );
        assert_eq!(emulators.unit("GW", "gnw_ball").unwrap().series, "");
        assert!(emulators.unit("GW", "no_title").is_none());
        // A list that isn't there: it takes none of the system's games.
        let gone = inktick.replace(&list.display().to_string(), "/nowhere/units.tsv");
        let emulators = load(&[("inktick", &gone)], &[]);
        assert!(emulators.choose_for("GW", "gnw_dkjr", None).is_none());
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

    /// `[achievements]`: the systems it plays that earn them, hardcore only when it says so;
    /// a system it doesn't play is a mistake; without the table, none.
    #[test]
    fn where_an_emulator_earns_achievements() {
        let inkdot = r#"
            name = "inkdot"
            [systems]
            GB = ["/usr/bin/inkdot", "{rom}"]
            GBC = ["/usr/bin/inkdot", "{rom}"]
            [achievements]
            systems = ["GB", "GBC"]
        "#;
        let dsperate =
            format!("{DSPERATE}\n[achievements]\nsystems = [\"NDS\"]\nhardcore = true\n");
        let emulators = load(
            &[("inkdot", inkdot), ("dsperate", &dsperate), ("mgba", MGBA)],
            &[],
        );
        let ink = emulators.get("inkdot").unwrap();
        assert!(ink.earns("GB") && ink.earns("GBC") && !ink.earns("NDS"));
        assert!(!ink.achievements.hardcore, "Casual unless it says");
        let ds = emulators.get("dsperate").unwrap();
        assert!(ds.earns("NDS") && !ds.earns("GBA") && ds.achievements.hardcore);
        let mgba = emulators.get("mgba").unwrap();
        assert_eq!(mgba.achievements, Achievements::default());
        assert!(!mgba.earns("GB"));
        assert_eq!(emulators.earning(), ["GB", "GBC", "NDS"]);
        // Not installed for a system: it earns nothing there.
        let some = load(&[("dsperate", &dsperate)], &["/usr/bin/dsperate"]);
        assert!(some.earning().is_empty());
        let parse = |text: &str| Emulator::parse("x", text, &PROBE);
        let wrong = "name = \"X\"\n[systems]\nGB = [\"/x\"]\n[achievements]\nsystems = [\"GBA\"]";
        assert!(parse(wrong).unwrap_err().contains("GBA"));
        let misspelt = "name = \"X\"\n[systems]\nGB = [\"/x\"]\n[achievements]\nsystem = [\"GB\"]";
        assert!(parse(misspelt).is_err());
        let empty = "name = \"X\"\n[systems]\nGB = [\"/x\"]\n[achievements]\n";
        assert!(parse(empty)
            .unwrap()
            .unwrap()
            .achievements
            .systems
            .is_empty());
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
