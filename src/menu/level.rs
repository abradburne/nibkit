//! The indicator for a level the volume keys just set ([`crate::levels`]): a small panel in the
//! toast's style at the screen's bottom right, a speaker (its waves lit by the volume) or a sun
//! at its left, a bar filled to the level, and the level in percent. It shows for [`SHOWS`],
//! then fades away over [`FADE`]; another press while it is up changes it and starts it again.
//! Every app's indicator is this one, so the volume looks the same over every game.
//!
//! An app keeps an [`Indicator`], [`shows`](Indicator::show) what a [`crate::levels::LevelWatch`]
//! gives, asks [`Indicator::tick`] once a frame for its opacity (0: nothing to draw), and lays
//! [`Indicator::draw`]'s picture over its frame at [`Panel::place`], at that opacity: with
//! [`Panel::lay`] when the frame is a [`Canvas`], or in its own way (blended, with an alpha of
//! its own; or [`Indicator::faded`], the picture with the opacity in its alpha). The picture is
//! drawn when the level changes (a few hundred pixels of text) and kept while it shows.
//!
//! A toast on show at the same time sits above it: [`Indicator::lift`] says how far up.
//!
//! ```
//! use nibkit::levels::{Level, What};
//! use nibkit::menu::level::{Indicator, FADE, SHOWS};
//! use nibkit::menu::raster::{Canvas, Fonts};
//! use std::time::Instant;
//! let fonts = Fonts::new();
//! let mut indicator = Indicator::new();
//! let now = Instant::now();
//! indicator.show(Level { what: What::Volume, percent: 40 }, now);
//! // Once a frame:
//! let mut screen = Canvas::new(1.0);
//! let opacity = indicator.tick(now);
//! assert_eq!(opacity, 1.0);
//! if let Some(panel) = indicator.draw(&fonts, screen.scale) {
//!     panel.lay(&mut screen, opacity);
//! }
//! // Gone once it has faded.
//! assert_eq!(indicator.tick(now + SHOWS + FADE), 0.0);
//! ```

use super::colour;
use super::glyphs::{self, Glyph};
use super::raster::{edge, mix, Canvas, Fonts};
use super::toast::STACK_GAP;
use crate::levels::{Level, What};
use std::time::{Duration, Instant};

/// How long the indicator shows at full strength after the last press.
pub const SHOWS: Duration = Duration::from_millis(1500);
/// How long it then takes to fade away.
pub const FADE: Duration = Duration::from_millis(500);

/// Its size, in units.
const WIDTH: f32 = 320.0;
const HEIGHT: f32 = 60.0;
/// The space inside the panel's edge.
const PAD: f32 = 16.0;
const RADIUS: f32 = 14.0;
const EDGE: f32 = 2.0;
/// The icon's height, and the width of its place (the speaker's, the wider): the bar starts at
/// the same place under both.
const ICON: f32 = 28.0;
const ICON_SLOT: f32 = ICON * 1.3;
/// Between the icon, the bar and the number.
const GAP: f32 = 14.0;
/// The bar's thickness.
const BAR: f32 = 8.0;
/// The number: its size (bold), and the room kept for "100%".
const NUMBER: f32 = 22.0;
const NUMBER_ROOM: f32 = 50.0;
/// A line box's top is above its text by about this much ([`super::toast`]'s).
const RISE: f32 = 3.0;
/// The panel's opacity, as a toast's: the game shows through a little.
const OPACITY: f32 = 0.94;

/// The indicator: the level on show, since when, and its picture.
#[derive(Default)]
pub struct Indicator {
    shown: Option<(Level, Instant)>,
    picture: Option<(Level, f32, Panel)>,
    faded: Option<(u8, Vec<u8>)>,
}

impl Indicator {
    /// Nothing on show.
    pub fn new() -> Indicator {
        Indicator::default()
    }

    /// Shows `level` from `now`, for [`SHOWS`] and then the fade.
    pub fn show(&mut self, level: Level, now: Instant) {
        self.shown = Some((level, now));
    }

