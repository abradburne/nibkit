//! The menu every app shows on the touch screen: DSlot's, with its tabs, rows, button hints and
//! clock bar, so a game paused in any emulator looks and works as DSlot's own menu does.
//!
//! It is shared as a description of what to draw rather than a renderer, so each app keeps its
//! own way of drawing (DSlot its SDL textures, an emulator its GL picture):
//!
//! - [`Menu`] and [`Shelf`] say what is on the screen: the tabs (and under them the page's
//!   sections, where it has more than one), the rows and their values, the highlighted row, the
//!   hint line; or a shelf of cards (save states).
//! - [`layout::menu`] and [`layout::shelf`] turn that into [`Draw`]s, text and shapes placed on a
//!   1024 × 768 screen in DSlot's sizes and colours ([`colour`]).
//! - [`Menu::key`], [`Menu::touch`] (and the same on [`Shelf`]) move the highlight with the
//!   buttons and find what a finger touched, saying what to do as an [`Effect`].
//! - [`keyboard`] is the on-screen keyboard, for a passphrase, a PIN or a search.
//! - `connections` (with feature `net-control`) is a ready-made page of Wi-Fi and Bluetooth
//!   on or off and the paired devices to connect, for a game's pause menu.
//! - [`top`] lays out the top screen while the menu is open: the game paused, or a save
//!   state's picture and details.
//! - [`raster`] (feature `menu-raster`) draws the [`Draw`]s into an RGBA picture in Roboto
//!   Condensed, the face DSlot uses, for an app with nothing of its own to draw text with.
//! - [`toast`] (feature `menu-raster`) draws a toast over a game that is playing (an
//!   achievement unlocked, a problem) and queues them, so they look and come and go the same
//!   in every app.
//! - [`indicators`] (feature `menu-raster`) keeps RetroAchievements' progress and challenge
//!   indicators on screen while they last at the bottom left: the progress a panel in the
//!   toast's style, each challenge its badge alone in a pulsing gold ring.
//! - [`scale_for`] says what scale a frame of any size is drawn at.
//!
//! ```
//! use nibkit::menu::{layout, Menu, Row, Status, Value};
//! let menu = Menu {
//!     tabs: vec![("Play".into(), true), ("States".into(), false)],
//!     sections: Vec::new(),
//!     title: "TETRIS".into(),
//!     rows: vec![Row::new("Resume", Value::None), Row::new("Fast-forward", Value::Text("Off".into()))],
//!     row: 0,
//!     scroll: 0,
//!     hint: "A  choose   ·   B  resume".into(),
//! };
//! let draws = layout::menu(&menu, &Status::default());
//! assert!(!draws.is_empty());
//! ```

#[cfg(feature = "net-control")]
pub mod connections;
pub mod glyphs;
#[cfg(feature = "menu-raster")]
pub mod indicators;
mod input;
pub mod keyboard;
pub mod keys;
pub mod layout;
mod model;
#[cfg(feature = "menu-raster")]
pub mod raster;
#[cfg(feature = "menu-raster")]
pub mod toast;
pub mod top;

pub use glyphs::Glyph;
pub use input::{Effect, Key};
pub use layout::Draw;
pub use model::{Card, Menu, Row, Shelf, Slot, Status, Value};

/// The width of the screen the menu is laid out for (the RG DS+'s panels), in its units.
pub const W: f32 = 1024.0;
/// That screen's height, in its units.
pub const H: f32 = 768.0;

/// The least scale anything is drawn at: below it, text is under a pixel tall.
pub(crate) const LEAST_SCALE: f32 = 0.05;

/// The pixels to a unit for a frame `w` × `h` pixels: the most at which the 1024 × 768 screen
/// fits in it, keeping its shape (1 on the RG DS+'s panels, 2 in a 2048 × 1536 window, 0.25 on a
/// 256 × 192 DS screen), never below 0.05. What to draw the menu, the top screen and a toast at
/// for that frame.
pub fn scale_for(w: usize, h: usize) -> f32 {
    checked_scale((w as f32 / W).min(h as f32 / H))
}

/// `scale` as one to draw at: one that isn't a number (NaN, infinite) is 1, one below 0.05 is
/// 0.05. Every scale an app gives nibkit's drawing goes through it, so a bad one is the same
/// mistake wherever it is made.
pub(crate) fn checked_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.max(LEAST_SCALE)
    } else {
        1.0
    }
}

/// DSlot's colours.
pub mod colour {
    /// A colour: red, green and blue, 0-255 each.
    pub type Rgb = [u8; 3];
    /// Text that matters: values, the highlighted row's label.
    pub const WHITE: Rgb = [241, 243, 248];
    /// Labels, and a value inherited from the level above.
    pub const DIM: Rgb = [154, 163, 184];
    /// Headings, hints, what is out of reach.
    pub const FAINT: Rgb = [98, 106, 128];
    /// The highlight band, unselected tabs, the bar.
    pub const PANEL: Rgb = [28, 33, 44];
    /// Stars, the selected card's edge.
    pub const GOLD: Rgb = [255, 196, 60];
    /// A warning: an action that deletes, asking for the second press.
    pub const RED: Rgb = [236, 72, 92];
    /// The text on the selected tab.
    pub const TAB_INK: Rgb = [20, 23, 30];
    /// The edge of a toast's panel.
    pub const EDGE: Rgb = [52, 60, 80];
    /// Behind everything.
    pub const BLACK: Rgb = [0, 0, 0];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frames_scale_is_the_most_the_screen_fits_at() {
        assert_eq!(scale_for(1024, 768), 1.0);
        assert_eq!(scale_for(2048, 1536), 2.0);
        assert_eq!(scale_for(256, 192), 0.25);
        // A wide window: as tall as it is.
        assert_eq!(scale_for(4096, 768), 1.0);
        assert_eq!(scale_for(0, 0), LEAST_SCALE);
    }

    #[test]
    fn a_bad_scale_is_one_and_a_tiny_one_the_least() {
        assert_eq!(checked_scale(f32::NAN), 1.0);
        assert_eq!(checked_scale(f32::INFINITY), 1.0);
        assert_eq!(checked_scale(-2.0), LEAST_SCALE);
        assert_eq!(checked_scale(0.01), LEAST_SCALE);
        assert_eq!(checked_scale(1.5), 1.5);
    }
}
