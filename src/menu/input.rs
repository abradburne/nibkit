//! The buttons and the touch screen, as DSlot's menu answers them: up and down move the
//! highlight (past headings, keeping it on screen), left and right change the row, A chooses,
//! Y goes back to the level above, L and R turn the page, B goes back. A tap highlights a row;
//! a tap on the highlighted row acts on it, as A would (or steps its value, back on its left
//! third). The app does what the [`Effect`] says and fills the page in again.

use super::layout::{self, VISIBLE};
use super::model::{Menu, Shelf, Value};

/// A button, by what is printed on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Up: the highlight up a row.
    Up,
    /// Down: the highlight down a row.
    Down,
    /// Left: the row's value back.
    Left,
    /// Right: the row's value on.
    Right,
    /// A: choose.
    A,
    /// B: back.
    B,
    /// X: on a shelf, a button on the highlighted card (save).
    X,
    /// Y: back to the level above for the row.
    Y,
    /// L: the page before.
    L,
    /// R: the page after.
    R,
}

/// What a button or a touch asks of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Step this row's value back (-1) or on (+1).
    Change {
        /// The row, among all of them.
        row: usize,
        /// -1 back, +1 on.
        step: i32,
    },
    /// Act on this row (open what it opens, do what it does).
    Choose(usize),
    /// Back to the level above for this row (Y): clear it here.
    Use(usize),
    /// Show this page (by its tab).
    Page(usize),
    /// Back a level, or close the menu at the top.
    Back,
    /// On a shelf: a button on the highlighted card (A load, X save, Y ...).
    Card {
        /// The highlighted card.
        at: usize,
        /// The button.
        key: Key,
    },
}

/// Turns a page by `step` tabs, round from the last to the first.
fn turn(tabs: &[(String, bool)], step: i32) -> Option<Effect> {
    let n = tabs.len() as i32;
    let at = tabs.iter().position(|t| t.1)? as i32;
    (n > 1).then(|| Effect::Page((at + step).rem_euclid(n) as usize))
}

impl Menu {
    /// Whether a row can be highlighted (headings can't).
    fn takes(&self, i: usize) -> bool {
        self.rows.get(i).is_some_and(|r| !r.heading)
    }

    /// Puts the highlight on a row that takes it (the first after a heading) and scrolls so
    /// it shows.
    pub fn settle(&mut self) {
        if !self.takes(self.row) {
            if let Some(i) = (self.row..self.rows.len())
                .chain(0..self.row)
                .find(|&i| self.takes(i))
            {
                self.row = i;
            }
        }
        if self.row < self.scroll {
            self.scroll = self.row;
            // Show the heading over it too, when there is one just above.
            if self.row > 0 && self.rows[self.row - 1].heading {
                self.scroll = self.row - 1;
            }
        } else if self.row >= self.scroll + VISIBLE {
            self.scroll = self.row + 1 - VISIBLE;
        }
    }

    fn step(&mut self, by: i32) {
        let mut i = self.row as i32;
        loop {
            i += by;
            if i < 0 || i >= self.rows.len() as i32 {
                return;
            }
            if self.takes(i as usize) {
                self.row = i as usize;
                self.settle();
                return;
            }
        }
    }

    /// A button.
    pub fn key(&mut self, key: Key) -> Option<Effect> {
        match key {
            Key::Up => {
                self.step(-1);
                None
            }
            Key::Down => {
                self.step(1);
                None
            }
            Key::Left | Key::Right => {
                let step = if key == Key::Left { -1 } else { 1 };
                match self.rows.get(self.row)?.value {
                    Value::Text(_) | Value::Inherited(_) | Value::Stars(_) | Value::Slider(_) => {
                        Some(Effect::Change {
                            row: self.row,
                            step,
                        })
                    }
                    _ => None,
                }
            }
            Key::A => self.takes(self.row).then_some(Effect::Choose(self.row)),
            Key::Y => match self.rows.get(self.row)?.value {
                Value::Text(_) => Some(Effect::Use(self.row)),
                _ => None,
            },
            Key::L => turn(&self.tabs, -1),
            Key::R => turn(&self.tabs, 1),
            Key::B => Some(Effect::Back),
            Key::X => None,
        }
    }

    /// A finger down at `(x, y)` (screen units).
    pub fn touch(&mut self, x: f32, y: f32) -> Option<Effect> {
        if let Some(t) = layout::tab_hit(x, y, self.tabs.len()) {
            return (!self.tabs[t].1).then_some(Effect::Page(t));
        }
        let i = self.scroll + layout::row_hit(y)?;
        if !self.takes(i) {
            return None;
        }
        if i != self.row {
            self.row = i;
            self.settle();
            return None;
        }
        match self.rows[i].value {
            Value::Text(_) | Value::Inherited(_) | Value::Stars(_) | Value::Slider(_) => {
                let back = x < layout::LEFT + (layout::RIGHT - layout::LEFT) / 3.0;
                Some(Effect::Change {
                    row: i,
                    step: if back { -1 } else { 1 },
                })
            }
            _ => Some(Effect::Choose(i)),
        }
    }
}

impl Shelf {
    /// A button.
    pub fn key(&mut self, key: Key) -> Option<Effect> {
        match key {
            Key::Left if self.at > 0 => {
                self.at -= 1;
                None
            }
            Key::Right if self.at + 1 < self.cards.len() => {
                self.at += 1;
                None
            }
            Key::A | Key::X | Key::Y if !self.cards.is_empty() => {
                Some(Effect::Card { at: self.at, key })
            }
            Key::L => turn(&self.tabs, -1),
            Key::R => turn(&self.tabs, 1),
            Key::B => Some(Effect::Back),
            _ => None,
        }
    }

