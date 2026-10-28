//! The sign-in, from the launcher to the game. The player signs in once, in DSlot, which keeps
//! the service's token in a file of its own and starts a game with three variables in its
//! environment ([`env`]); the emulator signs in with that token ([`Launch::from_env`]), never a
//! password, and never writes the token anywhere.
//!
//! The token file is two lines, the username then the token (DSperate's `--cheevos-token`
//! reads the same), readable by its owner only, written whole or not at all
//! ([`write_token_file`]). A file of the token alone is read too, the username then coming
//! from `RA_USERNAME`.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The token file's path.
pub const ENV_TOKEN_FILE: &str = "RA_TOKEN_FILE";
/// Who the token signs in as.
pub const ENV_USERNAME: &str = "RA_USERNAME";
/// `1` when the player chose hardcore (and the launcher offers it), else `0`.
pub const ENV_HARDCORE: &str = "RA_HARDCORE";

/// The sign-in: who, and the token that stands for them. The password is never seen here, and
/// the token is never printed, `{:?}` included.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    /// The account as the service spells it; may be empty when the token file had only the
    /// token.
    pub username: String,
    /// The service's token for it.
    pub token: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Reads a token file: the username, then the token, one a line; or the token alone (the
/// username left empty). Blank lines and whitespace around either are ignored.
pub fn read_token_file(path: &Path) -> Result<Credentials, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(2)
        .collect();
    match lines[..] {
        [] => Err(format!("{}: no token in the file", path.display())),
        [token] => Ok(Credentials {
            username: String::new(),
            token: token.to_owned(),
        }),
        [username, token, ..] => Ok(Credentials {
            username: username.to_owned(),
            token: token.to_owned(),
        }),
    }
}

/// Writes the token file, readable by its owner only, whole or not at all: to a file beside it
/// first, then renamed over it, so a crash never leaves half a sign-in. Its folder is made if
/// it isn't there.
pub fn write_token_file(path: &Path, credentials: &Credentials) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let part = path.with_file_name(format!(".{name}.part"));
    let write = || -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&part)?;
        #[cfg(unix)]
        {
            // One left from before keeps its mode through truncation: set it again.
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        write!(file, "{}\n{}\n", credentials.username, credentials.token)?;
        file.sync_all()?;
        std::fs::rename(&part, path)
    };
    write().inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })
}

/// Removes the token file: signed out. One that isn't there is no error.
pub fn remove_token_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// The environment a game starts with for a player signed in: where the token file is, who it
/// signs in as, and whether to play hardcore.
pub fn env(token_file: &Path, username: &str, hardcore: bool) -> Vec<(String, String)> {
    vec![
        (
            ENV_TOKEN_FILE.into(),
            token_file.to_string_lossy().into_owned(),
        ),
        (ENV_USERNAME.into(), username.into()),
        (ENV_HARDCORE.into(), if hardcore { "1" } else { "0" }.into()),
    ]
}

/// The sign-in a game was started with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    /// The token file it came from.
    pub token_file: PathBuf,
    /// Who, and the token; the username is never empty.
    pub credentials: Credentials,
    /// The player chose hardcore (`RA_HARDCORE=1`). An emulator that doesn't offer it plays
    /// Casual (softcore) anyway, and may say so.
    pub hardcore: bool,
}

impl Launch {
    /// The sign-in in this process's environment: `Ok(None)` without `RA_TOKEN_FILE` (no
    /// session; nothing to say), an error for a token file that can't be read or a username
    /// found nowhere.
    pub fn from_env() -> Result<Option<Launch>, String> {
        Launch::from_vars(|name| std::env::var(name).ok())
    }

