//! A terminal keyboard on the bottom screen: every key a shell wants (Esc, Tab, Ctrl, Alt, the
//! arrows, F1-F12), typed as key presses rather than collected into a string, as
//! [`super::keyboard`] does for a passphrase. What it gives is Linux key codes
//! (`<linux/input-event-codes.h>`), pressed and released, for a uinput keyboard: a key held down
//! stays down, so the kernel repeats it.
//!
//! Shift, Ctrl, Alt and Fn are tapped before the key they change: once for the next key, twice
//! to keep them on, a third time off. Fn turns the number row into F1-F12 and the arrows into
//! Home, End, Page Up and Page Down.
//!
//! The buttons type too: the D-pad the arrows, A Enter, B Backspace, X Tab, Y Ctrl-C, and L and
//! R are Shift and Ctrl while held.
//!
//! ```
//! use nibkit::menu::keys::{Keys, Out};
//! let mut keys = Keys::new();
//! // A finger on the "a" key, then lifted.
//! let (x, y) = keys.centre_of('a').unwrap();
//! assert_eq!(keys.touch_down(x, y), [Out::Press(30)]);
//! assert_eq!(keys.touch_up(), [Out::Release(30)]);
//! ```

use super::colour::{self, Rgb};
use super::glyphs::{Dir, Glyph};
use super::layout::Draw;
use super::{H, W};

/// Linux key codes (`KEY_*`), the ones the keyboard types.
pub mod code {
    #![allow(missing_docs)]
    pub const ESC: u16 = 1;
    pub const MINUS: u16 = 12;
    pub const EQUAL: u16 = 13;
    pub const BACKSPACE: u16 = 14;
    pub const TAB: u16 = 15;
    pub const LEFTBRACE: u16 = 26;
    pub const RIGHTBRACE: u16 = 27;
    pub const ENTER: u16 = 28;
    pub const LEFTCTRL: u16 = 29;
    pub const SEMICOLON: u16 = 39;
    pub const APOSTROPHE: u16 = 40;
    pub const GRAVE: u16 = 41;
    pub const LEFTSHIFT: u16 = 42;
    pub const BACKSLASH: u16 = 43;
    pub const COMMA: u16 = 51;
    pub const DOT: u16 = 52;
    pub const SLASH: u16 = 53;
    pub const LEFTALT: u16 = 56;
    pub const SPACE: u16 = 57;
    pub const F11: u16 = 87;
    pub const F12: u16 = 88;
    pub const HOME: u16 = 102;
    pub const UP: u16 = 103;
    pub const PAGEUP: u16 = 104;
    pub const LEFT: u16 = 105;
    pub const RIGHT: u16 = 106;
    pub const END: u16 = 107;
    pub const DOWN: u16 = 108;
    pub const PAGEDOWN: u16 = 109;
    pub const C: u16 = 46;
    pub const D: u16 = 32;
    pub const L: u16 = 38;
    pub const Z: u16 = 44;

    /// F1-F10, then F11 and F12, which the kernel numbers apart.
    pub fn f(n: u8) -> u16 {
        match n {
            1..=10 => 58 + u16::from(n),
            11 => F11,
            _ => F12,
        }
    }

    /// Every code the keyboard can send, for setting up the uinput device.
    pub fn all() -> Vec<u16> {
        let mut all: Vec<u16> = (1..=88).collect();
        all.extend(HOME..=PAGEDOWN);
        all
    }
}

/// The keys' codes in rows, as on a PC: the number row (2-13), the letter rows.
const NUMBERS: [(char, char, u16); 12] = [
    ('1', '!', 2),
    ('2', '@', 3),
    ('3', '#', 4),
    ('4', '$', 5),
    ('5', '%', 6),
    ('6', '^', 7),
    ('7', '&', 8),
    ('8', '*', 9),
    ('9', '(', 10),
    ('0', ')', 11),
    ('-', '_', code::MINUS),
    ('=', '+', code::EQUAL),
];
const TOP: [(char, char, u16); 12] = [
    ('q', 'Q', 16),
    ('w', 'W', 17),
    ('e', 'E', 18),
    ('r', 'R', 19),
    ('t', 'T', 20),
    ('y', 'Y', 21),
    ('u', 'U', 22),
    ('i', 'I', 23),
    ('o', 'O', 24),
    ('p', 'P', 25),
    ('[', '{', code::LEFTBRACE),
    (']', '}', code::RIGHTBRACE),
];
const HOME_ROW: [(char, char, u16); 11] = [
    ('a', 'A', 30),
    ('s', 'S', 31),
    ('d', 'D', 32),
    ('f', 'F', 33),
    ('g', 'G', 34),
    ('h', 'H', 35),
    ('j', 'J', 36),
    ('k', 'K', 37),
    ('l', 'L', 38),
    (';', ':', code::SEMICOLON),
    ('\'', '"', code::APOSTROPHE),
];
const BOTTOM: [(char, char, u16); 10] = [
    ('z', 'Z', 44),
    ('x', 'X', 45),
    ('c', 'C', 46),
    ('v', 'V', 47),
    ('b', 'B', 48),
    ('n', 'N', 49),
    ('m', 'M', 50),
    (',', '<', code::COMMA),
    ('.', '>', code::DOT),
    ('/', '?', code::SLASH),
];

