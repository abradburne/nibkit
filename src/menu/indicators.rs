//! RetroAchievements' indicators over a game that is playing, the same in every app: on screen
//! while they last, rather than in turn as toasts are.
//!
//! - **Progress:** one achievement's measured progress ("42/100"), shown, updated in place and
//!   taken down when rcheevos says: a panel in the toast's style headed "Progress", the badge,
//!   the title and the progress under it.
//! - **Challenges:** achievements that can be earned right now (finish without being hit), each
//!   shown until rcheevos takes it down, several at once: each just its badge, an icon
//!   [`ICON`] units square with rounded corners, in a gold ring that pulses slowly
//!   ([`toast::pulse_strength`]: the ring is what says it is live), the whole icon
//!   [`ICON_OPACITY`] opaque so the game shows through. No heading, no title: they are small so
//!   as not to be in the way. A challenge for the achievement whose progress is shown is not an
//!   icon of its own: it is shown in the progress panel, headed "Challenge", its badge ringed.
//!
//! They sit at the screen's bottom left, opposite the toasts ([`toast::stack`]): the progress
//! panel at the top, the challenges' icons in a row under it, the newest at the right; icons
//! that don't fit across the screen are left out, the oldest first. A panel or icon whose badge
//! is on its way waits for it up to [`toast::BADGE_WAIT`], as an unlock does, and then shows
//! without it (an icon as a dark square in its ring).
//!
//! An app keeps one [`Indicators`], tells it what rcheevos says, asks [`Indicators::tick`] once
//! a frame whether the picture changed, and lays [`Indicators::draw`]'s [`Stack`] over its
//! frame. The panel and icons are drawn (text rasterised: a few milliseconds) only when what
//! they say or the frame's size changes; in between, only the rings are painted again, at
//! [`PULSE_RATE`] steps a second, which is cheap: an app on a real-time thread need take it off
//! only around a draw whose tick changed. nibkit-menu-c's `nib_indicators_*` is the same for C
//! and C++.
//!
//! ```
//! use nibkit::menu::indicators::Indicators;
//! use nibkit::menu::raster::Fonts;
//! use nibkit::menu::toast::Notice;
//! use std::time::{Duration, Instant};
//! let fonts = Fonts::new();
//! let mut up = Indicators::new();
//! up.progress(11, Notice::progress("Collector", "42/100"));
//! up.challenge(12, Notice::challenge("No Damage").awaits_badge("12345"));
//! up.badge("12345", vec![255; 64 * 64 * 4], 64, 64);
//! // Once a frame:
//! let now = Instant::now();
//! let shown = up.tick(now);
//! assert_ne!(shown, 0);
//! let stack = up.draw(&fonts, (1024, 768)).unwrap();
//! assert_eq!(stack.place().0, 16);
//! // The ring moves on a step later: the same panel and icon, painted again.
//! assert_ne!(up.tick(now + Duration::from_millis(100)), shown);
//! ```

use super::raster::{Canvas, Fonts};
use super::scale_for;
use super::toast::{self, Kind, Notice};
use std::time::{Duration, Instant};

/// How many times a second a challenge's ring is painted again while it pulses: smooth enough
/// for a slow pulse, and a quarter of the frames a 60 Hz app would otherwise hand on.
pub const PULSE_RATE: u32 = 16;

/// A challenge's icon: its badge's side, in units (a toast's badge's least,
/// [`toast::BADGE_LEAST`]); the ring is outside it.
pub const ICON: f32 = toast::BADGE_LEAST;

/// How opaque a challenge's icon is, its badge and ring together (0 to 1): enough to read, the
/// game showing through.
pub const ICON_OPACITY: f32 = 0.8;

/// How round an icon's corners are, in units: as the menu's keys and buttons.
const ICON_RADIUS: f32 = 10.0;

/// Between icons' rings in their row, in units.
const ICON_GAP: f32 = 6.0;

/// An icon's picture, ring and all, in units: the badge and the ring each side.
const ICON_BOX: f32 = ICON + 2.0 * (toast::RING_GAP + toast::RING_WIDTH);

