//! The on-screen keyboard every app shows on the touch screen to have something typed: a
//! passphrase, a PIN, a name to search for. It is shared the way the menu is: a [`Keyboard`]
//! says what is typed and which key is highlighted, [`Keyboard::draws`] lays it out as
//! [`Draw`]s on the 1024 × 768 screen, and [`Keyboard::key`] and [`Keyboard::touch`] answer the
//! buttons and fingers, saying when it is [`Typed::Done`] or [`Typed::Cancel`]led.
//!
//! Buttons: the D-pad moves the highlight, A types its key, B deletes, Y shows or hides what is
//! typed (where it is [`Purpose::Secret`]), L is shift, R the symbols, Start is Done, Select
//! Cancel. A tap types the key under it.
//!
//! ```
//! use nibkit::menu::keyboard::{Key, Keyboard, Purpose, Typed};
//! let mut kb = Keyboard::new("Passphrase for Home", Purpose::Secret, 63);
//! for c in ['h', 'i'] {
//!     kb.type_char(c);
//! }
//! assert_eq!(kb.key(Key::Start), Some(Typed::Done("hi".into())));
//! ```

use super::colour::{self, Rgb};
use super::layout::Draw;
use super::{H, W};

/// What is being typed, which picks the keys and whether it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Words: letters, digits and symbols, shown as typed.
    Text,
    /// A passphrase or password: the same keys, shown as dots until Y shows it.
    Secret,
    /// A PIN or passkey: digits only, on a number pad.
    Digits,
}

/// A button, by what is printed on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Up: the highlight up a row.
    Up,
    /// Down: the highlight down a row.
    Down,
    /// Left: the highlight left.
    Left,
    /// Right: the highlight right.
    Right,
    /// A: type the highlighted key.
    A,
    /// B: delete the last character.
    B,
    /// X: a space.
    X,
    /// Y: show or hide a secret.
    Y,
    /// L: shift.
    L,
    /// R: the symbols.
    R,
    /// Start: done.
    Start,
    /// Select: cancel.
    Select,
}

/// What typing came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typed {
    /// Done, with what was typed.
    Done(String),
    /// Cancelled: nothing is to be done with it.
    Cancel,
}

/// A key on the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cap {
    Char(char),
    Shift,
    Symbols,
    Space,
    Delete,
    Show,
    Cancel,
    Done,
}

/// The keyboard: what is typed, and its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyboard {
    /// What it is for, shown above what is typed ("Passphrase for Home").
    pub title: String,
    /// What is typed so far.
    pub text: String,
    /// What it is for.
    pub purpose: Purpose,
    /// The most characters it takes.
    pub max: usize,
    /// A line under what is typed: a hint, or why the last try didn't work.
    pub note: Option<String>,
    shift: bool,
    symbols: bool,
    shown: bool,
    /// The highlighted key: its row, and its place in the row.
    at: (usize, usize),
}

const LETTERS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const SYMBOLS: [&str; 3] = ["!@#$%^&*()", "-_=+[]{};", ":'\",./?"];
const MORE: [&str; 3] = ["`~\\|<>€£¥•", "§°¿¡«»…·", "¢±×÷'\""];

/// A key placed on screen: its row, its place in the row, its cap, and its box (x, y, w, h).
type Placed = (usize, usize, Cap, (f32, f32, f32, f32));

/// Where the keys start, and the room each takes.
const KEYS_TOP: f32 = 236.0;
const ROW: f32 = 92.0;
const UNIT: f32 = 94.0;
const GAP: f32 = 8.0;

impl Keyboard {
    /// An empty keyboard for typing `purpose`, up to `max` characters, under `title`.
    pub fn new(title: impl Into<String>, purpose: Purpose, max: usize) -> Keyboard {
        Keyboard {
            title: title.into(),
            text: String::new(),
            purpose,
            max,
            note: None,
            shift: false,
            symbols: false,
            shown: false,
            at: (1, 0),
        }
    }