/// A modifier: off, on for the next key, or kept on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mod {
    /// Off.
    #[default]
    Off,
    /// On for the next key.
    Once,
    /// On until tapped again.
    Locked,
}

impl Mod {
    fn on(self) -> bool {
        self != Mod::Off
    }

    fn tap(self) -> Mod {
        match self {
            Mod::Off => Mod::Once,
            Mod::Once => Mod::Locked,
            Mod::Locked => Mod::Off,
        }
    }

    fn used(self) -> Mod {
        if self == Mod::Once {
            Mod::Off
        } else {
            self
        }
    }
}

/// What a key on the screen is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cap {
    /// A character key: unshifted and shifted, and its code.
    Char(char, char, u16),
    /// A key with a name (Esc, Tab, Enter, the arrows...): its label and code.
    Named(&'static str, u16),
    /// A shortcut: Ctrl and a letter (Ctrl-C).
    Ctrl(&'static str, u16),
    /// A number-row key that is an F key with Fn: its digit, its shifted sign, its code, its F.
    Number(char, char, u16, u8),
    /// An arrow, which Fn makes Home, End, Page Up or Page Down: its way and both codes.
    Arrow(Dir, u16, u16),
    Backspace,
    Shift,
    Ctrl_,
    Alt,
    Fn,
    Hide,
}

/// What pressing keys comes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// A key goes down (a Linux key code).
    Press(u16),
    /// A key comes up.
    Release(u16),
    /// The keyboard is to be hidden.
    Hide,
}

/// A button, by what is printed on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// The D-pad.
    Up,
    /// The D-pad.
    Down,
    /// The D-pad.
    Left,
    /// The D-pad.
    Right,
    /// Enter.
    A,
    /// Backspace.
    B,
    /// Tab.
    X,
    /// Ctrl-C.
    Y,
    /// Shift, while held.
    L,
    /// Ctrl, while held.
    R,
}

/// The keyboard's place on the screen.
const HINT_H: f32 = 44.0;
const ROWS: usize = 6;
const UNITS: f32 = 15.0;
const GAP: f32 = 6.0;
const SIDE: f32 = 6.0;

/// The keyboard and its state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    shift: Mod,
    ctrl: Mod,
    alt: Mod,
    func: Mod,
    /// The key under the finger, and the codes it pressed (to release when the finger lifts).
    down: Option<((usize, usize), Vec<u16>)>,
    /// What each held button pressed.
    held: Vec<(Button, Vec<u16>)>,
}

impl Keys {
    /// The keyboard, nothing held.
    pub fn new() -> Keys {
        Keys::default()
    }

