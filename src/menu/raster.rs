//! Drawing the menu into an RGBA picture, for an app with no text drawing of its own. Text is
//! Roboto Condensed (SIL OFL 1.1, `assets/OFL.txt`; the face DSlot embeds), laid out exactly as DSlot renders a
//! line; the battery and the bar are DSlot's, drawn the same way (gfx.rs, ui/draw.rs there).

use super::colour::{self, Rgb};
use super::layout::{Draw, BAR_H, ICON};
use super::model::Slot;
use super::{H, W};
use fontdue::{Font, FontSettings};

/// A picture the size of a screen (1024 × 768, or scaled by `scale`), RGBA, rows top first.
pub struct Canvas {
    /// Its width, in pixels.
    pub w: usize,
    /// Its height, in pixels.
    pub h: usize,
    /// The pixels: 4 bytes each, red, green, blue and alpha.
    pub px: Vec<u8>,
    /// Screen units to pixels.
    pub scale: f32,
}

/// The two faces.
pub struct Fonts {
    regular: Font,
    bold: Font,
}

impl Default for Fonts {
    fn default() -> Fonts {
        Fonts::new()
    }
}

impl Fonts {
    /// Loads both faces, which are built in.
    pub fn new() -> Fonts {
        let load = |b: &[u8]| Font::from_bytes(b, FontSettings::default()).expect("embedded font");
        Fonts {
            regular: load(include_bytes!("../../assets/RobotoCondensed-Regular.ttf")),
            bold: load(include_bytes!("../../assets/RobotoCondensed-Bold.ttf")),
        }
    }

    fn face(&self, bold: bool) -> &Font {
        if bold {
            &self.bold
        } else {
            &self.regular
        }
    }

    /// Whether the face has a glyph for `c` (not the blank box a missing one draws).
    pub fn has(&self, c: char, bold: bool) -> bool {
        self.face(bold).lookup_glyph_index(c) != 0
    }

    /// A line's width at `size` pixels.
    pub fn width(&self, s: &str, size: f32, bold: bool) -> f32 {
        let f = self.face(bold);
        s.chars().map(|c| f.metrics(c, size).advance_width).sum()
    }
}

impl Canvas {
    /// A black picture of the screen, `scale` pixels to a unit.
    pub fn new(scale: f32) -> Canvas {
        let (w, h) = ((W * scale) as usize, (H * scale) as usize);
        let mut px = vec![0; w * h * 4];
        for p in px.chunks_exact_mut(4) {
            p[3] = 255;
        }
        Canvas { w, h, px, scale }
    }