/// One step of the ring.
const STEP: Duration = Duration::from_nanos(1_000_000_000 / PULSE_RATE as u64);

/// An indicator up: the achievement, what its panel says, and since when it has been up (from
/// the first tick that saw it), for its badge's wait.
#[derive(Clone, Debug)]
struct Up {
    id: u32,
    notice: Notice,
    since: Option<Instant>,
}

impl Up {
    /// Whether it can show at `now`: it has its badge, or none is coming, or the wait is over.
    fn ready(&self, now: Instant) -> bool {
        self.notice.badge.is_some()
            || self.notice.awaits.is_none()
            || self
                .since
                .is_some_and(|s| now.saturating_duration_since(s) >= toast::BADGE_WAIT)
    }
}

/// A panel in the stack, by what it shows: found again by id when it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Progress,
    /// The challenge for the achievement whose progress is shown: one panel.
    Both(u32),
    Challenge(u32),
}

/// What the picture is of: what is said, the panels shown, and the ring's step.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Seen {
    said: u64,
    panels: Vec<Panel>,
    step: Option<u64>,
}

/// The panels drawn for a frame: the stack without its rings, where they go, and the stack as
/// last painted.
struct Drawn {
    said: u64,
    panels: Vec<Panel>,
    frame: (usize, usize),
    plain: Vec<u8>,
    rings: Vec<Ring>,
    step: Option<u64>,
    stack: Stack,
}

/// A ring to paint: the badge's square in the stack, its corners' radius in units, and how
/// opaque it is at full strength.
#[derive(Clone, Copy, Debug)]
struct Ring {
    badge: (usize, usize, usize, usize),
    corner: f32,
    opacity: f32,
}

/// RetroAchievements' indicators an app has up: see the module's documentation.
#[derive(Default)]
pub struct Indicators {
    progress: Option<Up>,
    challenges: Vec<Up>,
    /// Changes whenever what is said does.
    said: u64,
    /// The first tick's time: the rings pulse from it, together.
    epoch: Option<Instant>,
    /// What the last tick found, and the number it gave for it.
    seen: Option<Seen>,
    serial: u64,
    drawn: Option<Drawn>,
}

impl Indicators {
    /// None up.
    pub fn new() -> Indicators {
        Indicators::default()
    }

    /// Achievement `id`'s progress, made with [`Notice::progress`] (and its badge, or
    /// [`Notice::awaits_badge`]): up, or updated in place when it is already. One at a time, as
    /// rcheevos shows it.
    pub fn progress(&mut self, id: u32, notice: Notice) {
        let notice = self.keep_badge(id, notice);
        let since = self
            .progress
            .take()
            .filter(|p| p.id == id)
            .and_then(|p| p.since);
        self.progress = Some(Up { id, notice, since });
        self.said += 1;
    }

    /// The progress comes down.
    pub fn hide_progress(&mut self) {
        if self.progress.take().is_some() {
            self.said += 1;
        }
    }

    /// Achievement `id`'s challenge, made with [`Notice::challenge`] (and its badge, or
    /// [`Notice::awaits_badge`]): up after those already up, or updated in place.
    pub fn challenge(&mut self, id: u32, notice: Notice) {
        let notice = self.keep_badge(id, notice);
        match self.challenges.iter_mut().find(|c| c.id == id) {
            Some(c) => c.notice = notice,
            None => self.challenges.push(Up {
                id,
                notice,
                since: None,
            }),
        }
        self.said += 1;
    }

    /// Achievement `id`'s challenge comes down.
    pub fn end_challenge(&mut self, id: u32) {
        let before = self.challenges.len();
        self.challenges.retain(|c| c.id != id);
        if self.challenges.len() != before {
            self.said += 1;
        }
    }

    /// Every indicator comes down (the game changed, the machine was reset).
    pub fn clear(&mut self) {
        if !self.is_empty() {
            self.progress = None;
            self.challenges.clear();
            self.said += 1;
        }
    }