    /// The rows, each key with its width in units (15 to a row).
    fn rows(&self) -> [Vec<(Cap, f32)>; ROWS] {
        let chars = |keys: &[(char, char, u16)]| -> Vec<(Cap, f32)> {
            keys.iter()
                .map(|&(c, s, k)| (Cap::Char(c, s, k), 1.0))
                .collect()
        };
        let mut numbers = vec![(Cap::Char('`', '~', code::GRAVE), 1.0)];
        numbers.extend(
            NUMBERS
                .iter()
                .enumerate()
                .map(|(i, &(c, s, k))| (Cap::Number(c, s, k, i as u8 + 1), 1.0)),
        );
        numbers.push((Cap::Backspace, 2.0));
        let mut top = vec![(Cap::Named("Tab", code::TAB), 1.5)];
        top.extend(chars(&TOP));
        top.push((Cap::Char('\\', '|', code::BACKSLASH), 1.5));
        let mut home = vec![(Cap::Ctrl_, 1.75)];
        home.extend(chars(&HOME_ROW));
        home.push((Cap::Named("Enter", code::ENTER), 2.25));
        let mut bottom = vec![(Cap::Shift, 2.25)];
        bottom.extend(chars(&BOTTOM));
        bottom.push((Cap::Arrow(Dir::Up, code::UP, code::PAGEUP), 1.0));
        bottom.push((Cap::Shift, 1.75));
        [
            vec![
                (Cap::Named("Esc", code::ESC), 1.5),
                (Cap::Ctrl("Ctrl-C", code::C), 2.0),
                (Cap::Ctrl("Ctrl-D", code::D), 2.0),
                (Cap::Ctrl("Ctrl-Z", code::Z), 2.0),
                (Cap::Ctrl("Ctrl-L", code::L), 2.0),
                (Cap::Named("Home", code::HOME), 1.5),
                (Cap::Named("End", code::END), 1.5),
                (Cap::Named("PgUp", code::PAGEUP), 1.25),
                (Cap::Named("PgDn", code::PAGEDOWN), 1.25),
            ],
            numbers,
            top,
            home,
            bottom,
            vec![
                (Cap::Fn, 1.5),
                (Cap::Alt, 1.5),
                (Cap::Named("space", code::SPACE), 6.0),
                (Cap::Arrow(Dir::Left, code::LEFT, code::HOME), 1.0),
                (Cap::Arrow(Dir::Down, code::DOWN, code::PAGEDOWN), 1.0),
                (Cap::Arrow(Dir::Right, code::RIGHT, code::END), 1.0),
                (Cap::Hide, 3.0),
            ],
        ]
    }

    /// Each key's place: its row, its place in the row, what it is, and its box.
    fn boxes(&self) -> Vec<(usize, usize, Cap, (f32, f32, f32, f32))> {
        let unit = (W - 2.0 * SIDE) / UNITS;
        let row_h = (H - HINT_H - SIDE) / ROWS as f32;
        let mut out = Vec::new();
        for (r, row) in self.rows().iter().enumerate() {
            let mut x = SIDE;
            let y = HINT_H + r as f32 * row_h;
            for (i, &(cap, units)) in row.iter().enumerate() {
                let w = units * unit;
                out.push((
                    r,
                    i,
                    cap,
                    (x + GAP / 2.0, y + GAP / 2.0, w - GAP, row_h - GAP),
                ));
                x += w;
            }
        }
        out
    }

    /// The middle of the key that types `c` unshifted, for tests and examples.
    pub fn centre_of(&self, c: char) -> Option<(f32, f32)> {
        self.boxes()
            .into_iter()
            .find_map(|(_, _, cap, (x, y, w, h))| {
                let theirs = match cap {
                    Cap::Char(k, ..) | Cap::Number(k, ..) => k,
                    _ => return None,
                };
                (theirs == c).then_some((x + w / 2.0, y + h / 2.0))
            })
    }

    /// The modifiers on now, as the codes to press before a key.
    fn mods(&self) -> Vec<u16> {
        [
            (self.ctrl, code::LEFTCTRL),
            (self.alt, code::LEFTALT),
            (self.shift, code::LEFTSHIFT),
        ]
        .iter()
        .filter(|(m, _)| m.on())
        .map(|&(_, k)| k)
        .collect()
    }

    /// The codes a key presses, modifiers first; `None` for a modifier or Hide.
    fn codes(&self, cap: Cap) -> Option<Vec<u16>> {
        let fnk = self.func.on();
        let key = match cap {
            Cap::Char(_, _, k) | Cap::Named(_, k) => k,
            Cap::Number(_, _, _, f) if fnk => code::f(f),
            Cap::Number(_, _, k, _) => k,
            Cap::Arrow(_, _, alt) if fnk => alt,
            Cap::Arrow(_, k, _) => k,
            Cap::Backspace => code::BACKSPACE,
            Cap::Ctrl(_, k) => {
                let mut with = vec![code::LEFTCTRL];
                with.extend(self.mods().into_iter().filter(|&m| m != code::LEFTCTRL));
                with.push(k);
                return Some(with);
            }
            Cap::Shift | Cap::Ctrl_ | Cap::Alt | Cap::Fn | Cap::Hide => return None,
        };
        let mut codes = self.mods();
        codes.push(key);
        Some(codes)
    }