    /// The rows of keys now, each key with its width in units.
    fn rows(&self) -> Vec<Vec<(Cap, f32)>> {
        let chars =
            |s: &str| -> Vec<(Cap, f32)> { s.chars().map(|c| (Cap::Char(c), 1.0)).collect() };
        if self.purpose == Purpose::Digits {
            let pad =
                |s: &str| -> Vec<(Cap, f32)> { s.chars().map(|c| (Cap::Char(c), 3.0)).collect() };
            return vec![
                pad("123"),
                pad("456"),
                pad("789"),
                vec![
                    (Cap::Cancel, 3.0),
                    (Cap::Char('0'), 3.0),
                    (Cap::Delete, 3.0),
                ],
                vec![(Cap::Done, 9.0 + 2.0 * GAP / UNIT)],
            ];
        }
        let set = match (self.symbols, self.shift) {
            (false, false) => LETTERS.map(str::to_owned),
            (false, true) => LETTERS.map(str::to_uppercase),
            (true, false) => SYMBOLS.map(str::to_owned),
            (true, true) => MORE.map(str::to_owned),
        };
        let mut third = vec![(Cap::Shift, 1.5)];
        third.extend(chars(&set[2]));
        third.push((Cap::Delete, 1.5));
        let mut last = vec![(Cap::Symbols, 1.5)];
        if self.purpose == Purpose::Secret {
            last.push((Cap::Show, 1.5));
        }
        let space = if self.purpose == Purpose::Secret {
            3.0
        } else {
            4.5
        };
        last.extend([(Cap::Space, space), (Cap::Cancel, 1.5), (Cap::Done, 2.0)]);
        vec![
            chars("1234567890"),
            chars(&set[0]),
            chars(&set[1]),
            third,
            last,
        ]
    }

    /// Each key's place on screen: its row, place, cap and box.
    fn boxes(&self) -> Vec<Placed> {
        let mut out = Vec::new();
        for (r, row) in self.rows().iter().enumerate() {
            let width: f32 = row.iter().map(|(_, u)| u * UNIT).sum::<f32>();
            let mut x = (W - width) / 2.0;
            let y = KEYS_TOP + r as f32 * ROW;
            for (i, &(cap, units)) in row.iter().enumerate() {
                let w = units * UNIT;
                out.push((r, i, cap, (x + GAP / 2.0, y, w - GAP, ROW - GAP)));
                x += w;
            }
        }
        out
    }

    fn cap_at(&self, (r, i): (usize, usize)) -> Option<Cap> {
        self.rows()
            .get(r)
            .and_then(|row| row.get(i))
            .map(|(c, _)| *c)
    }

    /// Types `c`, if there is room for it.
    pub fn type_char(&mut self, c: char) {
        if self.text.chars().count() < self.max {
            self.text.push(c);
            self.note = None;
        }
    }

    /// Does what a key does; `Some` when typing is over.
    fn press(&mut self, cap: Cap) -> Option<Typed> {
        match cap {
            Cap::Char(c) => {
                self.type_char(c);
                // Shift lasts for one letter.
                if !self.symbols {
                    self.shift = false;
                }
            }
            Cap::Shift => self.shift = !self.shift,
            Cap::Symbols => {
                self.symbols = !self.symbols;
                self.shift = false;
            }
            Cap::Space => self.type_char(' '),
            Cap::Delete => {
                self.text.pop();
            }
            Cap::Show => self.shown = !self.shown,
            Cap::Cancel => return Some(Typed::Cancel),
            Cap::Done => return Some(Typed::Done(self.text.clone())),
        }
        // A key that changed the rows may have moved out from under the highlight.
        self.clamp();
        None
    }

    fn clamp(&mut self) {
        let rows = self.rows();
        self.at.0 = self.at.0.min(rows.len() - 1);
        self.at.1 = self.at.1.min(rows[self.at.0].len() - 1);
    }

    /// The highlight moved up or down a row, to the key nearest the one it was on.
    fn vertical(&mut self, d: i32) {
        let boxes = self.boxes();
        let rows = self.rows().len() as i32;
        let centre = |b: &(f32, f32, f32, f32)| b.0 + b.2 / 2.0;
        let Some(from) = boxes
            .iter()
            .find(|b| (b.0, b.1) == self.at)
            .map(|b| centre(&b.3))
        else {
            return;
        };
        let to = (self.at.0 as i32 + d).rem_euclid(rows) as usize;
        if let Some(b) = boxes.iter().filter(|b| b.0 == to).min_by(|a, b| {
            (centre(&a.3) - from)
                .abs()
                .total_cmp(&(centre(&b.3) - from).abs())
        }) {
            self.at = (b.0, b.1);
        }
    }