    /// A badge arrives: every indicator awaiting `name` has it. One none awaits is dropped.
    pub fn badge(&mut self, name: &str, rgba: Vec<u8>, w: usize, h: usize) {
        let mut used = false;
        for up in self.progress.iter_mut().chain(self.challenges.iter_mut()) {
            if up.notice.awaits.as_deref() == Some(name) {
                up.notice = std::mem::take(&mut up.notice).badge(rgba.clone(), w, h);
                used = true;
            }
        }
        if used {
            self.said += 1;
        }
    }

    /// Whether none is up.
    pub fn is_empty(&self) -> bool {
        self.progress.is_none() && self.challenges.is_empty()
    }

    /// Whether a ring is pulsing: the picture changes [`PULSE_RATE`] times a second.
    pub fn pulsing(&self) -> bool {
        self.seen.as_ref().is_some_and(|s| s.step.is_some())
    }

    /// What is on show at `now` (an [`Instant`] from a clock that only goes forward): 0 for
    /// nothing, else a number that changes whenever the picture does (what is said, a badge
    /// arriving, a wait for one over, the ring's next step). Call it once a frame.
    pub fn tick(&mut self, now: Instant) -> u64 {
        let epoch = *self.epoch.get_or_insert(now);
        for up in self.progress.iter_mut().chain(self.challenges.iter_mut()) {
            up.since.get_or_insert(now);
        }
        let panels = self.panels(now);
        if panels.is_empty() {
            self.seen = None;
            return 0;
        }
        let rings = panels.iter().any(|&p| self.ringed(p));
        let step = rings
            .then(|| (now.saturating_duration_since(epoch).as_nanos() / STEP.as_nanos()) as u64);
        let seen = Seen {
            said: self.said,
            panels,
            step,
        };
        if self.seen.as_ref() != Some(&seen) {
            self.seen = Some(seen);
            self.serial += 1;
        }
        self.serial
    }

    /// The panels as [`tick`](Indicators::tick) last found them, drawn for a `frame` (width,
    /// height) at its scale ([`scale_for`]) and stacked in its corner; `None` when there are
    /// none. Drawn when what they say or the frame's size changed, the rings painted again when
    /// they moved on; kept otherwise.
    pub fn draw(&mut self, fonts: &Fonts, frame: (usize, usize)) -> Option<&Stack> {
        let seen = self.seen.clone()?;
        let fresh = self
            .drawn
            .as_ref()
            .is_some_and(|d| d.said == seen.said && d.panels == seen.panels && d.frame == frame);
        if !fresh {
            self.drawn = self.draw_panels(fonts, &seen, frame);
        }
        let d = self.drawn.as_mut()?;
        if d.step != seen.step {
            d.stack.canvas.px.copy_from_slice(&d.plain);
            if let Some(step) = seen.step {
                let since = Duration::from_nanos((STEP.as_nanos() as u64).saturating_mul(step));
                let strength = toast::pulse_strength(since);
                let size = (d.stack.canvas.w, d.stack.canvas.h);
                for r in &d.rings {
                    toast::ring(
                        &mut d.stack.canvas.px,
                        size,
                        r.badge,
                        d.stack.canvas.scale,
                        strength * r.opacity,
                        r.corner,
                    );
                }
            }
            d.step = seen.step;
        }
        Some(&d.stack)
    }

    /// The panels to show at `now`, from the top: the progress (or it and its challenge in
    /// one), then the challenges, oldest first.
    fn panels(&self, now: Instant) -> Vec<Panel> {
        let both = self.progress.as_ref().and_then(|p| {
            self.challenges
                .iter()
                .find(|c| c.id == p.id)
                .map(|c| (p, c))
        });
        let mut out = Vec::new();
        match (both, &self.progress) {
            (Some((p, c)), _) if p.ready(now) || c.ready(now) => out.push(Panel::Both(c.id)),
            (None, Some(p)) if p.ready(now) => out.push(Panel::Progress),
            _ => {}
        }
        let merged = both.map(|(_, c)| c.id);
        out.extend(
            self.challenges
                .iter()
                .filter(|c| Some(c.id) != merged && c.ready(now))
                .map(|c| Panel::Challenge(c.id)),
        );
        out
    }

