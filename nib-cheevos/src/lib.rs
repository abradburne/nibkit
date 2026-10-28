//! RetroAchievements for nibOS: one implementation for every emulator and for DSlot, the
//! launcher, through rcheevos, the service's own library (vendored under `rcheevos/`, MIT).
//!
//! - [`client`] (feature `client`): an emulator's session, driven once per emulated frame: signed
//!   in, the game's set loaded and evaluated against the machine's memory ([`Memory`]), what
//!   happens told as [`Event`]s, progress kept in save states. [`badges`] fetches the badges'
//!   pictures for it.
//! - [`hash`] (feature `hash`): the identity the service gives a game, by rhash, for every
//!   console nibOS plays, named as the `Roms/` folders are.
//! - [`api`] (feature `api`): the requests a launcher makes outside a game (signing in, a game's
//!   sets, the player's unlocks), built and read by rcheevos.
//! - [`launch`]: the sign-in, from DSlot to the game: the token file and the environment.
//! - [`ua`]: the User-Agent every request carries.
//! - [`paths`]: the service's hosts, the badge cache every app shares, the service's warnings.
//!
//! The network is `curl`'s: nib-cheevos has no TLS of its own. Nothing here is English meant for
//! the player but the service's own titles and messages: each app words what happens itself.

#[cfg(feature = "api")]
pub mod api;
#[cfg(feature = "client")]
pub mod badges;
#[cfg(feature = "client")]
pub mod client;
mod ffi;
#[cfg(feature = "hash")]
pub mod hash;
pub mod launch;
pub mod paths;
#[cfg(any(feature = "client", feature = "api"))]
mod transport;
pub mod ua;

#[cfg(feature = "client")]
pub use client::{Achievement, Client, Event, Leaderboard, Memory, Settings, State, Summary};
pub use launch::Credentials;
#[cfg(any(feature = "client", feature = "api"))]
pub use transport::curl_path;

/// rcheevos' version: "12.4".
pub fn rcheevos_version() -> &'static str {
    // SAFETY: the shim returns a static NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(ffi::nib_rc_version()) }
        .to_str()
        .unwrap_or("0")
}

#[cfg(test)]
mod tests {
    #[test]
    fn rcheevos_is_the_vendored_one() {
        assert!(super::rcheevos_version().starts_with("12.4"));
    }
}