    /// Hides it at once (the game is closing, a menu covers the screen).
    pub fn hide(&mut self) {
        self.shown = None;
    }

    /// The level on show, if any.
    pub fn level(&self) -> Option<Level> {
        self.shown.map(|(l, _)| l)
    }

    /// Its opacity `now`: 1 while it shows, falling to 0 over the fade; 0 once it is gone (and
    /// from then on, till the next [`show`](Indicator::show)).
    pub fn tick(&mut self, now: Instant) -> f32 {
        let o = self.opacity(now);
        if o <= 0.0 {
            self.shown = None;
        }
        o
    }

    /// Its opacity at `now`, as [`tick`](Indicator::tick) gives it, without forgetting it.
    pub fn opacity(&self, now: Instant) -> f32 {
        let Some((_, from)) = self.shown else {
            return 0.0;
        };
        let since = now.saturating_duration_since(from);
        if since <= SHOWS {
            return 1.0;
        }
        let t = (since - SHOWS).as_secs_f32() / FADE.as_secs_f32();
        if t >= 1.0 {
            return 0.0;
        }
        // Eased: quick to start going, gentle at the end.
        let k = 1.0 - t;
        k * k * (3.0 - 2.0 * k)
    }

    /// Whether it is on show (its opacity above 0 at the last [`tick`](Indicator::tick)).
    pub fn is_on(&self) -> bool {
        self.shown.is_some()
    }

    /// Its picture at `scale` pixels to a unit, at full strength, drawn the first time it is
    /// asked for after the level changes; `None` with nothing on show.
    pub fn draw(&mut self, fonts: &Fonts, scale: f32) -> Option<&Panel> {
        let (level, _) = self.shown?;
        let fresh = self
            .picture
            .as_ref()
            .is_some_and(|(l, s, _)| *l == level && *s == scale);
        if !fresh {
            self.picture = Some((level, scale, draw(fonts, level, scale)));
            self.faded = None;
        }
        self.picture.as_ref().map(|(_, _, p)| p)
    }

    /// Its picture with `opacity` in its alpha, for an app that can't blend at an opacity of its
    /// own: RGBA, straight alpha, the [`Panel`]'s size. Made again only when the opacity moves
    /// by a step of 1/32.
    pub fn faded(&mut self, fonts: &Fonts, scale: f32, opacity: f32) -> Option<&[u8]> {
        let step = (opacity.clamp(0.0, 1.0) * 32.0).round() as u8;
        self.draw(fonts, scale)?;
        let panel = &self.picture.as_ref()?.2;
        if self.faded.as_ref().is_none_or(|(s, _)| *s != step) {
            let k = f32::from(step) / 32.0;
            let mut px = panel.rgba().to_vec();
            for p in px.chunks_exact_mut(4) {
                p[3] = (p[3] as f32 * k).round() as u8;
            }
            self.faded = Some((step, px));
        }
        self.faded.as_ref().map(|(_, px)| px.as_slice())
    }