    /// What a panel says, or `None` when what it showed has come down since the tick.
    fn notice(&self, panel: Panel) -> Option<Notice> {
        let challenge = |id| self.challenges.iter().find(|c| c.id == id);
        match panel {
            Panel::Progress => self.progress.as_ref().map(|p| p.notice.clone()),
            Panel::Challenge(id) => challenge(id).map(|c| c.notice.clone()),
            Panel::Both(id) => {
                let (p, c) = (self.progress.as_ref()?, challenge(id)?);
                let mut n = c.notice.clone();
                n.detail = p.notice.detail.clone();
                if n.badge.is_none() {
                    n.badge = p.notice.badge.clone();
                }
                Some(n)
            }
        }
    }

    /// Whether it has a ring: every challenge's icon, and the progress panel shared with a
    /// challenge when it has a badge to ring.
    fn ringed(&self, panel: Panel) -> bool {
        match panel {
            Panel::Challenge(_) => true,
            Panel::Both(_) => self.notice(panel).is_some_and(|n| n.badge.is_some()),
            Panel::Progress => false,
        }
    }

    /// `notice` with the badge achievement `id` already has (an update to its progress, or
    /// its challenge after its progress), when it comes without one: an achievement's badge
    /// doesn't change.
    fn keep_badge(&self, id: u32, notice: Notice) -> Notice {
        if notice.badge.is_some() {
            return notice;
        }
        let had = self
            .progress
            .iter()
            .chain(&self.challenges)
            .filter(|u| u.id == id)
            .find_map(|u| u.notice.badge.clone());
        match had {
            Some((px, w, h)) => notice.badge(px, w, h),
            None => notice,
        }
    }

    /// Draws the panel and the icons for `frame` and lays them in their corner.
    fn draw_panels(&self, fonts: &Fonts, seen: &Seen, frame: (usize, usize)) -> Option<Drawn> {
        let scale = scale_for(frame.0, frame.1);
        // The progress panel, shared with its challenge or not.
        let top = seen
            .panels
            .iter()
            .filter(|p| matches!(p, Panel::Progress | Panel::Both(_)))
            .find_map(|&p| {
                let n = self.notice(p)?;
                let ringed = n.kind == Kind::Challenge && n.badge.is_some();
                Some((toast::draw(fonts, &n.said(), scale), ringed))
            });
        // The challenges' icons, newest last: as many as fit across, the newest kept.
        let side = (ICON_BOX * scale).round() as usize;
        let gap = (ICON_GAP * scale).round() as usize;
        let m = (toast::MARGIN * scale).round() as usize;
        let across = (frame.0.saturating_sub(2 * m) + gap) / (side + gap).max(1);
        let badges: Vec<Option<toast::Badge>> = seen
            .panels
            .iter()
            .filter_map(|&p| match p {
                Panel::Challenge(id) => self.challenges.iter().find(|c| c.id == id),
                _ => None,
            })
            .map(|c| {
                c.notice
                    .badge
                    .as_ref()
                    .map(|(px, w, h)| (px.as_slice(), *w, *h))
            })
            .collect();
        let badges = &badges[badges.len().saturating_sub(across)..];
        let row_w = (badges.len() * (side + gap)).saturating_sub(gap);
        let mut sizes = Vec::new();
        if let Some((t, _)) = &top {
            sizes.push((t.width(), t.height()));
        }
        if !badges.is_empty() {
            sizes.push((row_w, side));
        }
        let at = toast::stack(frame, scale, &sizes, usize::from(top.is_some()));
        let (top_at, row_at) = match &top {
            Some(_) => (at.first().copied().flatten(), at.get(1).copied().flatten()),
            None => (None, at.first().copied().flatten()),
        };
        let mut boxes: Vec<(usize, usize, usize, usize)> = Vec::new();
        if let (Some((t, _)), Some((x, y))) = (&top, top_at) {
            boxes.push((x, y, t.width(), t.height()));
        }
        if let Some((x, y)) = row_at.filter(|_| !badges.is_empty()) {
            boxes.push((x, y, row_w, side));
        }
        let x0 = boxes.iter().map(|b| b.0).min()?;
        let y0 = boxes.iter().map(|b| b.1).min()?;
        let x1 = boxes.iter().map(|b| b.0 + b.2).max()?;
        let y1 = boxes.iter().map(|b| b.1 + b.3).max()?;
        let mut canvas = Canvas::blank(x1 - x0, y1 - y0, scale);
        let mut rings = Vec::new();
        if let (Some((t, ringed)), Some((x, y))) = (&top, top_at) {
            let (ox, oy) = (x - x0, y - y0);
            put(&mut canvas, t.rgba(), (t.width(), t.height()), (ox, oy));
            if let (true, Some((bx, by, bw, bh))) = (*ringed, t.badge_rect()) {
                rings.push(Ring {
                    badge: (ox + bx, oy + by, bw, bh),
                    corner: toast::RING_RADIUS - toast::RING_GAP,
                    opacity: 1.0,
                });
            }
        }
        if let Some((x, y)) = row_at {
            for (i, &badge) in badges.iter().enumerate() {
                let (ox, oy) = (x - x0 + i * (side + gap), y - y0);
                let (px, inner) = icon(fonts, badge, scale, side);
                put(&mut canvas, &px, (side, side), (ox, oy));
                rings.push(Ring {
                    badge: (ox + inner.0, oy + inner.1, inner.2, inner.3),
                    corner: ICON_RADIUS,
                    opacity: ICON_OPACITY,
                });
            }
        }
        Some(Drawn {
            said: seen.said,
            panels: seen.panels.clone(),
            frame,
            plain: canvas.px.clone(),
            rings,
            step: None,
            stack: Stack {
                canvas,
                at: (x0, y0),
            },
        })
    }
}