    /// The modifiers tapped once go off, now a key has used them.
    fn used(&mut self) {
        self.shift = self.shift.used();
        self.ctrl = self.ctrl.used();
        self.alt = self.alt.used();
        self.func = self.func.used();
    }

    /// A finger comes down at `(x, y)` (screen units): the key under it goes down.
    pub fn touch_down(&mut self, x: f32, y: f32) -> Vec<Out> {
        let mut out = self.touch_up();
        let Some((r, i, cap, _)) = self.boxes().into_iter().find(|&(_, _, _, (bx, by, w, h))| {
            (bx - GAP / 2.0..bx + w + GAP / 2.0).contains(&x)
                && (by - GAP / 2.0..by + h + GAP / 2.0).contains(&y)
        }) else {
            return out;
        };
        match cap {
            Cap::Shift => self.shift = self.shift.tap(),
            Cap::Ctrl_ => self.ctrl = self.ctrl.tap(),
            Cap::Alt => self.alt = self.alt.tap(),
            Cap::Fn => self.func = self.func.tap(),
            Cap::Hide => out.push(Out::Hide),
            _ => {}
        }
        let codes = self.codes(cap).unwrap_or_default();
        out.extend(codes.iter().map(|&k| Out::Press(k)));
        self.down = Some(((r, i), codes));
        out
    }

    /// The finger lifts: what it pressed comes up, in the opposite order.
    pub fn touch_up(&mut self) -> Vec<Out> {
        let Some((_, codes)) = self.down.take() else {
            return Vec::new();
        };
        if !codes.is_empty() {
            self.used();
        }
        codes.iter().rev().map(|&k| Out::Release(k)).collect()
    }

    /// A button goes down or up.
    pub fn button(&mut self, button: Button, down: bool) -> Vec<Out> {
        if !down {
            let Some(at) = self.held.iter().position(|(b, _)| *b == button) else {
                return Vec::new();
            };
            let (_, codes) = self.held.remove(at);
            return codes.iter().rev().map(|&k| Out::Release(k)).collect();
        }
        let key = match button {
            Button::Up => code::UP,
            Button::Down => code::DOWN,
            Button::Left => code::LEFT,
            Button::Right => code::RIGHT,
            Button::A => code::ENTER,
            Button::B => code::BACKSPACE,
            Button::X => code::TAB,
            Button::Y => code::C,
            Button::L => code::LEFTSHIFT,
            Button::R => code::LEFTCTRL,
        };
        let mut codes = if button == Button::Y {
            vec![code::LEFTCTRL]
        } else {
            Vec::new()
        };
        codes.push(key);
        let out = codes.iter().map(|&k| Out::Press(k)).collect();
        self.held.push((button, codes));
        out
    }

    /// Everything held comes up (the keyboard hidden or going away).
    pub fn release_all(&mut self) -> Vec<Out> {
        let mut out = self.touch_up();
        for (_, codes) in self.held.drain(..) {
            out.extend(codes.iter().rev().map(|&k| Out::Release(k)));
        }
        out
    }

    /// A key's label, as the modifiers have it now; `None` for Backspace and the arrows, which
    /// are drawn (the font has neither sign).
    fn label(&self, cap: Cap) -> Option<String> {
        let shift = self.shift.on();
        Some(match cap {
            Cap::Char(c, s, _) => (if shift { s } else { c }).to_string(),
            Cap::Number(_, _, _, f) if self.func.on() => format!("F{f}"),
            Cap::Number(c, s, _, _) => (if shift { s } else { c }).to_string(),
            Cap::Arrow(_, _, alt) if self.func.on() => match alt {
                code::HOME => "Home",
                code::END => "End",
                code::PAGEUP => "PgUp",
                _ => "PgDn",
            }
            .into(),
            Cap::Named(l, _) | Cap::Ctrl(l, _) => l.into(),
            Cap::Backspace | Cap::Arrow(..) => return None,
            Cap::Shift => "shift".into(),
            Cap::Ctrl_ => "Ctrl".into(),
            Cap::Alt => "Alt".into(),
            Cap::Fn => "Fn".into(),
            Cap::Hide => "hide".into(),
        })
    }

