//! The User-Agent every request to the service carries. RetroAchievements asks that it name the
//! program, its version and the system it runs on, then the core that plays the game where that
//! is a part of its own, then rcheevos' own clause (`rc_client_get_user_agent_clause`):
//!
//! ```text
//! inkdot/0.1.0 (nibOS; linux aarch64) rcheevos/12.4
//! nibcase/0.3.0 (nibOS; linux aarch64) snes9x/1.62.3 rcheevos/12.4
//! ```

use std::ffi::c_char;

/// The User-Agent for `product` at `version`, with `core` (`name/version`, the core that plays
/// the game, for a program that runs several) when there is one. The system is the one the
/// program runs on ([`system`]).
pub fn user_agent(product: &str, version: &str, core: Option<&str>) -> String {
    let system = system();
    let core = core
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| format!("{c} "))
        .unwrap_or_default();
    format!(
        "{product}/{version} ({system}; {} {}) {core}{}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        rcheevos_clause()
    )
}

/// rcheevos' part of the User-Agent: `rcheevos/12.4`.
pub fn rcheevos_clause() -> String {
    let mut clause = [0 as c_char; 64];
    // SAFETY: the shim writes at most `clause.len()` bytes, NUL included.
    unsafe { crate::ffi::nib_rc_user_agent_clause(clause.as_mut_ptr(), clause.len()) };
    // SAFETY: NUL-terminated within the buffer (rcheevos makes sure of it).
    unsafe { crate::ffi::string(clause.as_ptr()) }
}

/// The system the program runs on: `nibOS` where nibos-hw is installed (its
/// `/usr/lib/nibos/board.conf`: nibOS is Arch Linux ARM underneath, which is what its
/// os-release says), else what `/etc/os-release` names it (the stock OS's own name), else the
/// operating system's.
pub fn system() -> String {
    system_from(
        std::path::Path::new(NIBOS_BOARD).is_file(),
        std::fs::read_to_string("/etc/os-release").ok().as_deref(),
    )
}

/// nibos-hw's description of the board, there on nibOS alone.
const NIBOS_BOARD: &str = "/usr/lib/nibos/board.conf";

/// [`system`] from whether nibos-hw is there and the os-release file.
fn system_from(nibos: bool, os_release: Option<&str>) -> String {
    if nibos {
        return "nibOS".to_owned();
    }
    os_release
        .and_then(os_release_name)
        .unwrap_or_else(|| std::env::consts::OS.to_owned())
}

/// `NAME` in an os-release file, unquoted.
fn os_release_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("NAME="))
        .map(|v| v.trim().trim_matches(['"', '\'']).trim().to_owned())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_agent_names_the_program_its_system_and_rcheevos() {
        let ua = user_agent("inkdot", "0.1.0", None);
        let system = system();
        assert!(ua.starts_with(&format!("inkdot/0.1.0 ({system}; ")), "{ua}");
        let rc = rcheevos_clause();
        assert!(rc.starts_with("rcheevos/12."), "{rc}");
        assert!(ua.ends_with(&format!(") {rc}")), "{ua}");
        let ua = user_agent("nibcase", "0.3.0", Some("snes9x/1.62.3"));
        assert!(ua.ends_with(&format!(") snes9x/1.62.3 {rc}")), "{ua}");
        assert_eq!(user_agent("x", "1", Some(" ")), user_agent("x", "1", None));
    }

    #[test]
    fn the_system_is_nibos_or_os_releases_name() {
        let arch = "NAME=\"Arch Linux ARM\"\nPRETTY_NAME=\"Arch Linux ARM\"\nID=archarm\n";
        assert_eq!(
            system_from(true, Some(arch)),
            "nibOS",
            "nibOS is Arch underneath"
        );
        assert_eq!(system_from(false, Some(arch)), "Arch Linux ARM");
        assert_eq!(system_from(false, None), std::env::consts::OS);
        let nibos = "ID=nibos\nNAME=\"nibOS\"\nPRETTY_NAME=\"nibOS 0.1.0\"\n";
        assert_eq!(os_release_name(nibos).as_deref(), Some("nibOS"));
        assert_eq!(
            os_release_name("NAME=Buildroot\n").as_deref(),
            Some("Buildroot")
        );
        assert_eq!(os_release_name("PRETTY_NAME=x\nNAME=\"\"\n"), None);
        assert_eq!(os_release_name(""), None);
    }
}