/// `px` (`size` pixels) copied into `canvas` with its top left at `at`.
fn put(canvas: &mut Canvas, px: &[u8], (w, h): (usize, usize), (x, y): (usize, usize)) {
    for row in 0..h {
        let d = ((y + row) * canvas.w + x) * 4;
        canvas.px[d..d + w * 4].copy_from_slice(&px[row * w * 4..(row + 1) * w * 4]);
    }
}

/// A challenge's icon, `side` pixels square at `scale`: its badge (or a dark square while it
/// has none) with rounded corners in the middle, room for the ring round it, the whole
/// [`ICON_OPACITY`] opaque. Its pixels, and the badge's square in them.
fn icon(
    fonts: &Fonts,
    badge: Option<toast::Badge>,
    scale: f32,
    side: usize,
) -> (Vec<u8>, (usize, usize, usize, usize)) {
    let mut c = Canvas::blank(side, side, scale);
    let pad = toast::RING_GAP + toast::RING_WIDTH;
    let at = (pad, pad, ICON, ICON);
    match badge {
        Some(b) => {
            let place = super::Draw::Picture {
                index: 0,
                x: pad,
                y: pad,
                w: ICON,
                h: ICON,
                faded: false,
            };
            c.draw(fonts, std::slice::from_ref(&place), |_| Some(b));
        }
        None => {
            for p in c.px.chunks_exact_mut(4) {
                p[..3].copy_from_slice(&super::colour::PANEL);
            }
        }
    }
    // Its shape in the alpha: the rounded square, ICON_OPACITY at most.
    let shape = |x, y| super::raster::edge(x, y, at, ICON_RADIUS);
    c.cover(at, shape, |p, a| {
        p[3] = (a * ICON_OPACITY * 255.0).round() as u8
    });
    let (x0, x1) = ((pad * scale) as usize, ((pad + ICON) * scale) as usize);
    (c.px, (x0, x0, x1 - x0, x1 - x0))
}

/// The indicators drawn: the panels stacked in one picture, RGBA with straight alpha (clear
/// between and around them), rows top first, and where on the frame it goes.
pub struct Stack {
    canvas: Canvas,
    at: (usize, usize),
}

impl Stack {
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

    /// Its top left on the frame it was drawn for: the bottom left corner, [`toast::MARGIN`] in.
    pub fn place(&self) -> (usize, usize) {
        self.at
    }

