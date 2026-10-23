//! The small pictures a row's value can carry ([`super::Value::Status`]): a Wi-Fi network's
//! signal (the status bar's fan of arcs, the bars it has bright and the rest dim), a lock for a
//! secured network, a check for the one joined. Drawn here, into RGBA, at whatever height the
//! app draws its text, so DSlot's textures and `menu-raster`'s pictures are the same shapes.
//! A device's battery is the menu's battery icon (`Draw::Battery`).

use super::colour::Rgb;

/// A picture on a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    /// A signal, in bars of 4.
    Signal(u8),
    /// Secured: a passphrase.
    Lock,
    /// The one in use (the network joined).
    Check,
    /// A battery's charge, in percent.
    Battery(u8),
    /// Delete (the keyboard's backspace key): a tag pointing left with a cross in it. Dark
    /// (in the dim colour) for a key lit white.
    Backspace(bool),
    /// An arrow key's arrow, which way it points; dark for a key lit white.
    Arrow(Dir, bool),
}

/// Which way an arrow points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
}

impl Glyph {
    /// Its width for a height of `h`.
    pub fn width(self, h: f32) -> f32 {
        h * match self {
            Glyph::Signal(_) => 1.3,
            Glyph::Lock => 0.75,
            Glyph::Check => 0.95,
            Glyph::Battery(_) => 2.1,
            Glyph::Backspace(_) => 1.5,
            Glyph::Arrow(..) => 1.0,
        }
    }
}