    /// [`Launch::from_env`] with the variables from `var`.
    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Option<Launch>, String> {
        let Some(token_file) = var(ENV_TOKEN_FILE).filter(|v| !v.is_empty()) else {
            return Ok(None);
        };
        let token_file = PathBuf::from(token_file);
        let mut credentials = read_token_file(&token_file)?;
        if credentials.username.is_empty() {
            credentials.username = var(ENV_USERNAME).unwrap_or_default().trim().to_owned();
        }
        if credentials.username.is_empty() {
            return Err(format!(
                "{} has a token but no username, and {ENV_USERNAME} is not set",
                token_file.display()
            ));
        }
        Ok(Some(Launch {
            token_file,
            credentials,
            hardcore: var(ENV_HARDCORE).is_some_and(|v| v.trim() == "1"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nib-cheevos-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn creds(user: &str, token: &str) -> Credentials {
        Credentials {
            username: user.into(),
            token: token.into(),
        }
    }

    #[test]
    fn token_files_in_both_formats() {
        let dir = temp("formats");
        let two = dir.join("two");
        std::fs::write(&two, "Alan\nt0k3n\n").unwrap();
        assert_eq!(read_token_file(&two).unwrap(), creds("Alan", "t0k3n"));
        let spaced = dir.join("spaced");
        std::fs::write(&spaced, "\n  Alan  \r\n t0k3n\n\n").unwrap();
        assert_eq!(read_token_file(&spaced).unwrap(), creds("Alan", "t0k3n"));
        let one = dir.join("one");
        std::fs::write(&one, "t0k3n").unwrap();
        assert_eq!(read_token_file(&one).unwrap(), creds("", "t0k3n"));
        let none = dir.join("none");
        std::fs::write(&none, " \n").unwrap();
        assert!(read_token_file(&none).is_err());
        assert!(read_token_file(&dir.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_token_is_never_printed() {
        let shown = format!("{:?}", creds("Alan", "t0k3n"));
        assert!(
            shown.contains("Alan") && !shown.contains("t0k3n"),
            "{shown}"
        );
    }

    #[test]
    fn the_token_file_round_trips_whole_and_private() {
        let dir = temp("write");
        let path = dir.join("sub").join("retroachievements.token");
        let alan = creds("Alan", "t0k3n");
        write_token_file(&path, &alan).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Alan\nt0k3n\n");
        assert_eq!(read_token_file(&path).unwrap(), alan);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // One there already, more open, is made private too.
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            write_token_file(&path, &creds("Alan", "new")).unwrap();
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        // Written beside it and renamed: nothing else is left in the folder.
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["retroachievements.token"]);
        remove_token_file(&path).unwrap();
        remove_token_file(&path).unwrap();
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_reads_the_sign_in_from_its_environment() {
        let dir = temp("env");
        let file = dir.join("retroachievements.token");
        write_token_file(&file, &creds("Alan", "t0k3n")).unwrap();
        let vars = env(&file, "Alan", false);
        assert_eq!(
            vars.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["RA_TOKEN_FILE", "RA_USERNAME", "RA_HARDCORE"]
        );
        let get = |vars: Vec<(String, String)>| {
            move |name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
        };
        let launch = Launch::from_vars(get(vars)).unwrap().unwrap();
        assert_eq!(launch.credentials, creds("Alan", "t0k3n"));
        assert_eq!(
            (launch.token_file.as_path(), launch.hardcore),
            (file.as_path(), false)
        );
        assert!(
            Launch::from_vars(get(env(&file, "Alan", true)))
                .unwrap()
                .unwrap()
                .hardcore
        );
        // Nothing passed: no session, and nothing to say.
        assert_eq!(Launch::from_vars(|_| None), Ok(None));
        // A token alone takes the username from RA_USERNAME, and fails without it.
        let one = dir.join("one");
        std::fs::write(&one, "t0k3n\n").unwrap();
        let vars = env(&one, "Bea", false);
        assert_eq!(
            Launch::from_vars(get(vars)).unwrap().unwrap().credentials,
            creds("Bea", "t0k3n")
        );
        let vars = env(&one, "", false);
        assert!(Launch::from_vars(get(vars)).is_err());
        let vars = env(&dir.join("gone"), "Alan", false);
        assert!(Launch::from_vars(get(vars)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
