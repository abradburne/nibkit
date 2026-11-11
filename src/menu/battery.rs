//! The low-battery indicator over a game ([`crate::battery::Alerts`]): a small panel in the
//! toast's style at the screen's top right, DSlot's battery icon (red: low) and the charge left
//! in percent. It stays up while nibOS says the battery is very low (5%) or empty, till the
//! charger goes in or the device powers off, so a player in the middle of a game can't miss it.
//! The warning before it (10%) is a toast, which goes away. Every app's indicator is this one.
//!
//! An app keeps an [`Indicator`], [`shows`](Indicator::show) the percent while
//! [`Indicator::wanted`] says so for the alert up, [`hides`](Indicator::hide) it otherwise, and
//! lays [`Indicator::draw`]'s picture over its frame at [`Panel::place`]. The picture is drawn
//! when the percent changes and kept.
//!
//! ```
//! use nibkit::battery::Alert;
//! use nibkit::menu::battery::Indicator;
//! use nibkit::menu::raster::Fonts;
//! let fonts = Fonts::new();
//! let mut indicator = Indicator::new();
//! match Indicator::wanted(Some(Alert::Low(4))) {
//!     Some(percent) => indicator.show(percent),
//!     None => indicator.hide(),
//! }
//! let panel = indicator.draw(&fonts, 1.0).unwrap();
//! assert_eq!(panel.place((1024, 768)).1, 16);
//! ```

use super::colour;
use super::layout::Draw;
use super::raster::{edge, mix, Canvas, Fonts};
use super::toast::MARGIN;
use crate::battery::Alert;

/// Its size, in units.
const WIDTH: f32 = 128.0;
const HEIGHT: f32 = 48.0;
/// The space inside the panel's edge.
const PAD: f32 = 12.0;
const RADIUS: f32 = 12.0;
const EDGE: f32 = 2.0;
/// The battery icon's height (DSlot's icon is 2.1 times as wide).
const ICON: f32 = 20.0;
/// The number: its size (bold).
const NUMBER: f32 = 20.0;
/// A line box's top is above its text by about this much ([`super::toast`]'s).
const RISE: f32 = 3.0;
/// The panel's opacity, as a toast's.
const OPACITY: f32 = 0.94;

/// The indicator: the percent on show, and its picture.
#[derive(Default)]
pub struct Indicator {
    shown: Option<u8>,
    picture: Option<(u8, f32, Panel)>,
}

impl Indicator {
    /// Nothing on show.
    pub fn new() -> Indicator {
        Indicator::default()
    }

    /// The percent to show for the alert up `now`: very low or empty; `None` (nothing to
    /// show) otherwise. Empty has no percent of its own: 0.
    pub fn wanted(now: Option<Alert>) -> Option<u8> {
        match now {
            Some(Alert::Low(p)) => Some(p),
            Some(Alert::Critical) => Some(0),
            _ => None,
        }
    }

    /// Shows `percent` left, until [`hide`](Indicator::hide).
    pub fn show(&mut self, percent: u8) {
        self.shown = Some(percent.min(100));
    }

    /// Hides it (the charger is in).
    pub fn hide(&mut self) {
        self.shown = None;
    }

    /// The percent on show, if any.
    pub fn percent(&self) -> Option<u8> {
        self.shown
    }

    /// Its picture at `scale` pixels to a unit, drawn the first time it is asked for after the
    /// percent changes; `None` with nothing on show.
    pub fn draw(&mut self, fonts: &Fonts, scale: f32) -> Option<&Panel> {
        let percent = self.shown?;
        let fresh = self
            .picture
            .as_ref()
            .is_some_and(|(p, s, _)| *p == percent && *s == scale);
        if !fresh {
            self.picture = Some((percent, scale, draw(fonts, percent, scale)));
        }
        self.picture.as_ref().map(|(_, _, p)| p)
    }
}

/// The indicator drawn: RGBA with straight alpha (clear corners, the panel a little
/// see-through), rows top first, its own size.
pub struct Panel {
    canvas: Canvas,
}

impl Panel {
    /// Its width, in pixels.
    pub fn width(&self) -> usize {
        self.canvas.w
    }

    /// Its height, in pixels.
    pub fn height(&self) -> usize {
        self.canvas.h
    }

    /// Its pixels: 4 bytes each, red, green, blue and alpha (straight).
    pub fn rgba(&self) -> &[u8] {
        &self.canvas.px
    }

    /// Its top left on a `frame` (width, height) drawn at its scale: [`MARGIN`] in from the top
    /// right corner, clear of the toasts (bottom right) and the achievements' indicators
    /// (bottom left).
    pub fn place(&self, frame: (usize, usize)) -> (usize, usize) {
        let m = (MARGIN * self.canvas.scale).round() as usize;
        (frame.0.saturating_sub(self.canvas.w + m), m)
    }

