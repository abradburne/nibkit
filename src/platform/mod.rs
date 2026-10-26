//! The facts that differ between the systems nibkit runs on. Each module provides the same
//! items; the public modules read them from here. Which one is built is `build.rs`'s choice.

#[cfg(nibkit_nibos)]
mod nibos;
#[cfg(nibkit_nibos)]
pub use nibos::*;

#[cfg(nibkit_stock)]
mod stock;
#[cfg(nibkit_stock)]
pub use stock::*;

#[cfg(nibkit_desktop)]
mod desktop;
#[cfg(nibkit_desktop)]
pub use desktop::*;

/// The games' `Roms` folders, on a system whose are fixed places.
#[cfg(nibkit_stock)]
pub fn rom_roots() -> Vec<std::path::PathBuf> {
    ROM_ROOTS.iter().map(std::path::PathBuf::from).collect()
}

/// Nothing to wait for: the cards are mounted before anything runs, or there are none.
#[cfg(not(nibkit_nibos))]
pub fn wait_for_cards(_timeout: std::time::Duration) {}

/// Where saves go, on a system whose is a fixed place.
#[cfg(not(nibkit_desktop))]
pub fn saves_root() -> std::path::PathBuf {
    std::path::PathBuf::from(SAVES_ROOT)
}