    /// Fills the whole picture with `c`.
    pub fn clear(&mut self, c: Rgb) {
        for p in self.px.chunks_exact_mut(4) {
            p.copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }

    fn blend(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        let a = a.min(1.0);
        for (d, &s) in self.px[i..i + 3].iter_mut().zip(&c) {
            *d = (*d as f32 + (s as f32 - *d as f32) * a).round() as u8;
        }
    }

    fn fill(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb) {
        let s = self.scale;
        let (x0, y0) = ((x * s).round() as i32, (y * s).round() as i32);
        let (x1, y1) = (((x + w) * s).round() as i32, ((y + h) * s).round() as i32);
        for py in y0.max(0)..y1.min(self.h as i32) {
            for px in x0.max(0)..x1.min(self.w as i32) {
                self.blend(px, py, c, 1.0);
            }
        }
    }

    /// Covers each pixel of a box by how much of it `inside` says is in the shape (4 × 4
    /// samples), in `c`.
    fn shape(
        &mut self,
        (x, y, w, h): (f32, f32, f32, f32),
        c: Rgb,
        inside: impl Fn(f32, f32) -> bool,
    ) {
        let s = self.scale;
        for py in (y * s).floor() as i32..((y + h) * s).ceil() as i32 {
            for px in (x * s).floor() as i32..((x + w) * s).ceil() as i32 {
                let mut n = 0;
                for k in 0..16 {
                    let sx = (px as f32 + ((k % 4) as f32 + 0.5) / 4.0) / s;
                    let sy = (py as f32 + ((k / 4) as f32 + 0.5) / 4.0) / s;
                    n += inside(sx, sy) as u32;
                }
                self.blend(px, py, c, n as f32 / 16.0);
            }
        }
    }

    /// One line, as DSlot renders it: the line box is the face's ascent less its descent, plus
    /// a pixel round it; `y` is its top and `anchor` where `x` falls along its width.
    pub fn text(
        &mut self,
        f: &Fonts,
        s: &str,
        (size, bold): (f32, bool),
        (x, y): (f32, f32),
        anchor: f32,
        c: Rgb,
    ) {
        let px = (size * self.scale).round();
        let face = f.face(bold);
        let asc = face
            .horizontal_line_metrics(px)
            .map_or(px * 0.9, |m| m.ascent);
        let w = f.width(s, px, bold).ceil() + 2.0;
        let ox = (x * self.scale - anchor * w).round() as i32;
        let oy = (y * self.scale).round() as i32;
        let mut pen = 1.0f32;
        for ch in s.chars() {
            let (m, bitmap) = face.rasterize(ch, px);
            let gx = (pen + m.xmin as f32).round() as i32;
            let gy = (asc + 1.0).round() as i32 - m.height as i32 - m.ymin;
            for by in 0..m.height {
                for bx in 0..m.width {
                    let a = bitmap[by * m.width + bx];
                    if a > 0 {
                        self.blend(
                            ox + gx + bx as i32,
                            oy + gy + by as i32,
                            c,
                            a as f32 / 255.0,
                        );
                    }
                }
            }
            pen += m.advance_width;
        }
    }

    /// Draws the list; `picture(index)` gives an app's picture (RGBA, its width and height).
    pub fn draw<'p>(
        &mut self,
        f: &Fonts,
        draws: &[Draw],
        picture: impl Fn(usize) -> Option<(&'p [u8], usize, usize)>,
    ) {
        for d in draws {
            match d {
                Draw::Rect { x, y, w, h, colour } => self.fill(*x, *y, *w, *h, *colour),
                Draw::Round {
                    x,
                    y,
                    w,
                    h,
                    radius,
                    colour,
                } => {
                    let (x, y, w, h, r) = (*x, *y, *w, *h, *radius);
                    self.shape((x, y, w, h), *colour, |px, py| {
                        rounded(px, py, (x, y, w, h), r)
                    });
                }
                Draw::Frame {
                    x,
                    y,
                    w,
                    h,
                    radius,
                    width,
                    colour,
                } => {
                    let (x, y, w, h, r, t) = (*x, *y, *w, *h, *radius, *width);
                    self.shape((x, y, w, h), *colour, |px, py| {
                        rounded(px, py, (x, y, w, h), r)
                            && !rounded(
                                px,
                                py,
                                (x + t, y + t, w - 2.0 * t, h - 2.0 * t),
                                (r - t).max(0.0),
                            )
                    });
                }
                Draw::Text {
                    text,
                    size,
                    bold,
                    x,
                    y,
                    anchor,
                    colour,
                } => self.text(f, text, (*size, *bold), (*x, *y), *anchor, *colour),
                Draw::Stars { x, y, rating } => {
                    for i in 0..5 {
                        let cx = x + i as f32 * (ICON + 4.0) + ICON / 2.0;
                        let cy = y + ICON / 2.0;
                        let c = if (i as u8) < *rating {
                            colour::GOLD
                        } else {
                            [60, 66, 82]
                        };
                        let points = star(cx, cy, ICON / 2.0);
                        self.shape(
                            (cx - ICON / 2.0, cy - ICON / 2.0, ICON, ICON),
                            c,
                            |px, py| poly(px, py, &points),
                        );
                    }
                }
                Draw::Battery {
                    x,
                    y,
                    h,
                    percent,
                    charging,
                } => self.battery(*x, *y, *h, *percent, *charging),
                Draw::Bar { y, slot } => self.bar(*y, *slot),
                Draw::Glyph { glyph, x, y, h } => {
                    let s = self.scale;
                    let px_h = (h * s).round().max(1.0) as usize;
                    let (on, off) = (colour::WHITE, [70, 76, 92]);
                    let (gw, rgba) = super::glyphs::render(*glyph, px_h, on, off);
                    let (x0, y0) = ((x * s).round() as i32, (y * s).round() as i32);
                    for py in 0..px_h {
                        for px in 0..gw {
                            let i = (py * gw + px) * 4;
                            let a = rgba[i + 3];
                            if a > 0 {
                                let c = [rgba[i], rgba[i + 1], rgba[i + 2]];
                                self.blend(x0 + px as i32, y0 + py as i32, c, a as f32 / 255.0);
                            }
                        }
                    }
                }
                Draw::Picture {
                    index,
                    x,
                    y,
                    w,
                    h,
                    faded,
                } => {
                    if let Some((rgba, pw, ph)) = picture(*index) {
                        self.picture(rgba, pw, ph, (*x, *y, *w, *h), *faded);
                    }
                }
            }
        }
    }

    /// A picture fitted into a box (its proportions kept, centred), nearest pixel; faded
    /// towards black for a card aside.
    fn picture(
        &mut self,
        rgba: &[u8],
        pw: usize,
        ph: usize,
        (x, y, w, h): (f32, f32, f32, f32),
        faded: bool,
    ) {
        if pw == 0 || ph == 0 {
            return;
        }
        let fit = (w / pw as f32).min(h / ph as f32);
        let (fw, fh) = (pw as f32 * fit, ph as f32 * fit);
        let (x, y, w, h) = (x + (w - fw) / 2.0, y + (h - fh) / 2.0, fw, fh);
        let s = self.scale;
        let (x0, y0, x1, y1) = (
            (x * s) as i32,
            (y * s) as i32,
            ((x + w) * s) as i32,
            ((y + h) * s) as i32,
        );
        let k = if faded { 0.55 } else { 1.0 };
        for py in y0.max(0)..y1.min(self.h as i32) {
            let sy =
                ((py - y0) as usize * ph / (y1 - y0).max(1) as usize).min(ph.saturating_sub(1));
            for px in x0.max(0)..x1.min(self.w as i32) {
                let sx =
                    ((px - x0) as usize * pw / (x1 - x0).max(1) as usize).min(pw.saturating_sub(1));
                let i = (sy * pw + sx) * 4;
                let Some(p) = rgba.get(i..i + 3) else {
                    continue;
                };
                let c = [
                    (p[0] as f32 * k) as u8,
                    (p[1] as f32 * k) as u8,
                    (p[2] as f32 * k) as u8,
                ];
                self.blend(px, py, c, 1.0);
            }
        }
    }

    /// DSlot's bar: the panel colour with the slot cut into its top edge.
    fn bar(&mut self, top: f32, slot: Slot) {
        let s = self.scale;
        let (y0, h) = ((top * s) as i32, (BAR_H * s) as i32);
        let cx = W * s / 2.0;
        for y in 0..h {
            for x in 0..self.w as i32 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let (c, a) = match slot {
                    Slot::One => {
                        let (rx, ry) = (215.0 * s, 54.0 * s);
                        let d = ((fx - cx) / rx).hypot(fy / ry);
                        (colour::PANEL, (0.5 + (d - 1.0) * ry).clamp(0.0, 1.0))
                    }
                    Slot::Two(cart_w) => {
                        let (hw, depth, r) =
                            (cart_w * (600.0 / 560.0) / 2.0 * s, 30.0 * s, 12.0 * s);
                        let (qx, qy) = ((fx - cx).abs() - hw + r, fy - depth + r);
                        let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
                        let mouth = (fx - cx).abs() < hw - 16.0 * s
                            && (depth..depth + 7.0 * s).contains(&fy);
                        if mouth {
                            ([8, 9, 12], 1.0)
                        } else {
                            (colour::PANEL, (0.5 + d).clamp(0.0, 1.0))
                        }
                    }
                };
                self.blend(x, y0 + y, c, a);
            }
        }
    }