    /// How far a toast placed at [`super::toast::place`] should move up while this shows, in
    /// pixels at `scale`, so it sits just above the indicator: 0 with nothing on show.
    pub fn lift(&self, scale: f32) -> usize {
        if self.shown.is_none() {
            return 0;
        }
        let s = super::checked_scale(scale);
        ((HEIGHT * s).ceil() + (STACK_GAP * s).round()) as usize
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

    /// The pixels to a unit it was drawn at.
    pub fn scale(&self) -> f32 {
        self.canvas.scale
    }

    /// Its pixels: 4 bytes each, red, green, blue and alpha (straight).
    pub fn rgba(&self) -> &[u8] {
        &self.canvas.px
    }

    /// Its top left on a `frame` (width, height) drawn at its scale: [`super::toast::MARGIN`] in from the
    /// bottom right corner, where a toast goes ([`super::toast::place`]).
    pub fn place(&self, frame: (usize, usize)) -> (usize, usize) {
        super::toast::place(frame, self.canvas.scale, (self.canvas.w, self.canvas.h))
    }

    /// Blends it over `screen` (a screen-sized canvas, opaque) at its place, at `opacity`.
    pub fn lay(&self, screen: &mut Canvas, opacity: f32) {
        let t = &self.canvas;
        let (x0, y0) = self.place((screen.w, screen.h));
        let k = opacity.clamp(0.0, 1.0);
        for y in 0..t.h.min(screen.h.saturating_sub(y0)) {
            for x in 0..t.w.min(screen.w.saturating_sub(x0)) {
                let s = (y * t.w + x) * 4;
                let d = ((y0 + y) * screen.w + x0 + x) * 4;
                let a = t.px[s + 3] as f32 / 255.0 * k;
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

/// How many of the speaker's three waves `percent` lights: none when silent.
pub fn waves(percent: u8) -> u8 {
    match percent {
        0 => 0,
        1..=33 => 1,
        34..=66 => 2,
        _ => 3,
    }
}

/// The icon for a level: the speaker with its waves, or the sun.
pub fn glyph(level: Level) -> Glyph {
    match level.what {
        What::Volume => Glyph::Speaker(waves(level.percent)),
        What::Brightness => Glyph::Sun,
    }
}

/// Draws the indicator for `level` at `scale` pixels to a unit (the frame's:
/// [`super::scale_for`]).
pub fn draw(f: &Fonts, level: Level, scale: f32) -> Panel {
    let scale = super::checked_scale(scale);
    let percent = level.percent.min(100);
    let mut c = Canvas::blank(
        (WIDTH * scale).ceil() as usize,
        (HEIGHT * scale).ceil() as usize,
        scale,
    );
    panel(&mut c);

    // The icon, at the left, centred.
    let glyph = glyph(level);
    let ih = (ICON * scale).round().max(1.0) as usize;
    let (iw, icon) = glyphs::render(glyph, ih, colour::WHITE, colour::FAINT);
    let (ix, iy) = (
        ((PAD + ICON_SLOT / 2.0) * scale - iw as f32 / 2.0)
            .round()
            .max(0.0) as usize,
        ((HEIGHT * scale - ih as f32) / 2.0).round() as usize,
    );
    for y in 0..ih.min(c.h.saturating_sub(iy)) {
        for x in 0..iw.min(c.w.saturating_sub(ix)) {
            let s = (y * iw + x) * 4;
            let a = icon[s + 3] as f32 / 255.0;
            if a > 0.0 {
                let d = ((iy + y) * c.w + ix + x) * 4;
                mix(&mut c.px[d..d + 3], [icon[s], icon[s + 1], icon[s + 2]], a);
            }
        }
    }

    // The bar, from after the icon to before the number: its track, then the level over it.
    let left = PAD + ICON_SLOT + GAP;
    let right = WIDTH - PAD - NUMBER_ROOM - GAP;
    let top = (HEIGHT - BAR) / 2.0;
    let track = (left, top, right - left, BAR);
    let r = BAR / 2.0;
    c.cover(
        track,
        |x, y| edge(x, y, track, r),
        |p, a| mix(&mut p[..3], colour::EDGE, a),
    );
    if percent > 0 {
        // At least a dot's worth, so 5% shows.
        let w = ((right - left) * f32::from(percent) / 100.0).max(BAR);
        let fill = (left, top, w, BAR);
        c.cover(
            fill,
            |x, y| edge(x, y, fill, r),
            |p, a| mix(&mut p[..3], colour::WHITE, a),
        );
    }

    // The number, right-aligned, centred on the bar.
    let text = format!("{percent}%");
    let y = (HEIGHT - NUMBER) / 2.0 - RISE;
    c.text(
        f,
        &text,
        (NUMBER, true),
        (WIDTH - PAD, y),
        1.0,
        colour::WHITE,
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
        mix(&mut p[..3], colour::EDGE, a / covered.max(a));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::toast::MARGIN;

    fn volume(percent: u8) -> Level {
        Level {
            what: What::Volume,
            percent,
        }
    }

    fn rgb(p: &Panel, x: usize, y: usize) -> [u8; 3] {
        let i = (y * p.width() + x) * 4;
        let px = p.rgba();
        [px[i], px[i + 1], px[i + 2]]
    }

    #[test]
    fn shows_then_fades_then_is_gone() {
        let mut i = Indicator::new();
        let t0 = Instant::now();
        assert_eq!(i.tick(t0), 0.0);
        i.show(volume(40), t0);
        assert_eq!(i.tick(t0 + SHOWS), 1.0);
        let mid = i.tick(t0 + SHOWS + FADE / 2);
        assert!(mid > 0.1 && mid < 0.9, "{mid}");
        assert!(i.tick(t0 + SHOWS + FADE * 3 / 4) < mid);
        assert!(i.is_on());
        assert_eq!(i.tick(t0 + SHOWS + FADE), 0.0);
        assert!(!i.is_on());
        assert_eq!(i.lift(1.0), 0);
        // Another press starts it again.
        i.show(volume(45), t0 + SHOWS * 2);
        assert_eq!(i.tick(t0 + SHOWS * 3), 1.0);
        assert!(i.lift(1.0) > HEIGHT as usize);
    }

    #[test]
    fn the_bar_follows_the_level() {
        let f = Fonts::new();
        let (y, left) = ((HEIGHT / 2.0) as usize, PAD + ICON_SLOT + GAP);
        let right = WIDTH - PAD - NUMBER_ROOM - GAP;
        let at = |frac: f32| (left + (right - left) * frac) as usize;
        let level = |percent| Level {
            what: What::Brightness,
            percent,
        };
        let half = draw(&f, level(50), 1.0);
        assert_eq!(half.width(), WIDTH as usize);
        assert_eq!(half.height(), HEIGHT as usize);
        assert_eq!(rgb(&half, at(0.25), y), colour::WHITE);
        assert_eq!(rgb(&half, at(0.75), y), colour::EDGE);
        let full = draw(&f, level(100), 1.0);
        assert_eq!(rgb(&full, at(0.75), y), colour::WHITE);
        // Corners clear, the middle all but opaque.
        assert_eq!(half.rgba()[3], 0);
        let mid = (y * half.width() + half.width() / 2) * 4 + 3;
        assert_eq!(half.rgba()[mid], (OPACITY * 255.0).round() as u8);
        // Twice the scale, twice the size.
        assert_eq!(draw(&f, level(50), 2.0).width(), WIDTH as usize * 2);
    }

    #[test]
    fn placed_at_the_bottom_right_and_faded_in_its_alpha() {
        let f = Fonts::new();
        let mut i = Indicator::new();
        let now = Instant::now();
        i.show(volume(0), now);
        let (x, y) = i.draw(&f, 1.0).unwrap().place((1024, 768));
        assert_eq!(
            (x, y),
            (1024 - 320 - MARGIN as usize, 768 - 60 - MARGIN as usize)
        );
        let full = i.draw(&f, 1.0).unwrap().rgba().to_vec();
        let half = i.faded(&f, 1.0, 0.5).unwrap();
        let k = full.len() / 2 + 3;
        assert_eq!(half[k], (full[k] as f32 * 0.5).round() as u8);
        assert_eq!(&half[..3], &full[..3]);
        // Laid over a screen: the panel shows where it sits, and nowhere else.
        let mut screen = Canvas::new(1.0);
        let opacity = i.tick(now);
        i.draw(&f, 1.0).unwrap().lay(&mut screen, opacity);
        let px = |x: usize, y: usize| screen.px[(y * screen.w + x) * 4];
        assert_eq!(px(10, 10), 0);
        assert!(px(1024 - 200, 768 - 40) > 0);
    }

    #[test]
    fn waves_by_volume() {
        assert_eq!(
            [0, 5, 33, 34, 66, 67, 100].map(waves),
            [0, 1, 1, 2, 2, 3, 3]
        );
    }
}
