//! The achievements' badge pictures, for an emulator, in the cache every app on the device
//! shares with DSlot ([`crate::paths::badges_dir`]). A badge not there yet is fetched by `curl`
//! on a thread of its own, written whole or not at all, and handed back by name on a later
//! frame; decoding it is the app's.

use crate::paths::{badge_file, badge_url};
use crate::transport;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// The shared badge folder; `None` without a home ([`crate::paths::badges_dir`]).
pub fn dir() -> Option<PathBuf> {
    crate::paths::badges_dir()
}

/// The badges asked for, and the thread that fetches them.
pub struct Badges {
    dir: PathBuf,
    asks: Sender<(PathBuf, String)>,
    fetched: Receiver<(String, PathBuf)>,
    asked: HashSet<String>,
}

impl Badges {
    /// Badges in `dir` (normally [`dir`]), fetched with `curl`, saying they come from
    /// `user_agent` ([`crate::ua::user_agent`]), when they aren't there. `None` without a
    /// `curl`.
    pub fn start(dir: PathBuf, user_agent: &str) -> Option<Badges> {
        let curl = transport::curl_path()?;
        let user_agent = user_agent.to_owned();
        let (asks, incoming) = channel::<(PathBuf, String)>();
        let (outgoing, fetched) = channel();
        std::thread::Builder::new()
            .name("cheevos-badges".into())
            .spawn(move || {
                transport::ordinary_priority();
                for (to, url) in incoming {
                    let name = to
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    match transport::fetch(&curl, &url, &to, &user_agent) {
                        Ok(()) => {
                            if outgoing.send((name, to)).is_err() {
                                break;
                            }
                        }
                        Err(e) => eprintln!("cheevos: badge {name}: {}", e.why),
                    }
                }
            })
            .ok()?;
        Some(Badges {
            dir,
            asks,
            fetched,
            asked: HashSet::new(),
        })
    }

    /// The badge `name`'s file when it is there already; else `None`, and it is fetched (from
    /// `url`, or the media host's usual place when that isn't an https address), once, and
    /// comes out of [`Badges::fetched`] when it has been.
    pub fn want(&mut self, name: &str, url: &str) -> Option<PathBuf> {
        let file = badge_file(&self.dir, name)?;
        if file.is_file() {
            return Some(file);
        }
        if self.asked.insert(name.to_owned()) {
            let url = if url.starts_with("https://") {
                url.to_owned()
            } else {
                badge_url(name)
            };
            let _ = self.asks.send((file, url));
        }
        None
    }

    /// The badges fetched since the last call: their names and files.
    pub fn fetched(&mut self) -> Vec<(String, PathBuf)> {
        self.fetched.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_badge_already_there_is_not_fetched() {
        if transport::curl_path().is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("nib-cheevos-badges-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("00001.png"), b"png").unwrap();
        let mut b = Badges::start(dir.clone(), "nib-cheevos-test/0").unwrap();
        assert_eq!(b.want("00001", ""), Some(dir.join("00001.png")));
        assert_eq!(b.want("../x", ""), None);
        assert!(b.fetched().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
