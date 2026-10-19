//! How the time is shown: a 12-hour or a 24-hour clock, one preference for every app (DSlot's
//! status bar, an emulator's clock). Kept in `~/.config/nibos/prefs.toml` (`clock = "12h"` or
//! `"24h"`); with none set, the locale's habit (`LC_ALL`, `LC_TIME`, `LANG`): 12-hour where
//! that is usual (the US, Canada, Australia, ...), 24-hour everywhere else and with no locale.
//!
//! ```
//! use nibkit::clock;
//! assert_eq!(clock::format(17, 5, false), "17:05");
//! assert_eq!(clock::format(17, 5, true), "5:05 PM");
//! let shown = clock::format(9, 30, clock::hour12());
//! # let _ = shown;
//! ```
//!
//! An app showing the time reads [`hour12`] again every few seconds (it is a small file), so a
//! change made in another app shows soon after.

use std::path::PathBuf;

/// The preferences file every app shares.
pub fn prefs_file() -> PathBuf {
    crate::paths::AppDirs::new("nibos")
        .config
        .join("prefs.toml")
}

/// Whether the clock is shown 12-hour: as set, or else as the locale has it.
pub fn hour12() -> bool {
    let set = std::fs::read_to_string(prefs_file())
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|t| t.get("clock")?.as_str().map(str::to_owned));
    match set.as_deref() {
        Some("12h") => true,
        Some("24h") => false,
        _ => locale_hour12(locale().as_deref()),
    }
}

/// Sets the clock 12-hour or 24-hour, for every app; the file's other preferences are kept.
pub fn set_hour12(on: bool) -> std::io::Result<()> {
    let file = prefs_file();
    let mut table = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .unwrap_or_default();
    table.insert("clock".into(), (if on { "12h" } else { "24h" }).into());
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Replaced whole, so another app reading it never sees half a file.
    let new = file.with_extension(format!("toml.{}.new", std::process::id()));
    std::fs::write(
        &new,
        toml::to_string(&table).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&new, &file)
}

/// The time as the clock shows it: `17:05`, or `5:05 PM`.
pub fn format(hour: u32, minute: u32, hour12: bool) -> String {
    if !hour12 {
        return format!("{hour:02}:{minute:02}");
    }
    let half = if hour < 12 { "AM" } else { "PM" };
    let h = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{h}:{minute:02} {half}")
}

/// The locale the time is shown in, as the environment says.
fn locale() -> Option<String> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
}

/// Whether a locale (`en_US.UTF-8`) shows a 12-hour clock: by its country.
fn locale_hour12(locale: Option<&str>) -> bool {
    const TWELVE: &[&str] = &[
        "US", "CA", "AU", "NZ", "PH", "IN", "PK", "BD", "EG", "SA", "MY", "CO", "SV", "HN", "NI",
    ];
    let Some(locale) = locale else {
        return false;
    };
    let country = locale
        .split(['.', '@'])
        .next()
        .and_then(|l| l.split('_').nth(1));
    // French Canada keeps the 24-hour clock.
    if locale.starts_with("fr_CA") {
        return false;
    }
    country.is_some_and(|c| TWELVE.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format(0, 0, false), "00:00");
        assert_eq!(format(9, 7, false), "09:07");
        assert_eq!(format(23, 59, false), "23:59");
        assert_eq!(format(0, 0, true), "12:00 AM");
        assert_eq!(format(9, 7, true), "9:07 AM");
        assert_eq!(format(12, 0, true), "12:00 PM");
        assert_eq!(format(13, 30, true), "1:30 PM");
        assert_eq!(format(23, 59, true), "11:59 PM");
    }

    #[test]
    fn locales() {
        assert!(locale_hour12(Some("en_US.UTF-8")));
        assert!(locale_hour12(Some("en_AU")));
        assert!(!locale_hour12(Some("en_GB.UTF-8")));
        assert!(!locale_hour12(Some("ja_JP.UTF-8")));
        assert!(!locale_hour12(Some("fr_CA.UTF-8")));
        assert!(!locale_hour12(Some("C")));
        assert!(!locale_hour12(Some("C.UTF-8")));
        assert!(!locale_hour12(None));
    }
}