    /// A button; `Some` when typing is over.
    pub fn key(&mut self, key: Key) -> Option<Typed> {
        let len = self.rows()[self.at.0].len();
        match key {
            Key::Up => self.vertical(-1),
            Key::Down => self.vertical(1),
            Key::Left => self.at.1 = (self.at.1 + len - 1) % len,
            Key::Right => self.at.1 = (self.at.1 + 1) % len,
            Key::A => return self.cap_at(self.at).and_then(|c| self.press(c)),
            Key::B => return self.press(Cap::Delete),
            Key::X if self.purpose != Purpose::Digits => return self.press(Cap::Space),
            Key::Y if self.purpose == Purpose::Secret => return self.press(Cap::Show),
            Key::L if self.purpose != Purpose::Digits => return self.press(Cap::Shift),
            Key::R if self.purpose != Purpose::Digits => return self.press(Cap::Symbols),
            Key::Start => return self.press(Cap::Done),
            Key::Select => return self.press(Cap::Cancel),
            _ => {}
        }
        None
    }

    /// A tap at `(x, y)` on the screen: types the key under it, and highlights it.
    pub fn touch(&mut self, x: f32, y: f32) -> Option<Typed> {
        let hit = self
            .boxes()
            .into_iter()
            .find(|&(_, _, _, (bx, by, w, h))| {
                (bx - GAP / 2.0..bx + w + GAP / 2.0).contains(&x)
                    && (by - GAP / 2.0..by + h + GAP / 2.0).contains(&y)
            })?;
        self.at = (hit.0, hit.1);
        self.press(hit.2)
    }

    /// What is typed, as shown: dots for a secret not shown.
    pub fn shown_text(&self) -> String {
        if self.purpose == Purpose::Secret && !self.shown {
            "•".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        }
    }