    /// DSlot's battery icon: an outline with its terminal, filled to the level (red when low
    /// and not charging), a bolt over it while charging.
    fn battery(&mut self, x: f32, y: f32, h: f32, percent: u8, charging: bool) {
        let w = h * 2.1;
        let level = percent.min(100) as f32 / 100.0;
        let rim = [200, 204, 212];
        let fill = if percent <= 15 && !charging {
            [226, 70, 70]
        } else {
            [235, 238, 244]
        };
        let body = (0.03, 0.06, 1.82, 0.88);
        let bolt = [
            (1.02, 0.08),
            (0.62, 0.56),
            (0.90, 0.56),
            (0.80, 0.92),
            (1.22, 0.40),
            (0.94, 0.40),
        ];
        let grow = |k: f32| bolt.map(|(bx, by)| (0.92 + (bx - 0.92) * k, 0.5 + (by - 0.5) * k));
        let edge = grow(1.22);
        let unit = |px: f32, py: f32| ((px - x) / h, (py - y) / h);
        let layers: [(Rgb, Inside); 4] = [
            (
                rim,
                Box::new(move |px, py| {
                    let (u, v) = unit(px, py);
                    (rounded(u, v, body, 0.16)
                        && !rounded(
                            u,
                            v,
                            (body.0 + 0.09, body.1 + 0.09, body.2 - 0.18, body.3 - 0.18),
                            0.09,
                        ))
                        || rounded(u, v, (body.0 + body.2 + 0.03, 0.32, 0.13, 0.36), 0.05)
                }),
            ),
            (
                fill,
                Box::new(move |px, py| {
                    let (u, v) = unit(px, py);
                    rounded(
                        u,
                        v,
                        (
                            body.0 + 0.17,
                            body.1 + 0.17,
                            (body.2 - 0.34) * level,
                            body.3 - 0.34,
                        ),
                        0.04,
                    )
                }),
            ),
            (
                [20, 24, 32],
                Box::new(move |px, py| {
                    charging && {
                        let (u, v) = unit(px, py);
                        poly(u, v, &edge)
                    }
                }),
            ),
            (
                [255, 206, 72],
                Box::new(move |px, py| {
                    charging && {
                        let (u, v) = unit(px, py);
                        poly(u, v, &bolt)
                    }
                }),
            ),
        ];
        for (c, inside) in &layers {
            self.shape((x, y, w, h), *c, inside);
        }
    }
}