    /// Whether a modifier key is on, and kept on.
    fn lit(&self, cap: Cap) -> (bool, bool) {
        let m = match cap {
            Cap::Shift => self.shift,
            Cap::Ctrl_ => self.ctrl,
            Cap::Alt => self.alt,
            Cap::Fn => self.func,
            _ => Mod::Off,
        };
        (m.on(), m == Mod::Locked)
    }

    /// The keyboard laid out on the 1024 × 768 touch screen.
    pub fn draws(&self) -> Vec<Draw> {
        let text = |s: &str, size: f32, bold: bool, x: f32, y: f32, colour: Rgb| Draw::Text {
            text: s.to_owned(),
            size,
            bold,
            x,
            y,
            anchor: 0.5,
            colour,
        };
        let mut out = vec![Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: W,
            h: H,
            colour: colour::BLACK,
        }];
        out.push(text(
            "Menu + X hide   \u{b7}   D-pad arrows   \u{b7}   A Enter   \u{b7}   B Backspace   \u{b7}   X Tab   \u{b7}   Y Ctrl-C   \u{b7}   L shift   \u{b7}   R Ctrl",
            17.0,
            false,
            W / 2.0,
            12.0,
            colour::FAINT,
        ));
        let pressed = self.down.as_ref().map(|(at, _)| *at);
        for (r, i, cap, (x, y, w, h)) in self.boxes() {
            let on = pressed == Some((r, i));
            let (lit, locked) = self.lit(cap);
            let fill = match cap {
                _ if on => colour::WHITE,
                _ if lit => [70, 80, 104],
                Cap::Char(..) | Cap::Number(..) | Cap::Named("space", _) => [44, 50, 64],
                Cap::Hide => [36, 40, 52],
                _ => colour::PANEL,
            };
            out.push(Draw::Round {
                x,
                y,
                w,
                h,
                radius: 9.0,
                colour: fill,
            });
            if locked {
                out.push(Draw::Frame {
                    x,
                    y,
                    w,
                    h,
                    radius: 9.0,
                    width: 3.0,
                    colour: colour::GOLD,
                });
            }
            let ink = if on { colour::TAB_INK } else { colour::WHITE };
            let Some(label) = self.label(cap) else {
                let g = 28.0;
                let glyph = match cap {
                    Cap::Arrow(dir, ..) => Glyph::Arrow(dir, on),
                    _ => Glyph::Backspace(on),
                };
                out.push(Draw::Glyph {
                    glyph,
                    x: x + (w - glyph.width(g)) / 2.0,
                    y: y + (h - g) / 2.0,
                    h: g,
                });
                continue;
            };
            let character = matches!(cap, Cap::Char(..) | Cap::Number(..) | Cap::Arrow(..))
                && label.chars().count() == 1;
            let size = if character { 34.0 } else { 22.0 };
            out.push(text(
                &label,
                size,
                character,
                x + w / 2.0,
                y + (h - size * 1.2) / 2.0,
                ink,
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(keys: &Keys, cap: Cap) -> (f32, f32) {
        let (_, _, _, (x, y, w, h)) = keys
            .boxes()
            .into_iter()
            .find(|b| b.2 == cap)
            .unwrap_or_else(|| panic!("no {cap:?}"));
        (x + w / 2.0, y + h / 2.0)
    }

    fn tap(keys: &mut Keys, (x, y): (f32, f32)) -> Vec<Out> {
        let mut out = keys.touch_down(x, y);
        out.extend(keys.touch_up());
        out
    }

    #[test]
    fn rows_fill_the_screen() {
        let keys = Keys::new();
        for row in keys.rows() {
            let units: f32 = row.iter().map(|(_, u)| u).sum();
            assert!((units - UNITS).abs() < 0.01, "{units}");
        }
        for (_, _, cap, (x, y, w, h)) in keys.boxes() {
            assert!(
                x >= 0.0 && y >= HINT_H && x + w <= W && y + h <= H,
                "{cap:?}"
            );
        }
    }

    #[test]
    fn modifiers_once_and_kept() {
        let mut keys = Keys::new();
        let a = keys.centre_of('a').unwrap();
        // Shift once: the next key only.
        let p = at(&keys, Cap::Shift);
        tap(&mut keys, p);
        assert_eq!(
            tap(&mut keys, a),
            [
                Out::Press(code::LEFTSHIFT),
                Out::Press(30),
                Out::Release(30),
                Out::Release(code::LEFTSHIFT)
            ]
        );
        assert_eq!(tap(&mut keys, a), [Out::Press(30), Out::Release(30)]);
        // Ctrl twice: kept for every key until tapped again.
        let ctrl = at(&keys, Cap::Ctrl_);
        tap(&mut keys, ctrl);
        tap(&mut keys, ctrl);
        for _ in 0..2 {
            assert_eq!(tap(&mut keys, a)[0], Out::Press(code::LEFTCTRL));
        }
        tap(&mut keys, ctrl);
        assert_eq!(tap(&mut keys, a), [Out::Press(30), Out::Release(30)]);
    }

    #[test]
    fn fn_layer_and_shortcuts() {
        let mut keys = Keys::new();
        let p = at(&keys, Cap::Fn);
        tap(&mut keys, p);
        let one = keys.centre_of('1').unwrap();
        assert_eq!(tap(&mut keys, one), [Out::Press(59), Out::Release(59)]);
        // Fn was once: 1 again.
        assert_eq!(tap(&mut keys, one), [Out::Press(2), Out::Release(2)]);
        let ctrl_c = at(&keys, Cap::Ctrl("Ctrl-C", code::C));
        assert_eq!(
            tap(&mut keys, ctrl_c),
            [
                Out::Press(code::LEFTCTRL),
                Out::Press(code::C),
                Out::Release(code::C),
                Out::Release(code::LEFTCTRL)
            ]
        );
        let hide = at(&keys, Cap::Hide);
        assert_eq!(tap(&mut keys, hide), [Out::Hide]);
    }

    /// A key held under the finger stays down until the finger lifts (the kernel repeats it).
    #[test]
    fn held_until_lifted() {
        let mut keys = Keys::new();
        let (x, y) = at(&keys, Cap::Backspace);
        assert_eq!(keys.touch_down(x, y), [Out::Press(code::BACKSPACE)]);
        assert!(keys.draws().iter().any(|d| matches!(
            d,
            Draw::Glyph {
                glyph: Glyph::Backspace(true),
                ..
            }
        )));
        assert_eq!(keys.touch_up(), [Out::Release(code::BACKSPACE)]);
        assert!(keys.touch_up().is_empty());
    }

    #[test]
    fn buttons() {
        let mut keys = Keys::new();
        assert_eq!(keys.button(Button::Up, true), [Out::Press(code::UP)]);
        assert_eq!(
            keys.button(Button::Y, true),
            [Out::Press(code::LEFTCTRL), Out::Press(code::C)]
        );
        assert_eq!(
            keys.release_all(),
            [
                Out::Release(code::UP),
                Out::Release(code::C),
                Out::Release(code::LEFTCTRL)
            ]
        );
        assert!(keys.button(Button::A, false).is_empty());
        assert_eq!(keys.button(Button::A, true), [Out::Press(code::ENTER)]);
        assert_eq!(keys.button(Button::A, false), [Out::Release(code::ENTER)]);
    }

    /// Every label, in every state, is in Roboto Condensed: no blank keys.
    #[cfg(feature = "menu-raster")]
    #[test]
    fn every_label_in_the_font() {
        let fonts = super::super::raster::Fonts::new();
        let mut states = vec![Keys::new()];
        let mut shifted = Keys::new();
        shifted.shift = Mod::Once;
        states.push(shifted);
        let mut func = Keys::new();
        func.func = Mod::Locked;
        states.push(func);
        for keys in states {
            for d in keys.draws() {
                if let Draw::Text { text, bold, .. } = d {
                    for c in text.chars() {
                        assert!(fonts.has(c, bold), "{c:?} in {text:?}");
                    }
                }
            }
        }
    }

    /// The keyboard drawn, as raw RGBA to `$NIBKIT_RENDERS/keys.rgba` (1024 x 768) when set.
    #[cfg(feature = "menu-raster")]
    #[test]
    fn render() {
        let Some(dir) = std::env::var_os("NIBKIT_RENDERS") else {
            return;
        };
        let fonts = super::super::raster::Fonts::new();
        let mut keys = Keys::new();
        let ctrl = at(&keys, Cap::Ctrl_);
        tap(&mut keys, ctrl);
        let (x, y) = keys.centre_of('l').unwrap();
        keys.touch_down(x, y);
        let mut canvas = super::super::raster::Canvas::new(1.0);
        canvas.draw(&fonts, &keys.draws(), |_| None);
        std::fs::write(std::path::Path::new(&dir).join("keys.rgba"), &canvas.px).unwrap();
    }
}