    /// Blends it over `screen` (a screen-sized canvas, opaque) at its place.
    pub fn lay(&self, screen: &mut Canvas) {
        let t = &self.canvas;
        let (x0, y0) = self.place((screen.w, screen.h));
        for y in 0..t.h.min(screen.h.saturating_sub(y0)) {
            for x in 0..t.w.min(screen.w.saturating_sub(x0)) {
                let s = (y * t.w + x) * 4;
                let d = ((y0 + y) * screen.w + x0 + x) * 4;
                let a = t.px[s + 3] as f32 / 255.0;
                if a > 0.0 {
                    mix(
                        &mut screen.px[d..d + 3],
                        [t.px[s], t.px[s + 1], t.px[s + 2]],
                        a,
                    );
                }
            }
        }
    }
}

/// Draws the indicator for `percent` left at `scale` pixels to a unit (the frame's:
/// [`super::scale_for`]).
pub fn draw(f: &Fonts, percent: u8, scale: f32) -> Panel {
    let scale = super::checked_scale(scale);
    let mut c = Canvas::blank(
        (WIDTH * scale).ceil() as usize,
        (HEIGHT * scale).ceil() as usize,
        scale,
    );
    panel(&mut c);
    // DSlot's icon, filled to the charge: red, as it is at 15% and under off the charger.
    let percent = percent.min(100);
    c.draw(
        f,
        &[Draw::Battery {
            x: PAD,
            y: (HEIGHT - ICON) / 2.0,
            h: ICON,
            // At least a sliver, so the red shows at 0-4%.
            percent: percent.max(8),
            charging: false,
        }],
        |_| None,
    );
    let y = (HEIGHT - NUMBER) / 2.0 - RISE;
    c.text(
        f,
        &format!("{percent}%"),
        (NUMBER, true),
        (WIDTH - PAD, y),
        1.0,
        colour::RED,
    );
    Panel { canvas: c }
}

/// The panel: the toast's colour and edge, its corners rounded, a little see-through.
fn panel(c: &mut Canvas) {
    for p in c.px.chunks_exact_mut(4) {
        p[..3].copy_from_slice(&colour::PANEL);
    }
    let all = (0.0, 0.0, WIDTH, HEIGHT);
    let inner = (EDGE, EDGE, WIDTH - 2.0 * EDGE, HEIGHT - 2.0 * EDGE);
    let outside = |x, y| edge(x, y, all, RADIUS);
    c.cover(all, outside, |p, a| {
        p[3] = (a * OPACITY * 255.0).round() as u8
    });
    let rim = |x, y| outside(x, y).max(-edge(x, y, inner, RADIUS - EDGE));
    c.cover(all, rim, |p, a| {
        let covered = p[3] as f32 / (OPACITY * 255.0).round();
        mix(&mut p[..3], colour::RED, a / covered.max(a));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_while_very_low_or_empty() {
        assert_eq!(Indicator::wanted(None), None);
        assert_eq!(Indicator::wanted(Some(Alert::Warn(9))), None);
        assert_eq!(Indicator::wanted(Some(Alert::Low(4))), Some(4));
        assert_eq!(Indicator::wanted(Some(Alert::Critical)), Some(0));
        let f = Fonts::new();
        let mut i = Indicator::new();
        assert!(i.draw(&f, 1.0).is_none());
        i.show(4);
        assert_eq!(i.percent(), Some(4));
        assert!(i.draw(&f, 1.0).is_some());
        i.hide();
        assert!(i.draw(&f, 1.0).is_none());
    }

    #[test]
    fn top_right_red_and_mostly_opaque() {
        let f = Fonts::new();
        let p = draw(&f, 4, 1.0);
        assert_eq!((p.width(), p.height()), (WIDTH as usize, HEIGHT as usize));
        assert_eq!(
            p.place((1024, 768)),
            (1024 - WIDTH as usize - MARGIN as usize, MARGIN as usize)
        );
        // Corners clear, the middle all but opaque.
        assert_eq!(p.rgba()[3], 0);
        let mid = ((HEIGHT as usize / 2) * p.width() + p.width() / 2) * 4 + 3;
        assert_eq!(p.rgba()[mid], (OPACITY * 255.0).round() as u8);
        // Some red in it: the icon's fill and the number.
        let reddish = p
            .rgba()
            .chunks_exact(4)
            .filter(|px| px[0] > 180 && px[1] < 120 && px[3] > 200)
            .count();
        assert!(reddish > 50, "{reddish}");
        // Twice the scale, twice the size.
        assert_eq!(draw(&f, 4, 2.0).width(), WIDTH as usize * 2);
        // Laid over a screen: at the top right, nowhere else.
        let mut screen = Canvas::new(1.0);
        p.lay(&mut screen);
        let px = |x: usize, y: usize| screen.px[(y * screen.w + x) * 4];
        assert_eq!(px(10, 700), 0);
        assert!(px(1024 - 60, 40) > 0);
    }
}
