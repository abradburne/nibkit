//! Apps: programs DSlot lists on its Apps page, from the `Roms/APPS` folder of either card.
//! An app is a folder there with an `app.toml`:
//!
//! ```toml
//! name = "Template"
//! description = "Both screens, touch and sound"   # optional, shown under the name
//! command = ["./template", "--verbose"]           # the first word relative to the folder,
//!                                                 # absolute, or found on PATH
//! icon = "icon.png"                               # optional; icon.png if left out
//! runs_on = ["nibos"]                   # "nibos", "stock", "desktop"; nibOS only if left out
//! [env]                                           # optional
//! ```
//!
//! DSlot starts an app as it starts an emulator: it lets go of both screens and the sound,
//! runs the command in the app's folder, and takes them back when it exits. On SIGTERM an app
//! saves what it must and exits (the power menu puts it away; Cancel starts it again).
//!
//! On the stock OS the stock menu's own app scripts (`Roms/APPS/<name>.sh`, with an icon in
//! `Roms/APPS/Imgs/<name>.png`) are apps too, run with bash; on nibOS they are left out, as
//! they are the stock OS's.

use crate::platform;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// An app's description, as its `app.toml` says.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct App {
    /// The name DSlot shows.
    pub name: String,
    /// A line shown under the name.
    #[serde(default)]
    pub description: Option<String>,
    /// The program and its arguments ([`App::command_in`] finds the program).
    pub command: Vec<String>,
    /// The icon, relative to the folder; `icon.png` if left out.
    #[serde(default)]
    pub icon: Option<PathBuf>,
    /// The systems it runs on (`nibos`, `stock`, `desktop`); nibOS only if left out.
    #[serde(default = "nibos_only")]
    pub runs_on: Vec<String>,
    /// Environment variables set for it.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn nibos_only() -> Vec<String> {
    vec!["nibos".into()]
}

impl App {
    /// Reads an `app.toml`, checking what DSlot needs of it.
    pub fn parse(text: &str) -> Result<App, String> {
        let app: App = toml::from_str(text).map_err(|e| e.to_string())?;
        if app.command.is_empty() {
            return Err("an empty command".into());
        }
        if let Some(other) = app
            .runs_on
            .iter()
            .find(|s| !["nibos", "stock", "desktop"].contains(&s.as_str()))
        {
            return Err(format!(
                "runs_on: no system \"{other}\" (nibos, stock, desktop)"
            ));
        }
        Ok(app)
    }

    /// Whether it runs on the system nibkit was built for.
    pub fn runs_here(&self) -> bool {
        self.runs_on.iter().any(|s| s == platform::KEY)
    }

    /// The command with its program found: a first word with a slash in it is relative to the
    /// app's folder (unless absolute); a bare name is left for PATH.
    pub fn command_in(&self, folder: &Path) -> Vec<String> {
        let mut words = self.command.clone();
        if let Some(program) = words.first_mut() {
            if program.contains('/') && !Path::new(program.as_str()).is_absolute() {
                *program = folder.join(program.as_str()).to_string_lossy().into_owned();
            }
        }
        words
    }
}

/// An app found in a `Roms/APPS` folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Its folder (for a stock script, the APPS folder), where it runs.
    pub folder: PathBuf,
    /// Its folder's name (a stock script's name): what DSlot keeps its play time by.
    pub id: String,
    /// Its description.
    pub app: App,
    /// Its icon, if the file is there.
    pub icon: Option<PathBuf>,
}

/// The apps in `<root>/APPS` for each of `roots` (the cards' `Roms` folders), those that run on
/// this system, sorted by name. An `app.toml` that can't be read is logged and passed over.
pub fn apps(roots: &[PathBuf]) -> Vec<Found> {
    let mut found = Vec::new();
    for dir in roots.iter().map(|r| r.join("APPS")) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for path in entries {
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if id.starts_with('.') {
                continue;
            }
            let description = path.join("app.toml");
            if description.is_file() {
                match std::fs::read_to_string(&description)
                    .map_err(|e| e.to_string())
                    .and_then(|t| App::parse(&t))
                {
                    Ok(app) if app.runs_here() => {
                        let icon = path.join(app.icon.clone().unwrap_or_else(|| "icon.png".into()));
                        let icon = icon.is_file().then_some(icon);
                        found.push(Found {
                            folder: path,
                            id,
                            app,
                            icon,
                        });
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("nibkit: {}: {e}", description.display()),
                }
            } else if let Some(script) = stock_script(&path) {
                found.push(script);
            }
        }
    }
    found.sort_by_key(|f| f.app.name.to_lowercase());
    found
}