    /// A finger down: a neighbour comes to the middle; the middle card loads, as A would.
    pub fn touch(&mut self, x: f32, y: f32) -> Option<Effect> {
        if let Some(t) = layout::tab_hit(x, y, self.tabs.len()) {
            return (!self.tabs[t].1).then_some(Effect::Page(t));
        }
        match layout::card_hit(x, y)? {
            0 => (!self.cards.is_empty()).then_some(Effect::Card {
                at: self.at,
                key: Key::A,
            }),
            o => {
                let at = self.at as i32 + o;
                if (0..self.cards.len() as i32).contains(&at) {
                    self.at = at as usize;
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Card, Row};

    fn page() -> Menu {
        Menu {
            tabs: vec![
                ("Play".into(), true),
                ("States".into(), false),
                ("Game".into(), false),
            ],
            title: String::new(),
            rows: vec![
                Row::heading("LOOK"),
                Row::new("Look", Value::Text("Pocket".into())),
                Row::new("LCD texture", Value::Inherited("Use GB (On)".into())),
                Row::heading("PLAY"),
                Row::new("Save state", Value::More("Slot 3".into())),
                Row::new("Quit", Value::None),
            ],
            row: 0,
            scroll: 0,
            hint: String::new(),
        }
    }

    #[test]
    fn the_highlight_skips_headings() {
        let mut m = page();
        m.settle();
        assert_eq!(m.row, 1, "past the first heading");
        m.key(Key::Down);
        m.key(Key::Down);
        assert_eq!(m.row, 4, "past PLAY");
        m.key(Key::Down);
        m.key(Key::Down);
        assert_eq!(m.row, 5, "stops at the end");
        m.key(Key::Up);
        m.key(Key::Up);
        m.key(Key::Up);
        assert_eq!(m.row, 1);
    }

    #[test]
    fn buttons_say_what_to_do() {
        let mut m = page();
        m.settle();
        assert_eq!(m.key(Key::Right), Some(Effect::Change { row: 1, step: 1 }));
        assert_eq!(m.key(Key::Y), Some(Effect::Use(1)), "set here: Y goes back");
        m.key(Key::Down);
        assert_eq!(m.key(Key::Y), None, "inherited: nothing to clear");
        assert_eq!(m.key(Key::Left), Some(Effect::Change { row: 2, step: -1 }));
        m.key(Key::Down);
        assert_eq!(
            m.key(Key::Right),
            None,
            "a row that opens something doesn't step"
        );
        assert_eq!(m.key(Key::A), Some(Effect::Choose(4)));
        assert_eq!(
            m.key(Key::L),
            Some(Effect::Page(2)),
            "round to the last tab"
        );
        assert_eq!(m.key(Key::R), Some(Effect::Page(1)));
        assert_eq!(m.key(Key::B), Some(Effect::Back));
    }

    #[test]
    fn a_tap_highlights_then_acts() {
        let mut m = page();
        m.settle();
        let y = |place: usize| layout::row_y(place);
        assert_eq!(m.touch(600.0, y(4)), None);
        assert_eq!(m.row, 4);
        assert_eq!(m.touch(600.0, y(4)), Some(Effect::Choose(4)));
        assert_eq!(m.touch(600.0, y(1)), None);
        assert_eq!(
            m.touch(800.0, y(1)),
            Some(Effect::Change { row: 1, step: 1 })
        );
        assert_eq!(
            m.touch(200.0, y(1)),
            Some(Effect::Change { row: 1, step: -1 })
        );
        assert_eq!(m.touch(600.0, y(3)), None, "a heading takes no tap");
        assert_eq!(m.touch(300.0, 20.0), Some(Effect::Page(2)));
    }

    #[test]
    fn a_long_page_keeps_the_highlight_on_screen() {
        let mut m = Menu {
            rows: (0..20)
                .map(|i| Row::new(format!("{i}"), Value::None))
                .collect(),
            ..Menu::default()
        };
        for _ in 0..12 {
            m.key(Key::Down);
        }
        assert_eq!((m.row, m.scroll), (12, 3));
        for _ in 0..12 {
            m.key(Key::Up);
        }
        assert_eq!((m.row, m.scroll), (0, 0));
    }

    #[test]
    fn a_shelf_moves_and_acts_on_its_cards() {
        let mut s = Shelf {
            tabs: vec![("Play".into(), false), ("States".into(), true)],
            cards: (0..3)
                .map(|i| Card {
                    picture: None,
                    label: format!("{i}"),
                })
                .collect(),
            at: 0,
            hint: String::new(),
        };
        s.key(Key::Left);
        assert_eq!(s.at, 0);
        s.key(Key::Right);
        assert_eq!(s.key(Key::X), Some(Effect::Card { at: 1, key: Key::X }));
        assert_eq!(s.touch(512.0 + layout::CARD_STEP, 300.0), None);
        assert_eq!(s.at, 2);
        assert_eq!(
            s.touch(512.0, 300.0),
            Some(Effect::Card { at: 2, key: Key::A })
        );
        assert_eq!(s.key(Key::L), Some(Effect::Page(0)));
    }
}