    /// The keyboard laid out on the 1024 × 768 touch screen.
    pub fn draws(&self) -> Vec<Draw> {
        let text =
            |s: &str, size: f32, bold: bool, x: f32, y: f32, anchor: f32, colour: Rgb| Draw::Text {
                text: s.to_owned(),
                size,
                bold,
                x,
                y,
                anchor,
                colour,
            };
        let mut out = vec![Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: W,
            h: H,
            colour: colour::BLACK,
        }];
        out.push(text(&self.title, 26.0, false, 60.0, 34.0, 0.0, colour::DIM));
        // What is typed, in its field, with a caret.
        out.push(Draw::Round {
            x: 50.0,
            y: 78.0,
            w: W - 100.0,
            h: 84.0,
            radius: 12.0,
            colour: colour::PANEL,
        });
        let shown = self.shown_text();
        out.push(text(&shown, 40.0, true, 74.0, 96.0, 0.0, colour::WHITE));
        out.push(text(
            "|",
            40.0,
            false,
            74.0 + 21.0 * shown.chars().count() as f32,
            94.0,
            0.0,
            colour::GOLD,
        ));
        if let Some(note) = &self.note {
            out.push(text(note, 20.0, false, 60.0, 178.0, 0.0, colour::RED));
        } else {
            let hint = match self.purpose {
                Purpose::Digits => "A type   ·   B delete   ·   Start done   ·   Select cancel",
                Purpose::Secret => "A type   ·   B delete   ·   Y show   ·   L shift   ·   R symbols   ·   Start done",
                Purpose::Text => "A type   ·   B delete   ·   X space   ·   L shift   ·   R symbols   ·   Start done",
            };
            out.push(text(hint, 18.0, false, W / 2.0, 182.0, 0.5, colour::FAINT));
        }
        for (r, i, cap, (x, y, w, h)) in self.boxes() {
            let on = (r, i) == self.at;
            let lit = matches!(cap, Cap::Shift) && self.shift
                || matches!(cap, Cap::Symbols) && self.symbols
                || matches!(cap, Cap::Show) && self.shown;
            let fill = match cap {
                Cap::Done => [52, 120, 84],
                _ if on => colour::WHITE,
                _ if lit => [70, 80, 104],
                Cap::Char(_) | Cap::Space => [44, 50, 64],
                _ => colour::PANEL,
            };
            out.push(Draw::Round {
                x,
                y,
                w,
                h,
                radius: 10.0,
                colour: fill,
            });
            if on && cap == Cap::Done {
                out.push(Draw::Frame {
                    x,
                    y,
                    w,
                    h,
                    radius: 10.0,
                    width: 4.0,
                    colour: colour::WHITE,
                });
            }
            let label = match cap {
                Cap::Char(c) => c.to_string(),
                Cap::Shift => {
                    if self.symbols {
                        "more".into()
                    } else {
                        "shift".into()
                    }
                }
                Cap::Symbols => {
                    if self.symbols {
                        "abc".into()
                    } else {
                        "?123".into()
                    }
                }
                Cap::Space => "space".into(),
                Cap::Delete => "⌫".into(),
                Cap::Show => {
                    if self.shown {
                        "hide".into()
                    } else {
                        "show".into()
                    }
                }
                Cap::Cancel => "Cancel".into(),
                Cap::Done => "Done".into(),
            };
            let ink = if on && cap != Cap::Done {
                colour::TAB_INK
            } else {
                colour::WHITE
            };
            let size = if matches!(cap, Cap::Char(_)) {
                38.0
            } else {
                26.0
            };
            out.push(text(
                &label,
                size,
                matches!(cap, Cap::Char(_) | Cap::Done),
                x + w / 2.0,
                y + (h - size * 1.2) / 2.0,
                0.5,
                ink,
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_with_the_buttons() {
        let mut kb = Keyboard::new("Passphrase", Purpose::Secret, 8);
        // From q, right to w, A: "w"; L then A on w: "W" (shift lasts one letter).
        kb.key(Key::Right);
        kb.key(Key::A);
        kb.key(Key::L);
        kb.key(Key::A);
        kb.key(Key::A);
        assert_eq!(kb.text, "wWw");
        assert_eq!(kb.shown_text(), "•••");
        kb.key(Key::Y);
        assert_eq!(kb.shown_text(), "wWw");
        kb.key(Key::B);
        assert_eq!(kb.text, "wW");
        // Up from the letters is the digits; down past the last row comes round.
        kb.key(Key::Up);
        kb.key(Key::A);
        assert_eq!(kb.text, "wW2");
        // No more than `max`.
        for _ in 0..10 {
            kb.key(Key::A);
        }
        assert_eq!(kb.text.chars().count(), 8);
        assert_eq!(kb.key(Key::Select), Some(Typed::Cancel));
        assert!(matches!(kb.key(Key::Start), Some(Typed::Done(_))));
    }

    #[test]
    fn symbols_and_the_pad() {
        let mut kb = Keyboard::new("Search", Purpose::Text, 20);
        kb.key(Key::R);
        kb.key(Key::A);
        assert_eq!(kb.text, "!");
        kb.key(Key::X);
        assert_eq!(kb.text, "! ");

        let mut pin = Keyboard::new("PIN", Purpose::Digits, 6);
        // The pad: 1 2 3 on top; a tap on 5, then Done.
        assert_eq!(pin.touch(W / 2.0, KEYS_TOP + ROW * 1.5), None);
        assert_eq!(pin.text, "5");
        let done = pin
            .boxes()
            .into_iter()
            .find(|b| b.2 == Cap::Done)
            .unwrap()
            .3;
        assert_eq!(
            pin.touch(done.0 + 10.0, done.1 + 10.0),
            Some(Typed::Done("5".into()))
        );
        // Letters do nothing on the pad.
        pin.key(Key::L);
        assert_eq!(pin.rows().len(), 5);
    }

    #[test]
    fn every_key_on_screen() {
        for purpose in [Purpose::Text, Purpose::Secret, Purpose::Digits] {
            let kb = Keyboard::new("t", purpose, 10);
            for (_, _, _, (x, y, w, h)) in kb.boxes() {
                assert!(
                    x >= 0.0 && x + w <= W && y >= 0.0 && y + h <= H - 60.0,
                    "{purpose:?}"
                );
            }
            assert!(!kb.draws().is_empty());
        }
    }
}