/// `glyph` drawn `h` pixels high (Battery is the menu's own icon: none here), `on` its bright
/// parts and `off` its dim ones: its width, and RGBA.
pub fn render(glyph: Glyph, h: usize, on: Rgb, off: Rgb) -> (usize, Vec<u8>) {
    let w = glyph.width(h as f32).ceil() as usize;
    let mut px = vec![0u8; w * h * 4];
    let hf = h as f32;
    // Four samples a pixel, for smooth edges.
    const SUB: [(f32, f32); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
    for y in 0..h {
        for x in 0..w {
            let (mut lit, mut dim) = (0.0f32, 0.0f32);
            for (sx, sy) in SUB {
                // In units of the height, the origin top left.
                let (u, v) = ((x as f32 + sx) / hf, (y as f32 + sy) / hf);
                match shade(glyph, u, v) {
                    Some(true) => lit += 0.25,
                    Some(false) => dim += 0.25,
                    None => {}
                }
            }
            let a = lit + dim;
            if a <= 0.0 {
                continue;
            }
            let c = if lit >= dim { on } else { off };
            let i = (y * w + x) * 4;
            px[i..i + 3].copy_from_slice(&c);
            px[i + 3] = (a.min(1.0) * 255.0) as u8;
        }
    }
    (w, px)
}

/// Whether the point `(u, v)` (in heights, from the top left) is on the glyph: `Some(true)` a
/// bright part, `Some(false)` a dim one.
fn shade(glyph: Glyph, u: f32, v: f32) -> Option<bool> {
    match glyph {
        Glyph::Signal(bars) => {
            // A fan from a dot at the bottom middle: the dot and three arcs, 90 degrees wide.
            let (cx, cy) = (0.65, 0.92);
            let (dx, dy) = (u - cx, cy - v);
            let d = dx.hypot(dy);
            let inside = dy > 0.0 && dx.abs() <= dy * 1.0 + 0.02;
            let band = if d < 0.16 {
                Some(1)
            } else if inside {
                [(0.30, 0.44, 2), (0.57, 0.71, 3), (0.84, 0.98, 4)]
                    .iter()
                    .find(|(a, b, _)| (*a..*b).contains(&d))
                    .map(|&(_, _, k)| k)
            } else {
                None
            };
            band.map(|k| k <= bars)
        }
        Glyph::Lock => {
            // A body, and a shackle above it.
            let body = (0.06..0.69).contains(&u) && (0.45..0.95).contains(&v);
            let (dx, dy) = (u - 0.375, v - 0.45);
            let r = dx.hypot(dy);
            let shackle = dy <= 0.0 && (0.17..0.26).contains(&r)
                || (0.45..0.5).contains(&v)
                    && ((0.11..0.2).contains(&u) || (0.55..0.64).contains(&u));
            (body || shackle).then_some(true)
        }
        Glyph::Check => {
            // Two strokes: down to the bottom left, up to the top right.
            let near = |a: (f32, f32), b: (f32, f32)| {
                let (vx, vy) = (b.0 - a.0, b.1 - a.1);
                let t = (((u - a.0) * vx + (v - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                (u - a.0 - vx * t).hypot(v - a.1 - vy * t)
            };
            let d = near((0.08, 0.52), (0.36, 0.82)).min(near((0.36, 0.82), (0.88, 0.14)));
            (d < 0.075).then_some(true)
        }
        Glyph::Battery(_) => None,
        Glyph::Backspace(dark) => {
            // The tag: a point at the left, a body to the right, drawn as an outline.
            let tag = |inset: f32| {
                let (top, bottom, right) = (0.18 + inset, 0.82 - inset, 1.42 - inset);
                let half = 0.32;
                let off = (v - 0.5).abs();
                let left = 0.08 + inset * 1.4 + (off / half) * (0.45 - 0.08);
                (top..bottom).contains(&v) && u <= right && u >= left
            };
            let outline = tag(0.0) && !tag(0.08);
            // The cross, in the body.
            let (cx, cy, arm) = (0.93, 0.5, 0.15);
            let cross = (u - cx).abs() <= arm
                && (v - cy).abs() <= arm
                && ((u - cx) - (v - cy)).abs().min(((u - cx) + (v - cy)).abs()) < 0.075;
            (outline || cross).then_some(!dark)
        }
        Glyph::Arrow(dir, dark) => {
            // Drawn pointing up, the point turned to face the other ways.
            let (u, v) = match dir {
                Dir::Up => (u, v),
                Dir::Down => (u, 1.0 - v),
                Dir::Left => (1.0 - v, u),
                Dir::Right => (v, 1.0 - u),
            };
            let head = (0.12..=0.52).contains(&v) && (u - 0.5).abs() <= (v - 0.12) * 0.85;
            let stem = (0.42..=0.58).contains(&u) && (0.45..=0.9).contains(&v);
            (head || stem).then_some(!dark)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(rgba: &[u8], w: usize, x: usize, y: usize) -> u8 {
        rgba[(y * w + x) * 4 + 3]
    }

    #[test]
    fn signal_bars_lit_and_dim() {
        let (on, off) = ([255, 255, 255], [80, 80, 80]);
        let h = 40;
        let (w, two) = render(Glyph::Signal(2), h, on, off);
        assert_eq!(w, 52);
        // The outermost arc, straight up from the dot: dim with two bars, bright with four.
        let (x, y) = ((0.65 * 40.0) as usize, ((0.92 - 0.91) * 40.0) as usize + 1);
        assert!(alpha_at(&two, w, x, y) > 0);
        assert_eq!(&two[(y * w + x) * 4..][..3], &off);
        let (_, four) = render(Glyph::Signal(4), h, on, off);
        assert_eq!(&four[(y * w + x) * 4..][..3], &on);
        // The dot is always lit.
        let dot = ((0.92 * 40.0) as usize) * w + 26;
        assert_eq!(&two[dot * 4..][..3], &on);
    }

    #[test]
    fn lock_and_check_have_ink() {
        for g in [
            Glyph::Lock,
            Glyph::Check,
            Glyph::Backspace(false),
            Glyph::Backspace(true),
            Glyph::Arrow(Dir::Up, false),
            Glyph::Arrow(Dir::Left, true),
        ] {
            let (w, px) = render(g, 30, [255; 3], [0; 3]);
            assert!(px.chunks(4).any(|p| p[3] == 255), "{g:?}");
            assert_eq!(w, g.width(30.0).ceil() as usize);
        }
    }
}
