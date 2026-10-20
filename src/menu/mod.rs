//! The menu every app shows on the touch screen: DSlot's, with its tabs, rows, button hints and
//! clock bar, so a game paused in any emulator looks and works as DSlot's own menu does.
//!
//! It is shared as a description of what to draw rather than a renderer, so each app keeps its
//! own way of drawing (DSlot its SDL textures, an emulator its GL picture):
//!
//! - [`Menu`] and [`Shelf`] say what is on the screen: the tabs, the rows and their values, the
//!   highlighted row, the hint line; or a shelf of cards (save states).
//! - [`layout::menu`] and [`layout::shelf`] turn that into [`Draw`]s, text and shapes placed on a
//!   1024 × 768 screen in DSlot's sizes and colours ([`colour`]).
//! - [`Menu::key`], [`Menu::touch`] (and the same on [`Shelf`]) move the highlight with the
//!   buttons and find what a finger touched, saying what to do as an [`Effect`].
//! - [`top`] lays out the top screen while the menu is open: the game paused, or a save
//!   state's picture and details.
//! - [`raster`] (feature `menu-raster`) draws the [`Draw`]s into an RGBA picture in Roboto
//!   Condensed, the face DSlot uses, for an app with nothing of its own to draw text with.
//!
//! ```
//! use nibkit::menu::{layout, Menu, Row, Status, Value};
//! let menu = Menu {
//!     tabs: vec![("Play".into(), true), ("States".into(), false)],
//!     title: "TETRIS".into(),
//!     rows: vec![Row::new("Resume", Value::None), Row::new("Fast-forward", Value::Text("Off".into()))],
//!     row: 0,
//!     scroll: 0,
//!     hint: "A  choose   ·   B  resume".into(),
//! };
//! let draws = layout::menu(&menu, &Status::default());
//! assert!(!draws.is_empty());
//! ```

mod input;
pub mod layout;
mod model;
#[cfg(feature = "menu-raster")]
pub mod raster;
pub mod top;

pub use input::{Effect, Key};
pub use layout::Draw;
pub use model::{Card, Menu, Row, Shelf, Slot, Status, Value};

/// The width of the screen the menu is laid out for (the RG DS+'s panels), in its units.
pub const W: f32 = 1024.0;
/// That screen's height, in its units.
pub const H: f32 = 768.0;

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
    /// Behind everything.
    pub const BLACK: Rgb = [0, 0, 0];
}