    /// Lays it over `screen` (the frame it was drawn for, premultiplied RGBA) at its place: its
    /// panels' pixels replace the screen's, the clear ones between them leave it be.
    pub fn lay(&self, screen: &mut Canvas) {
        let (x0, y0) = self.at;
        let c = &self.canvas;
        for y in 0..c.h.min(screen.h.saturating_sub(y0)) {
            for x in 0..c.w.min(screen.w.saturating_sub(x0)) {
                let s = (y * c.w + x) * 4;
                let a = c.px[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let d = ((y0 + y) * screen.w + x0 + x) * 4;
                for k in 0..3 {
                    screen.px[d + k] = ((c.px[s + k] as u32 * a + 127) / 255) as u8;
                }
                screen.px[d + 3] = a as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::colour;
    use crate::menu::toast::{Said, BADGE_WAIT, PULSE};

    fn badge(v: u8) -> Vec<u8> {
        [v, v, v, 255].repeat(16 * 16)
    }

    fn alpha(s: &Stack, x: usize, y: usize) -> u8 {
        s.rgba()[(y * s.width() + x) * 4 + 3]
    }

    #[test]
    fn nothing_up_draws_nothing() {
        let mut up = Indicators::new();
        assert_eq!(up.tick(Instant::now()), 0);
        assert!(up.draw(&Fonts::new(), (1024, 768)).is_none());
        assert!(up.is_empty() && !up.pulsing());
    }

    /// An icon's side in pixels at scale 1: the badge and its ring.
    const BOX: usize = 66;

    #[test]
    fn progress_on_top_challenge_icons_in_a_row_under_it_bottom_left() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.progress(
            1,
            Notice::progress("Collector", "42/100").badge(badge(10), 16, 16),
        );
        up.challenge(2, Notice::challenge("No Damage").badge(badge(20), 16, 16));
        up.challenge(3, Notice::challenge("Speedy").badge(badge(30), 16, 16));
        assert_ne!(up.tick(t0), 0);
        assert_eq!(
            up.seen.as_ref().unwrap().panels,
            [Panel::Progress, Panel::Challenge(2), Panel::Challenge(3)]
        );
        let s = up.draw(&f, (1024, 768)).unwrap();
        let progress = toast::draw(
            &f,
            &Said::progress("Collector", "42/100").badge(&badge(10), 16, 16),
            1.0,
        );
        // Bottom left, the margin in: the panel as it is, a row of two icons under it.
        assert_eq!(ICON_BOX as usize, BOX);
        assert_eq!(s.place().0, 16);
        assert_eq!(s.place().1 + s.height(), 768 - 16);
        assert_eq!(s.height(), progress.height() + 8 + BOX);
        let row = &s.rgba()[..progress.width() * 4];
        assert_eq!(row, &progress.rgba()[..progress.width() * 4]);
        assert_eq!(alpha(s, 20, progress.height() + 4), 0, "a clear gap");
        // The icons' badges, oldest left, newest right, ICON_GAP apart.
        let y = progress.height() + 8 + BOX / 2;
        let px = |x: usize| &s.rgba()[(y * s.width() + x) * 4..][..4];
        let most = (ICON_OPACITY * 255.0).round() as u8;
        assert_eq!(px(BOX / 2), [20, 20, 20, most]);
        assert_eq!(px(BOX + 6 + BOX / 2), [30, 30, 30, most]);
        // Nothing past the second icon but the panel above.
        assert_eq!(alpha(s, 2 * BOX + 6 + 2, y), 0);
    }

    #[test]
    fn an_icon_is_its_badge_rounded_and_see_through() {
        let f = Fonts::new();
        let (px, inner) = icon(&f, Some((&badge(200), 16, 16)), 1.0, BOX);
        assert_eq!(inner, (5, 5, 56, 56));
        let at = |x: usize, y: usize| &px[(y * BOX + x) * 4..][..4];
        let most = (ICON_OPACITY * 255.0).round() as u8;
        // The badge, ICON_OPACITY opaque; its rounded corner and the ring's room clear.
        assert_eq!(at(33, 33), [200, 200, 200, most]);
        assert_eq!(at(5, 5)[3], 0, "the corner is rounded off");
        assert_eq!(at(2, 33)[3], 0);
        assert!(px.chunks_exact(4).all(|p| p[3] <= most));
        // Without a badge: a dark square in its place.
        let (px, _) = icon(&f, None, 1.0, BOX);
        assert_eq!(px[(33 * BOX + 33) * 4..][..3], colour::PANEL);
        // Twice the scale, twice the pixels.
        let (_, inner) = icon(&f, None, 2.0, 2 * BOX);
        assert_eq!(inner, (10, 10, 112, 112));
    }

    #[test]
    fn the_icons_ring_is_gold_and_as_see_through_as_the_icon() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.challenge(2, Notice::challenge("No Damage").badge(badge(5), 16, 16));
        up.tick(t0);
        // At the top of the pulse.
        up.tick(t0 + PULSE / 2);
        let s = up.draw(&f, (1024, 768)).unwrap();
        assert_eq!((s.width(), s.height()), (BOX, BOX));
        let at = |x: usize, y: usize| &s.rgba()[(y * BOX + x) * 4..][..4];
        let most = (ICON_OPACITY * 255.0).round() as u8;
        // In the middle of the ring's width, left of the badge: gold, ICON_OPACITY opaque.
        let ring = at(1, BOX / 2);
        assert_eq!(ring[..3], colour::GOLD);
        assert!(ring[3].abs_diff(most) <= 1, "{}", ring[3]);
        assert!(s.rgba().chunks_exact(4).all(|p| p[3] <= most + 1));
    }

    #[test]
    fn the_row_keeps_the_newest_when_it_is_too_long() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        // Across 1024 less the margins, 13 icons and their gaps fit; 15 are up.
        for id in 1..=15 {
            let v = id as u8 * 10;
            up.challenge(id, Notice::challenge("C").badge(badge(v), 16, 16));
        }
        up.tick(Instant::now());
        let s = up.draw(&f, (1024, 768)).unwrap();
        assert_eq!(s.width(), 13 * BOX + 12 * 6);
        let px = |i: usize| s.rgba()[((BOX / 2) * s.width() + i * (BOX + 6) + BOX / 2) * 4];
        // The two oldest left out; the newest at the right.
        assert_eq!([px(0), px(12)], [30, 150]);
    }

    #[test]
    fn a_challenge_for_the_progress_shown_is_one_panel() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        up.progress(
            7,
            Notice::progress("Collector", "42/100").badge(badge(10), 16, 16),
        );
        up.challenge(7, Notice::challenge("Collector"));
        up.challenge(8, Notice::challenge("No Damage").badge(badge(20), 16, 16));
        up.tick(Instant::now());
        assert_eq!(
            up.seen.as_ref().unwrap().panels,
            [Panel::Both(7), Panel::Challenge(8)]
        );
        // Headed Challenge, with the progress under the title and the progress's badge.
        let both = up.notice(Panel::Both(7)).unwrap();
        let said = both.said();
        assert_eq!(
            (said.kind, said.header, said.title, said.detail),
            (Kind::Challenge, toast::CHALLENGE, "Collector", "42/100")
        );
        assert!(said.badge.is_some());
        assert!(up.ringed(Panel::Both(7)));
        let s = up.draw(&f, (1024, 768)).unwrap();
        let want = toast::draw(&f, &said, 1.0);
        assert_eq!(s.height(), want.height() + 8 + BOX);
        assert_eq!(up.drawn.as_ref().unwrap().rings.len(), 2, "both ringed");
        // The challenge over, the progress is its own panel again.
        up.end_challenge(7);
        up.tick(Instant::now());
        assert_eq!(
            up.seen.as_ref().unwrap().panels,
            [Panel::Progress, Panel::Challenge(8)]
        );
    }