/// One of the stock menu's app scripts, on the stock OS: `<APPS>/<name>.sh`, except DSlot's own.
fn stock_script(path: &Path) -> Option<Found> {
    if platform::KEY != "stock" || !path.is_file() || path.extension().is_none_or(|e| e != "sh") {
        return None;
    }
    let name = path.file_stem()?.to_str()?.to_owned();
    if name.to_ascii_lowercase().starts_with("dslot") {
        return None;
    }
    let folder = path.parent()?.to_owned();
    let icon = folder.join("Imgs").join(format!("{name}.png"));
    let app = App {
        name: name.clone(),
        description: None,
        command: vec!["/bin/bash".into(), path.to_string_lossy().into_owned()],
        icon: None,
        runs_on: vec!["stock".into()],
        env: BTreeMap::new(),
    };
    Some(Found {
        icon: icon.is_file().then_some(icon),
        folder,
        id: name,
        app,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempDir;

    #[test]
    fn descriptions() {
        let app = App::parse("name = \"Template\"\ncommand = [\"./template\", \"-v\"]\n").unwrap();
        assert_eq!(app.runs_on, ["nibos"]);
        assert_eq!(
            app.command_in(Path::new("/r/APPS/template")),
            ["/r/APPS/template/./template", "-v"]
        );
        let app = App::parse(
            "name = \"T\"\ncommand = [\"python3\", \"t.py\"]\nruns_on = [\"stock\", \"nibos\"]\n",
        )
        .unwrap();
        assert_eq!(app.command_in(Path::new("/x"))[0], "python3");
        assert!(App::parse("name = \"T\"\ncommand = []\n").is_err());
        assert!(App::parse("name = \"T\"\ncommand = [\"x\"]\nruns_on = [\"windows\"]\n").is_err());
        assert!(App::parse("name = \"T\"\ncommand = [\"x\"]\ncolour = \"red\"\n").is_err());
    }

    #[test]
    fn found_on_both_cards() {
        let dir = TempDir::new("apps");
        let (tf1, tf2) = (dir.path().join("tf1/Roms"), dir.path().join("tf2/Roms"));
        let app = |root: &Path, folder: &str, text: &str| {
            std::fs::create_dir_all(root.join("APPS").join(folder)).unwrap();
            std::fs::write(root.join("APPS").join(folder).join("app.toml"), text).unwrap();
        };
        app(
            &tf1,
            "zeta",
            "name = \"Zeta\"\ncommand = [\"./zeta\"]\nruns_on = [\"nibos\", \"stock\", \"desktop\"]\n",
        );
        app(
            &tf2,
            "alpha",
            "name = \"alpha\"\ncommand = [\"./alpha\"]\nruns_on = [\"nibos\", \"stock\", \"desktop\"]\n",
        );
        app(&tf2, "broken", "name = ");
        std::fs::write(tf2.join("APPS/alpha/icon.png"), b"").unwrap();
        std::fs::write(tf1.join("APPS/Ports.sh"), "echo").unwrap();
        std::fs::write(tf1.join("APPS/DSlot.sh"), "echo").unwrap();
        let found = apps(&[tf1.clone(), tf2.clone()]);
        let names: Vec<&str> = found.iter().map(|f| f.app.name.as_str()).collect();
        // The stock menu's scripts are apps on the stock OS only, and DSlot's own never.
        if platform::KEY == "stock" {
            assert_eq!(names, ["alpha", "Ports", "Zeta"]);
        } else {
            assert_eq!(names, ["alpha", "Zeta"]);
        }
        let alpha = &found[0];
        assert_eq!((alpha.id.as_str(), alpha.icon.is_some()), ("alpha", true));
        assert!(found
            .iter()
            .find(|f| f.id == "zeta")
            .is_some_and(|f| f.icon.is_none()));
    }
}
