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
use super::glyphs::Glyph;
use super::layout::Draw;
use super::{H, W};

/// Where typed text starts in its field, and the furthest its caret goes, in screen units.
const FIELD_LEFT: f32 = 74.0;
const FIELD_RIGHT: f32 = W - 74.0 - 16.0;

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

    /// A key's label, in letters every font has; none for Delete, which is drawn
    /// ([`Glyph::Backspace`]).
    fn label(&self, cap: Cap) -> Option<String> {
        let pick = |on: bool, yes: &str, no: &str| if on { yes } else { no }.to_owned();
        Some(match cap {
            Cap::Char(c) => c.to_string(),
            Cap::Shift => pick(self.symbols, "more", "shift"),
            Cap::Symbols => pick(self.symbols, "abc", "?123"),
            Cap::Space => "space".into(),
            Cap::Delete => return None,
            Cap::Show => pick(self.shown, "hide", "show"),
            Cap::Cancel => "Cancel".into(),
            Cap::Done => "Done".into(),
        })
    }

    /// What the field shows: [`Keyboard::shown_text`], less as much of its start as it takes
    /// for the rest and the caret to fit in the field.
    fn field_text(&self, width: &impl Fn(&str, f32, bool) -> f32) -> String {
        let shown = self.shown_text();
        let room = FIELD_RIGHT - FIELD_LEFT;
        let from = shown
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| width(&shown[i..], 40.0, true) <= room)
            .unwrap_or(shown.len());
        shown[from..].to_owned()
    }

    /// The keyboard laid out on the 1024 × 768 touch screen. The caret goes after an estimate
    /// of the typed text's width; [`Keyboard::draws_measured`] places it exactly.
    pub fn draws(&self) -> Vec<Draw> {
        self.draws_measured(|s, size, _| s.chars().count() as f32 * size * 0.45)
    }

    /// [`Keyboard::draws`], with `width(text, size, bold)` saying how wide text is drawn, in
    /// screen units: the caret goes just after what is shown (the dots of a hidden secret, or
    /// its letters when shown), and text too long for the field shows its end.
    pub fn draws_measured(&self, width: impl Fn(&str, f32, bool) -> f32) -> Vec<Draw> {
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
        let (left, size) = (FIELD_LEFT, 40.0);
        let shown = self.field_text(&width);
        out.push(text(&shown, size, true, left, 96.0, 0.0, colour::WHITE));
        out.push(text(
            "|",
            size,
            false,
            left + width(&shown, size, true),
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
            // Delete is drawn, not lettered: no font is sure to have a backspace sign.
            let Some(label) = self.label(cap) else {
                let g = 30.0;
                let glyph = Glyph::Backspace(on);
                out.push(Draw::Glyph {
                    glyph,
                    x: x + (w - glyph.width(g)) / 2.0,
                    y: y + (h - g) / 2.0,
                    h: g,
                });
                continue;
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

    /// The caret sits just after what the field draws, whatever is typed: dots for a hidden
    /// secret, its letters once shown, plain text, and the end of text longer than the field.
    #[test]
    fn caret_after_what_is_drawn() {
        // Each character as wide as its own: a dot narrower than any letter, as in the font.
        let width = |s: &str, size: f32, _: bool| {
            s.chars()
                .map(|c| match c {
                    '•' => 0.3,
                    'W' | 'M' => 0.8,
                    c if c.is_ascii_digit() => 0.5,
                    _ => 0.45,
                })
                .sum::<f32>()
                * size
        };
        let typed_and_caret = |kb: &Keyboard| {
            let draws = kb.draws_measured(width);
            let texts: Vec<(String, f32, f32)> = draws
                .iter()
                .filter_map(|d| match d {
                    Draw::Text { text, x, size, .. } => Some((text.clone(), *x, *size)),
                    _ => None,
                })
                .collect();
            let caret = texts.iter().position(|(t, ..)| t == "|").unwrap();
            (texts[caret - 1].clone(), texts[caret].1)
        };
        for purpose in [Purpose::Secret, Purpose::Text] {
            for show in [false, true] {
                let mut kb = Keyboard::new("Passphrase", purpose, 63);
                if show {
                    kb.key(Key::Y);
                }
                for n in 0..=40 {
                    let ((typed, x, size), caret) = typed_and_caret(&kb);
                    assert_eq!(caret, x + width(&typed, size, true), "{purpose:?} {n}");
                    assert!(caret <= FIELD_RIGHT, "{purpose:?} {n}: past the field");
                    if purpose == Purpose::Secret && !show {
                        assert!(typed.chars().all(|c| c == '•'));
                    }
                    kb.type_char(if n % 3 == 0 { 'W' } else { 'a' });
                }
                // Deleting brings the caret back with the text.
                for _ in 0..25 {
                    kb.key(Key::B);
                    let ((typed, x, size), caret) = typed_and_caret(&kb);
                    assert_eq!(caret, x + width(&typed, size, true));
                }
            }
        }
    }

    /// Every layout has a Delete key, drawn rather than lettered, and every key's label is
    /// in Roboto Condensed (no blank keys). Tapping Delete deletes.
    #[cfg(feature = "menu-raster")]
    #[test]
    fn every_key_visible() {
        let fonts = super::super::raster::Fonts::new();
        for purpose in [Purpose::Text, Purpose::Secret, Purpose::Digits] {
            for (shift, symbols) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut kb = Keyboard::new("Passphrase", purpose, 63);
                kb.shift = shift;
                kb.symbols = symbols;
                let draws = kb.draws();
                let deletes = draws
                    .iter()
                    .filter(|d| {
                        matches!(
                            d,
                            Draw::Glyph {
                                glyph: Glyph::Backspace(_),
                                ..
                            }
                        )
                    })
                    .count();
                assert_eq!(deletes, 1, "{purpose:?} shift {shift} symbols {symbols}");
                for d in &draws {
                    if let Draw::Text { text, bold, .. } = d {
                        for c in text.chars() {
                            assert!(
                                fonts.has(c, *bold),
                                "{purpose:?}: {c:?} in {text:?} isn't in the font"
                            );
                        }
                    }
                }
                // A tap on Delete takes the last character away.
                kb.type_char('a');
                kb.type_char('b');
                let (_, _, _, (x, y, w, h)) =
                    kb.boxes().into_iter().find(|b| b.2 == Cap::Delete).unwrap();
                kb.touch(x + w / 2.0, y + h / 2.0);
                assert_eq!(kb.shown_text().chars().count(), 1, "{purpose:?}");
            }
        }
    }

    /// The passphrase screen with a few dots typed, written as raw RGBA to
    /// `$NIBKIT_RENDERS/keyboard-<name>.rgba` (1024 x 768) when that's set, to look at.
    #[cfg(feature = "menu-raster")]
    #[test]
    fn render_passphrase() {
        let Some(dir) = std::env::var_os("NIBKIT_RENDERS") else {
            return;
        };
        let fonts = super::super::raster::Fonts::new();
        let mut kb = Keyboard::new("Passphrase for Home", Purpose::Secret, 63);
        for c in "hunter2".chars() {
            kb.type_char(c);
        }
        let shot = |kb: &Keyboard, name: &str| {
            let mut canvas = super::super::raster::Canvas::new(1.0);
            let draws = kb.draws_measured(|s, size, bold| fonts.width(s, size, bold));
            canvas.draw(&fonts, &draws, |_| None);
            let path = std::path::Path::new(&dir).join(format!("keyboard-{name}.rgba"));
            std::fs::write(path, &canvas.px).unwrap();
        };
        shot(&kb, "secret");
        // With Delete highlighted (from the buttons), and with the passphrase shown.
        let at = kb.boxes().into_iter().find(|b| b.2 == Cap::Delete).unwrap();
        kb.at = (at.0, at.1);
        shot(&kb, "delete-lit");
        kb.key(Key::Y);
        shot(&kb, "shown");
    }

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