/// A test of whether a point (in screen units) is in a shape.
type Inside<'a> = Box<dyn Fn(f32, f32) -> bool + 'a>;

/// Whether a point is inside a rectangle with rounded corners.
fn rounded(x: f32, y: f32, (rx, ry, rw, rh): (f32, f32, f32, f32), r: f32) -> bool {
    let dx = (x - rx - rw / 2.0).abs() - rw / 2.0 + r;
    let dy = (y - ry - rh / 2.0).abs() - rh / 2.0 + r;
    dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - r < 0.0
}

/// Whether a point is inside a polygon (even-odd).
fn poly(x: f32, y: f32, pts: &[(f32, f32)]) -> bool {
    (0..pts.len())
        .filter(|&i| {
            let ((ax, ay), (bx, by)) = (pts[i], pts[(i + 1) % pts.len()]);
            (ay > y) != (by > y) && x < ax + (y - ay) / (by - ay) * (bx - ax)
        })
        .count()
        % 2
        == 1
}

/// A five-pointed star's corners round a centre.
fn star(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..10)
        .map(|i| {
            let a = std::f32::consts::PI * (i as f32 / 5.0 - 0.5);
            let d = if i % 2 == 0 { r } else { r * 0.45 };
            (cx + d * a.cos(), cy + d * a.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{layout, Menu, Row, Status, Value};

    /// Roboto Condensed has the bullet a hidden passphrase is drawn in, but no backspace sign:
    /// why the keyboard's Delete key is a drawn glyph.
    #[test]
    fn font_coverage() {
        let fonts = Fonts::new();
        for bold in [false, true] {
            assert!(fonts.has('•', bold));
            assert!(fonts.has('a', bold));
            assert!(!fonts.has('⌫', bold));
        }
    }

    fn at(c: &Canvas, x: usize, y: usize) -> [u8; 3] {
        let i = (y * c.w + x) * 4;
        [c.px[i], c.px[i + 1], c.px[i + 2]]
    }

    /// A DS state's two screens stacked (tall) in a wide box: as tall as the box, centred,
    /// the sides left as they were; a picture of the box's own shape fills it.
    #[test]
    fn pictures_keep_their_shape() {
        let mut c = Canvas::new(1.0);
        let (pw, ph) = (2, 3);
        let red = [255u8, 0, 0, 255].repeat(pw * ph);
        let draw = Draw::Picture {
            index: 0,
            x: 100.0,
            y: 100.0,
            w: 300.0,
            h: 150.0,
            faded: false,
        };
        c.draw(&Fonts::new(), std::slice::from_ref(&draw), |_| {
            Some((red.as_slice(), pw, ph))
        });
        // 150 tall, so 100 wide, from x 200 to 300.
        assert_eq!(at(&c, 250, 175), [255, 0, 0]);
        assert_eq!(at(&c, 150, 175), [0, 0, 0]);
        assert_eq!(at(&c, 350, 175), [0, 0, 0]);
        let wide = [0u8, 255, 0, 255].repeat(2);
        c.draw(&Fonts::new(), &[draw], |_| Some((wide.as_slice(), 2, 1)));
        assert_eq!(at(&c, 105, 175), [0, 255, 0]);
        assert_eq!(at(&c, 395, 175), [0, 255, 0]);
    }

    #[test]
    fn a_page_draws_where_the_layout_says() {
        let m = Menu {
            tabs: vec![("Play".into(), true), ("Game".into(), false)],
            title: "TETRIS".into(),
            rows: vec![
                Row::new("Resume", Value::None),
                Row::new("Fast-forward", Value::Text("6x".into())),
            ],
            row: 1,
            scroll: 0,
            hint: "A  choose".into(),
        };
        let status = Status {
            clock: "5:05 PM".into(),
            battery: Some((76, false)),
            ..Status::default()
        };
        let mut c = Canvas::new(1.0);
        c.draw(&Fonts::new(), &layout::menu(&m, &status), |_| None);
        assert_eq!((c.w, c.h), (1024, 768));
        assert_eq!(at(&c, 30, 12), colour::WHITE, "the selected tab");
        assert_eq!(at(&c, 170, 12), colour::PANEL, "the other tab");
        assert_eq!(at(&c, 125, 205), colour::PANEL, "the highlight band");
        assert_eq!(at(&c, 10, 760), colour::PANEL, "the bar");
        assert_eq!(at(&c, 512, 680), [0, 0, 0], "slot-2's notch in the bar");
        // Some text was drawn on the highlighted row, in white, right of the middle.
        let row: Vec<[u8; 3]> = (600..874).map(|x| at(&c, x, 200)).collect();
        assert!(row.iter().any(|p| p[0] > 200), "the value");
    }

    #[test]
    fn a_text_anchor_puts_the_line_where_asked() {
        let f = Fonts::new();
        let mut c = Canvas::new(1.0);
        c.text(
            &f,
            "Hello",
            (26.0, true),
            (500.0, 100.0),
            1.0,
            colour::WHITE,
        );
        let lit =
            |x0: usize, x1: usize| (x0..x1).any(|x| (100..135).any(|y| at(&c, x, y)[0] > 100));
        assert!(lit(430, 500) && !lit(501, 600), "ends at x");
    }
}