    #[test]
    fn a_challenge_waits_for_its_badge_a_moment() {
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.challenge(2, Notice::challenge("No Damage").awaits_badge("222"));
        assert_eq!(up.tick(t0), 0);
        assert_eq!(up.tick(t0 + BADGE_WAIT - Duration::from_millis(1)), 0);
        // Shown without it after the wait, a dark square in its ring.
        let before = up.tick(t0 + BADGE_WAIT);
        assert_ne!(before, 0);
        assert!(up.pulsing());
        // It comes: drawn again.
        up.badge("222", badge(5), 16, 16);
        assert_ne!(up.tick(t0 + BADGE_WAIT), before);
        // One that comes in time shows at once.
        let mut up = Indicators::new();
        up.challenge(2, Notice::challenge("No Damage").awaits_badge("222"));
        assert_eq!(up.tick(t0), 0);
        up.badge("222", badge(5), 16, 16);
        assert_ne!(up.tick(t0 + Duration::from_millis(10)), 0);
    }

    #[test]
    fn the_ring_moves_on_without_drawing_the_panels_again() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.challenge(2, Notice::challenge("No Damage").badge(badge(5), 16, 16));
        let a = up.tick(t0);
        let first = up.draw(&f, (1024, 768)).unwrap().rgba().to_vec();
        let plain = up.drawn.as_ref().unwrap().plain.as_ptr();
        // The same step: the same number, the same picture.
        assert_eq!(up.tick(t0 + STEP / 2), a);
        // The next steps: another number, the ring painted again over the same panels.
        let later = t0 + PULSE / 2;
        let b = up.tick(later);
        assert_ne!(b, a);
        let second = up.draw(&f, (1024, 768)).unwrap().rgba().to_vec();
        assert_ne!(first, second);
        assert_eq!(
            up.drawn.as_ref().unwrap().plain.as_ptr(),
            plain,
            "not drawn again"
        );
        // Steps come PULSE_RATE times a second.
        let n: std::collections::BTreeSet<u64> = (0..1000)
            .map(|ms| up.tick(t0 + Duration::from_millis(ms)))
            .collect();
        assert!(n.len().abs_diff(PULSE_RATE as usize) <= 1, "{}", n.len());
    }

    #[test]
    fn a_progress_alone_does_not_pulse() {
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.progress(
            1,
            Notice::progress("Collector", "1/10").badge(badge(1), 16, 16),
        );
        let a = up.tick(t0);
        assert_eq!(up.tick(t0 + Duration::from_secs(1)), a);
        assert!(!up.pulsing());
        // Updated in place: drawn again.
        up.progress(1, Notice::progress("Collector", "2/10"));
        let b = up.tick(t0 + Duration::from_secs(1));
        assert_ne!(b, a);
        assert!(
            up.progress.as_ref().unwrap().notice.badge.is_some(),
            "badge kept"
        );
    }

    #[test]
    fn coming_down_and_clearing() {
        let mut up = Indicators::new();
        let t0 = Instant::now();
        up.progress(1, Notice::progress("Collector", "1/10"));
        up.challenge(2, Notice::challenge("No Damage"));
        assert_ne!(up.tick(t0), 0);
        up.hide_progress();
        up.end_challenge(2);
        assert!(up.is_empty());
        assert_eq!(up.tick(t0), 0);
        up.challenge(3, Notice::challenge("Speedy"));
        up.clear();
        assert_eq!(up.tick(t0), 0);
    }

    #[test]
    fn laid_over_a_screen_where_its_icons_are() {
        let f = Fonts::new();
        let mut up = Indicators::new();
        up.challenge(2, Notice::challenge("No Damage").badge(badge(200), 16, 16));
        up.challenge(3, Notice::challenge("Speedy").badge(badge(90), 16, 16));
        up.tick(Instant::now());
        let s = up.draw(&f, (1024, 768)).unwrap();
        let mut screen = Canvas::new(1.0);
        for p in screen.px.chunks_exact_mut(4) {
            p.copy_from_slice(&[1, 2, 3, 255]);
        }
        s.lay(&mut screen);
        let (x, y) = s.place();
        let at = |x: usize, y: usize| &screen.px[(y * screen.w + x) * 4..][..4];
        // An icon's middle, premultiplied; between and beside them the screen untouched.
        let most = (ICON_OPACITY * 255.0).round() as u32;
        let v = ((200 * most + 127) / 255) as u8;
        assert_eq!(at(x + BOX / 2, y + BOX / 2), [v, v, v, most as u8]);
        assert_eq!(at(x + BOX + 3, y + BOX / 2), [1, 2, 3, 255]);
        assert_eq!(at(x + s.width() + 1, y + 20), [1, 2, 3, 255]);
    }
}
