//! What the menu shows: the app fills these in, the layout places them.

/// A page of rows under a row of tabs, as DSlot's menu shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    /// Each page's tab, and whether it is the page on show.
    pub tabs: Vec<(String, bool)>,
    /// The line under the tabs: what the page is about (a game's name, a console's).
    pub title: String,
    /// All the page's rows, on screen or not.
    pub rows: Vec<Row>,
    /// The highlighted row, among all of them.
    pub row: usize,
    /// The first row on screen ([`crate::menu::Cursor`] keeps it so the highlight is in view).
    pub scroll: usize,
    /// The buttons' line above the bar: what each does here.
    pub hint: String,
}

/// One row: its label on the left, its value on the right.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: Value,
    /// A heading over the rows that follow it, not a row to highlight: drawn small and faint.
    pub heading: bool,
    /// A few words beside the value, small and faint ("stops charging at about 80%").
    pub note: Option<String>,
}

impl Row {
    pub fn new(label: impl Into<String>, value: Value) -> Row {
        Row {
            label: label.into(),
            value,
            heading: false,
            note: None,
        }
    }

    /// The row with a note beside its value.
    pub fn with_note(self, note: impl Into<String>) -> Row {
        Row {
            note: Some(note.into()),
            ..self
        }
    }

    /// A heading line: rows below it belong to it.
    pub fn heading(label: impl Into<String>) -> Row {
        Row {
            label: label.into(),
            value: Value::None,
            heading: true,
            note: None,
        }
    }
}

/// What a row shows on its right.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Nothing: the row is an action (Resume, Quit).
    None,
    /// Set at this level: white; ‹ › round it when highlighted, for left and right.
    Text(String),
    /// Where the value comes from, not set here ("Use GB (Light)"): muted.
    Inherited(String),
    /// Opens a page or a list of its own ("Slot 3", "2 on"): white with › after it.
    More(String),
    /// A rating, 0-5 stars.
    Stars(u8),
    /// A level, 0-100, with its bar.
    Slider(u8),
    /// An action waiting for its second press ("A again to delete"): white, or red when it
    /// deletes something.
    Prompt { text: String, warning: bool },
}

/// A shelf of cards, one highlighted, as DSlot shows carts: save state slots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shelf {
    pub tabs: Vec<(String, bool)>,
    pub cards: Vec<Card>,
    /// The highlighted card.
    pub at: usize,
    pub hint: String,
}

/// One card: a picture (the app's own, by index), or empty, with a label under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// Which of the app's pictures goes on the card (it draws [`crate::menu::Draw::Picture`]
    /// with this), or `None` for an empty slot.
    pub picture: Option<usize>,
    pub label: String,
}

/// The bar along the bottom: the clock, the battery, and the slot cut into its top edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// The time as shown (`nibkit::clock::format`).
    pub clock: String,
    /// The battery's level and whether it is charging, where there is one.
    pub battery: Option<(u8, bool)>,
    pub slot: Slot,
}

impl Default for Status {
    fn default() -> Status {
        Status {
            clock: String::new(),
            battery: None,
            slot: Slot::Two(560.0),
        }
    }
}

/// The slot in the bar's top edge, as the console's games go into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    /// The DS card slot: a round dip.
    One,
    /// Slot-2's opening, for carts this wide (screen units): GBA 560, Game Boy as DSlot draws it.
    Two(f32),
}
