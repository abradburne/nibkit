//! A toast in DSlot's style over a game that is playing: a small rounded panel with a heading, a
//! line and a line under it, an unlock's points at the right, an optional picture (an
//! achievement's badge) at the left, drawn with DSlot's face into a picture of its own size with
//! alpha. Every app's toast is this one, so an achievement looks the same in every emulator, and
//! so is the way they come and go: one at a time, in turn, each for its kind's time
//! ([`Kind::shows`]), an unlock waiting a moment for its badge.
//!
//! An app keeps a [`Queue`]: it pushes a [`Notice`] whenever there is something to say, asks
//! [`Queue::tick`] once a frame what is on show, and lays [`Queue::draw`]'s picture over its
//! frame: with [`lay`] when the frame is a [`Canvas`], or in its own way otherwise (blended, or
//! as a GPU overlay) at [`Toast::place`]. The picture is drawn once, when a toast appears (text
//! rasterised: a few milliseconds), and kept while it shows. nibkit-menu-c's `nib_toasts_*` is
//! the same queue for C and C++.
//!
//! [`draw`] draws one toast from a borrowed [`Said`], for an app that times its own.
//!
//! RetroAchievements' indicators, on screen while they last rather than in turn, are panels in
//! the same style ([`Kind::Progress`], [`Kind::Challenge`]): [`super::indicators`] keeps them
//! and lays them in their corner ([`stack`]), a challenge's badge ringed in gold that pulses
//! ([`pulse`]).
//!
//! ```
//! use nibkit::menu::raster::{Canvas, Fonts};
//! use nibkit::menu::toast::{self, Kind, Notice, Queue};
//! use std::time::{Duration, Instant};
//! let fonts = Fonts::new();
//! let mut toasts = Queue::new();
//! toasts.push(Notice::unlock("First Blood", "Win a battle", 25));
//! toasts.push(Notice::new(Kind::Info, "Saved to slot 1"));
//! // Once a frame:
//! let now = Instant::now();
//! let mut screen = Canvas::new(1.0);
//! assert!(toasts.tick(now).is_some());
//! if let Some(picture) = toasts.draw(&fonts, screen.scale) {
//!     toast::lay(&mut screen, picture);
//! }
//! // The unlock for four seconds, then the other.
//! let later = now + Kind::Unlock.shows();
//! assert_eq!(toasts.tick(later).unwrap().said().title, "Saved to slot 1");
//! assert!(toasts.tick(later + Duration::from_secs(3)).is_none());
//! ```

use super::colour::{self, Rgb};
use super::raster::{edge, mix, Canvas, Fonts};
use super::Draw;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// What a toast is about: the heading's colour, and how long it shows.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// Something to know.
    #[default]
    Info,
    /// An achievement earned: gold.
    Unlock,
    /// Something went wrong: red.
    Problem,
    /// An achievement's measured progress ("42/100"), on screen while RetroAchievements says:
    /// headed [`PROGRESS`], dim.
    Progress,
    /// An achievement that can be earned right now (finish without being hit), on screen while
    /// RetroAchievements says: headed [`CHALLENGE`] in gold, its badge ringed ([`pulse`]).
    Challenge,
}

impl Kind {
    /// How long a toast of this kind shows: four seconds for an unlock, long enough to read its
    /// two lines and the badge; two and a half for anything else.
    pub const fn shows(self) -> Duration {
        match self {
            Kind::Unlock => Duration::from_millis(4000),
            Kind::Info | Kind::Problem | Kind::Progress | Kind::Challenge => {
                Duration::from_millis(2500)
            }
        }
    }

    /// The heading's colour.
    fn colour(self) -> Rgb {
        match self {
            Kind::Unlock | Kind::Challenge => colour::GOLD,
            Kind::Problem => colour::RED,
            Kind::Info | Kind::Progress => colour::DIM,
        }
    }
}

/// An unlock's heading, the same in every app.
pub const UNLOCKED: &str = "Achievement unlocked";
/// A progress indicator's heading.
pub const PROGRESS: &str = "Progress";
/// A challenge indicator's heading.
pub const CHALLENGE: &str = "Challenge";

/// The longest an unlock waits at the head of the queue for its badge
/// ([`Notice::awaits_badge`]) before it shows without it.
pub const BADGE_WAIT: Duration = Duration::from_millis(1500);

/// How far in from the screen's bottom right corner a toast sits, in units; the indicators sit
/// as far in from the bottom left ([`stack`]).
pub const MARGIN: f32 = 16.0;

/// Between indicator panels stacked in their corner ([`stack`]), in units.
pub const STACK_GAP: f32 = 8.0;

/// The smallest a badge is drawn, in units: a toast of fewer lines is as tall as it needs to be
/// for this, its lines centred beside it. A challenge's badge is this size.
pub const BADGE_LEAST: f32 = 56.0;

/// One beat of a challenge's ring ([`pulse_strength`]): slow, so it reads as live without
/// pulling the eye from the game.
pub const PULSE: Duration = Duration::from_millis(1800);

/// A picture borrowed for a toast's left: RGBA bytes with straight alpha, rows top first, its
/// width and height.
pub type Badge<'a> = (&'a [u8], usize, usize);

/// What a toast says, borrowed: for [`draw`]. A heading or a detail may be empty and is then
/// left out. Made with [`Said::new`] (or [`Said::unlock`]) and the setters, so it can grow.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default)]
pub struct Said<'a> {
    /// What it is about, which colours the heading.
    pub kind: Kind,
    /// The heading, small, over the title.
    pub header: &'a str,
    /// The line that matters, in white.
    pub title: &'a str,
    /// A line under it, dim.
    pub detail: &'a str,
    /// An unlock's value, at the title's right; 0 for none.
    pub points: u32,
    /// A picture at the left, as tall as the text.
    pub badge: Option<Badge<'a>>,
}

impl<'a> Said<'a> {
    /// A toast of `kind` saying `title`, with no heading, detail, points or badge.
    pub fn new(kind: Kind, title: &'a str) -> Said<'a> {
        Said {
            kind,
            title,
            ..Said::default()
        }
    }

    /// An achievement unlocked: headed [`UNLOCKED`] in gold, its `title`, `detail` and
    /// `points`.
    pub fn unlock(title: &'a str, detail: &'a str, points: u32) -> Said<'a> {
        Said::new(Kind::Unlock, title)
            .header(UNLOCKED)
            .detail(detail)
            .points(points)
    }

    /// An achievement's measured progress: headed [`PROGRESS`], its `title`, and `measured`
    /// ("42/100") under it.
    pub fn progress(title: &'a str, measured: &'a str) -> Said<'a> {
        Said::new(Kind::Progress, title)
            .header(PROGRESS)
            .detail(measured)
    }

    /// An achievement that can be earned right now: headed [`CHALLENGE`] in gold, its `title`.
    pub fn challenge(title: &'a str) -> Said<'a> {
        Said::new(Kind::Challenge, title).header(CHALLENGE)
    }

    /// The heading over the title.
    pub fn header(mut self, header: &'a str) -> Said<'a> {
        self.header = header;
        self
    }

    /// The line under the title.
    pub fn detail(mut self, detail: &'a str) -> Said<'a> {
        self.detail = detail;
        self
    }

    /// An unlock's points.
    pub fn points(mut self, points: u32) -> Said<'a> {
        self.points = points;
        self
    }

    /// The picture at the left: `rgba` is `w` × `h` × 4 bytes.
    pub fn badge(mut self, rgba: &'a [u8], w: usize, h: usize) -> Said<'a> {
        self.badge = Some((rgba, w, h));
        self
    }
}

/// What a toast says, owned: for a [`Queue`]. Made with [`Notice::new`] (or
/// [`Notice::unlock`]) and the setters; [`Notice::said`] borrows it for [`draw`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    pub(super) kind: Kind,
    header: String,
    title: String,
    pub(super) detail: String,
    points: u32,
    pub(super) badge: Option<(Vec<u8>, usize, usize)>,
    /// The badge on its way, by name.
    pub(super) awaits: Option<String>,
    shows: Duration,
}

impl Notice {
    /// A toast of `kind` saying `title`, shown for the kind's time.
    pub fn new(kind: Kind, title: impl Into<String>) -> Notice {
        Notice {
            kind,
            title: title.into(),
            shows: kind.shows(),
            ..Notice::default()
        }
    }

    /// An achievement unlocked: headed [`UNLOCKED`] in gold, its `title`, `detail` and
    /// `points`.
    pub fn unlock(title: impl Into<String>, detail: impl Into<String>, points: u32) -> Notice {
        Notice::new(Kind::Unlock, title)
            .header(UNLOCKED)
            .detail(detail)
            .points(points)
    }

    /// An achievement's measured progress: headed [`PROGRESS`], its `title`, and `measured`
    /// ("42/100") under it.
    pub fn progress(title: impl Into<String>, measured: impl Into<String>) -> Notice {
        Notice::new(Kind::Progress, title)
            .header(PROGRESS)
            .detail(measured)
    }

    /// An achievement that can be earned right now: headed [`CHALLENGE`] in gold, its `title`.
    pub fn challenge(title: impl Into<String>) -> Notice {
        Notice::new(Kind::Challenge, title).header(CHALLENGE)
    }

    /// The heading over the title.
    pub fn header(mut self, header: impl Into<String>) -> Notice {
        self.header = header.into();
        self
    }

    /// The line under the title.
    pub fn detail(mut self, detail: impl Into<String>) -> Notice {
        self.detail = detail.into();
        self
    }

    /// An unlock's points.
    pub fn points(mut self, points: u32) -> Notice {
        self.points = points;
        self
    }

    /// The picture at the left: `rgba` is `w` × `h` × 4 bytes, straight alpha.
    pub fn badge(mut self, rgba: Vec<u8>, w: usize, h: usize) -> Notice {
        self.badge = Some((rgba, w, h));
        self.awaits = None;
        self
    }

    /// The picture at the left is on its way (fetched on another thread) and will be handed to
    /// [`Queue::badge`] as `name`: at the head of the queue the toast waits for it, up to
    /// [`BADGE_WAIT`], and shows without it after that.
    pub fn awaits_badge(mut self, name: impl Into<String>) -> Notice {
        self.awaits = Some(name.into());
        self
    }

    /// How long it shows, instead of its kind's time.
    pub fn shows(mut self, time: Duration) -> Notice {
        self.shows = time;
        self
    }

    /// What it says, borrowed, for [`draw`].
    pub fn said(&self) -> Said<'_> {
        Said {
            kind: self.kind,
            header: &self.header,
            title: &self.title,
            detail: &self.detail,
            points: self.points,
            badge: self
                .badge
                .as_ref()
                .map(|(px, w, h)| (px.as_slice(), *w, *h)),
        }
    }
}

/// The toast on show, from [`Queue::tick`].
#[derive(Debug)]
pub struct Shown {
    notice: Notice,
    id: u64,
    serial: u64,
    until: Instant,
}

impl Shown {
    /// What it says.
    pub fn said(&self) -> Said<'_> {
        self.notice.said()
    }

    /// Which notice it is: what [`Queue::push`] returned for it.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Changes whenever what is shown does (another toast, or this one's badge arriving), never
    /// 0: the time to draw its picture again, or hand the new one to the GPU.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// When it goes.
    pub fn until(&self) -> Instant {
        self.until
    }
}

/// The toasts an app has to show, one at a time and in turn: see the module's documentation.
#[derive(Default)]
pub struct Queue {
    waiting: VecDeque<(u64, Notice)>,
    shown: Option<Shown>,
    /// Since when the head of the queue has waited for its badge.
    held: Option<Instant>,
    /// The last id and serial given out.
    ids: u64,
    serials: u64,
    /// The shown toast's picture: its serial and scale, and the picture.
    picture: Option<(u64, f32, Toast)>,
}

impl Queue {
    /// An empty queue.
    pub fn new() -> Queue {
        Queue::default()
    }

    /// Adds a toast after those waiting; returns its id ([`Shown::id`]).
    pub fn push(&mut self, notice: Notice) -> u64 {
        self.ids += 1;
        self.waiting.push_back((self.ids, notice));
        self.ids
    }

    /// A badge arrives: every toast awaiting `name` has it, the one on show too (which then
    /// shows with it, its [`Shown::serial`] changed). One no toast awaits is dropped.
    pub fn badge(&mut self, name: &str, rgba: Vec<u8>, w: usize, h: usize) {
        let awaits = |n: &Notice| n.awaits.as_deref() == Some(name);
        if let Some(s) = self.shown.as_mut().filter(|s| awaits(&s.notice)) {
            s.notice = std::mem::take(&mut s.notice).badge(rgba.clone(), w, h);
            self.serials += 1;
            s.serial = self.serials;
        }
        for (_, n) in self.waiting.iter_mut().filter(|(_, n)| awaits(n)) {
            *n = std::mem::take(n).badge(rgba.clone(), w, h);
        }
    }

    /// The toast on show at `now` (an [`Instant`] from a clock that only goes forward), moving
    /// to the next when its time is up; `None` while there is none, or the next is waiting for
    /// its badge. Call it once a frame.
    pub fn tick(&mut self, now: Instant) -> Option<&Shown> {
        if self.shown.as_ref().is_some_and(|s| now >= s.until) {
            self.shown = None;
            self.picture = None;
        }
        if self.shown.is_none() {
            let (_, next) = self.waiting.front()?;
            if next.awaits.is_some() {
                let since = *self.held.get_or_insert(now);
                if now.saturating_duration_since(since) < BADGE_WAIT {
                    return None;
                }
            }
            self.held = None;
            let (id, notice) = self.waiting.pop_front()?;
            self.serials += 1;
            self.shown = Some(Shown {
                until: now + notice.shows,
                notice,
                id,
                serial: self.serials,
            });
        }
        self.shown.as_ref()
    }

    /// The toast on show as [`tick`](Queue::tick) last found it.
    pub fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    /// The shown toast's picture at `scale` pixels to a unit (the frame's: [`super::scale_for`]),
    /// drawn the first time it is asked for and kept until the toast changes.
    pub fn draw(&mut self, fonts: &Fonts, scale: f32) -> Option<&Toast> {
        let shown = self.shown.as_ref()?;
        let fresh = self
            .picture
            .as_ref()
            .is_some_and(|(serial, at, _)| *serial == shown.serial && *at == scale);
        if !fresh {
            self.picture = Some((shown.serial, scale, draw(fonts, &shown.said(), scale)));
        }
        self.picture.as_ref().map(|(_, _, t)| t)
    }

    /// Whether there is nothing on show and nothing waiting.
    pub fn is_empty(&self) -> bool {
        self.shown.is_none() && self.waiting.is_empty()
    }

    /// Drops every toast, the one on show too (the game is closing).
    pub fn clear(&mut self) {
        *self = Queue {
            ids: self.ids,
            serials: self.serials,
            ..Queue::default()
        };
    }
}

/// A toast drawn: RGBA with straight alpha (clear corners, the panel a little see-through), rows
/// top first, its own size.
pub struct Toast {
    canvas: Canvas,
    /// The badge's square in pixels (left, top, width, height), when it has one.
    badge: Option<(usize, usize, usize, usize)>,
}

impl Toast {
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

    /// Its top left on a `frame` (width, height) drawn at its scale: [`place`].
    pub fn place(&self, frame: (usize, usize)) -> (usize, usize) {
        place(frame, self.canvas.scale, (self.canvas.w, self.canvas.h))
    }

    /// The square its badge is drawn in, in its pixels: left, top, width and height; `None`
    /// without a badge. Where [`pulse`] rings it.
    pub fn badge_rect(&self) -> Option<(usize, usize, usize, usize)> {
        self.badge
    }
}

/// A line of a toast, by what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Line {
    Header,
    Title,
    Detail,
}

impl Line {
    /// Its size and the step down to the next line, in units, and whether it is bold.
    const fn style(self) -> (f32, f32, bool) {
        match self {
            Line::Header => (18.0, 26.0, true),
            Line::Title => (24.0, 32.0, true),
            Line::Detail => (20.0, 28.0, false),
        }
    }
}

/// The widest a toast is, in units: two thirds of the screen, past which it starts hiding the
/// game.
const MOST: f32 = super::W * 2.0 / 3.0;
/// The narrowest a toast is, so a short one doesn't look like a label.
const LEAST: f32 = 260.0;
/// In screen units (the screen is 1024 wide): the space inside the panel's edge.
const PAD: f32 = 16.0;
const RADIUS: f32 = 14.0;
/// The panel's edge's width.
const EDGE: f32 = 2.0;
/// Between the badge and the text, and the title and its points.
const GAP: f32 = 14.0;
/// The points' size, bold.
const POINTS: f32 = 20.0;
/// The last line's step is taller than its text by this much.
const LEADING: f32 = 4.0;
/// A line box's top is above its text by about this much: the first line starts that far up
/// into the padding, so the text looks centred.
const RISE: f32 = 3.0;
/// A line box is a pixel wider than its text at each end ([`Canvas::text`]).
const INK: f32 = 2.0;
/// The points sit this much lower than centred on the title, on its baseline.
const POINTS_DROP: f32 = 1.0;
/// The panel's opacity: the game shows through a little.
const OPACITY: f32 = 0.94;

/// Draws a toast at `scale` pixels to a unit (the frame's: [`super::scale_for`]), as wide as its
/// longest line wants within two thirds of the screen, long lines ending in an ellipsis.
pub fn draw(f: &Fonts, said: &Said, scale: f32) -> Toast {
    let scale = super::checked_scale(scale);
    let measure = |s: &str, (size, bold): (f32, bool)| width(f, s, (size, bold), scale);
    let points = match said.points {
        0 => None,
        1 => Some("1 point".to_owned()),
        n => Some(format!("{n} points")),
    };
    let lines: Vec<(Line, &str)> = [
        (Line::Header, said.header),
        (Line::Title, said.title),
        (Line::Detail, said.detail),
    ]
    .into_iter()
    .filter(|&(line, s)| line == Line::Title || !s.is_empty())
    .collect();
    let text_h = lines.iter().map(|(l, _)| l.style().1).sum::<f32>() - LEADING;
    let side = if said.badge.is_some() {
        text_h.max(BADGE_LEAST)
    } else {
        0.0
    };
    // The lines centred beside a badge taller than they are.
    let inside_h = text_h.max(side);
    let left = PAD
        + if said.badge.is_some() {
            side + GAP
        } else {
            0.0
        };
    let points_w = points
        .as_ref()
        .map_or(0.0, |p| measure(p, (POINTS, true)) + GAP);
    let room_for = |line: Line| if line == Line::Title { points_w } else { 0.0 };
    // As wide as the longest line wants, within MOST.
    let want = lines
        .iter()
        .map(|&(line, s)| {
            let (size, _, bold) = line.style();
            measure(s, (size, bold)) + room_for(line)
        })
        .fold(0.0f32, f32::max);
    let w = (left + want + PAD + INK).clamp(LEAST, MOST).ceil();
    let h = (PAD * 2.0 + inside_h).ceil();
    let room = w - left - PAD;

    let mut c = Canvas::blank(
        (w * scale).ceil() as usize,
        (h * scale).ceil() as usize,
        scale,
    );
    panel(&mut c, w, h);
    let mut badge = None;
    if said.badge.is_some() {
        // In pixels as Canvas::picture fills it: its ends truncated.
        let (x0, y0) = ((PAD * scale) as usize, (PAD * scale) as usize);
        let (x1, y1) = (
            ((PAD + side) * scale) as usize,
            ((PAD + side) * scale) as usize,
        );
        badge = Some((x0, y0, x1 - x0, y1 - y0));
        let place = Draw::Picture {
            index: 0,
            x: PAD,
            y: PAD,
            w: side,
            h: side,
            faded: false,
        };
        c.draw(f, std::slice::from_ref(&place), |_| said.badge);
    }
    let mut y = PAD + (inside_h - text_h) / 2.0 - RISE;
    for &(line, s) in &lines {
        let (size, step, bold) = line.style();
        let ink = match line {
            Line::Header => said.kind.colour(),
            Line::Title => colour::WHITE,
            Line::Detail => colour::DIM,
        };
        let s = fit(f, s, (size, bold), (room - room_for(line)).max(0.0), scale);
        c.text(f, &s, (size, bold), (left, y), 0.0, ink);
        if let (Line::Title, Some(p)) = (line, &points) {
            let at = (w - PAD, y + (size - POINTS) / 2.0 + POINTS_DROP);
            c.text(f, p, (POINTS, true), at, 1.0, colour::GOLD);
        }
        y += step;
    }
    Toast { canvas: c, badge }
}

/// Lays `toast` over `screen` (a screen-sized canvas, premultiplied RGBA) at its bottom right,
/// [`MARGIN`] in. The toast's pixels replace the screen's, so the screen is clear there or the
/// game is drawn under it later.
pub fn lay(screen: &mut Canvas, toast: &Toast) {
    let t = &toast.canvas;
    let (x0, y0) = place((screen.w, screen.h), screen.scale, (t.w, t.h));
    for y in 0..t.h.min(screen.h - y0) {
        for x in 0..t.w.min(screen.w - x0) {
            let s = (y * t.w + x) * 4;
            let d = ((y0 + y) * screen.w + x0 + x) * 4;
            let a = t.px[s + 3] as u32;
            for k in 0..3 {
                screen.px[d + k] = ((t.px[s + k] as u32 * a + 127) / 255) as u8;
            }
            screen.px[d + 3] = a as u8;
        }
    }
}

/// Where a `toast`-sized picture (width, height) goes on a `frame`-sized one drawn at `scale`
/// pixels to a unit: its top left, [`MARGIN`] in from the bottom right corner; 0 where it doesn't
/// fit.
pub fn place(frame: (usize, usize), scale: f32, toast: (usize, usize)) -> (usize, usize) {
    let m = (MARGIN * super::checked_scale(scale)).round() as usize;
    (
        frame.0.saturating_sub(toast.0 + m),
        frame.1.saturating_sub(toast.1 + m),
    )
}

/// Where indicator panels go on a `frame`-sized picture drawn at `scale` pixels to a unit: in
/// one column at its bottom left, [`MARGIN`] in, [`STACK_GAP`] apart, `sizes` (width, height)
/// from the top down; each one's top left, or `None` for one left out. Every app's indicators
/// sit there, opposite the toasts, the same way.
///
/// What doesn't fit between the frame's top and bottom margins is left out: the first `pinned`
/// (the progress panel, at the top) are kept first, then the rest from the bottom up (the
/// newest, last, kept before the oldest), stopping at the first that doesn't fit. A panel wider
/// than the frame starts at its left edge.
pub fn stack(
    frame: (usize, usize),
    scale: f32,
    sizes: &[(usize, usize)],
    pinned: usize,
) -> Vec<Option<(usize, usize)>> {
    let scale = super::checked_scale(scale);
    let m = (MARGIN * scale).round() as usize;
    let gap = (STACK_GAP * scale).round() as usize;
    let room = frame.1.saturating_sub(2 * m);
    let pinned = pinned.min(sizes.len());
    let mut kept = vec![false; sizes.len()];
    let (mut used, mut n) = (0, 0);
    let mut fits = |h: usize| {
        let need = used + h + if n > 0 { gap } else { 0 };
        let fit = need <= room;
        if fit {
            (used, n) = (need, n + 1);
        }
        fit
    };
    for i in 0..pinned {
        kept[i] = fits(sizes[i].1);
    }
    for i in (pinned..sizes.len()).rev() {
        if !fits(sizes[i].1) {
            break;
        }
        kept[i] = true;
    }
    // From the bottom up, each above the one under it.
    let mut out = vec![None; sizes.len()];
    let mut bottom = frame.1.saturating_sub(m);
    for i in (0..sizes.len()).rev().filter(|&i| kept[i]) {
        let (w, h) = sizes[i];
        let top = bottom.saturating_sub(h);
        out[i] = Some((m.min(frame.0.saturating_sub(w)), top));
        bottom = top.saturating_sub(gap);
    }
    out
}

/// How bright a challenge's ring is `since` it started pulsing: from a fifth up to full and back
/// once every [`PULSE`], smoothly; for [`pulse`].
pub fn pulse_strength(since: Duration) -> f32 {
    const LOW: f32 = 0.2;
    let beat = since.as_secs_f64() / PULSE.as_secs_f64();
    let wave = (1.0 - (beat.fract() * std::f64::consts::TAU).cos()) / 2.0;
    LOW + (1.0 - LOW) * wave as f32
}

/// The ring round a badge, in units: its gap from the badge, its width, and its inner corners'
/// radius.
const RING_GAP: f32 = 2.0;
const RING_WIDTH: f32 = 3.0;
const RING_RADIUS: f32 = 4.0;

/// Paints a challenge's ring into a copy of a drawn panel's pixels: a gold ([`colour::GOLD`])
/// rounded frame just outside `badge` ([`Toast::badge_rect`], moved to where the panel is in
/// `rgba`), at `strength` (0 to 1: [`pulse_strength`]) over what is there. `rgba` is `w` x `h`
/// pixels, straight alpha, drawn at `scale` pixels to a unit. Cheap (only the ring's pixels are
/// touched, no text is drawn), so the ring can pulse while the panel is drawn once: start each
/// step from the panel's own pixels. At strength 0 it changes nothing.
pub fn pulse(
    rgba: &mut [u8],
    (w, h): (usize, usize),
    badge: (usize, usize, usize, usize),
    scale: f32,
    strength: f32,
) {
    let strength = if strength.is_nan() {
        0.0
    } else {
        strength.clamp(0.0, 1.0)
    };
    if strength <= 0.0 || rgba.len() < w * h * 4 {
        return;
    }
    let s = super::checked_scale(scale);
    let (bx, by, bw, bh) = (
        badge.0 as f32,
        badge.1 as f32,
        badge.2 as f32,
        badge.3 as f32,
    );
    let (gap, width) = (RING_GAP * s, RING_WIDTH * s);
    // The badge's square grown by `d` pixels each way.
    let grow = |d: f32| (bx - d, by - d, bw + 2.0 * d, bh + 2.0 * d);
    let inner = grow(gap);
    let outer = grow(gap + width);
    let (ri, ro) = (RING_RADIUS * s, (RING_RADIUS + RING_WIDTH) * s);
    let x0 = outer.0.floor().max(0.0) as usize;
    let y0 = outer.1.floor().max(0.0) as usize;
    let x1 = ((outer.0 + outer.2).ceil().max(0.0) as usize).min(w);
    let y1 = ((outer.1 + outer.3).ceil().max(0.0) as usize).min(h);
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            // Inside the outer frame and outside the inner, by the distance to the nearer edge;
            // a pixel's share is how far its centre is inside, half a pixel either way.
            let d = edge(cx, cy, outer, ro).max(-edge(cx, cy, inner, ri));
            let a = (0.5 - d).clamp(0.0, 1.0) * strength;
            if a <= 0.0 {
                continue;
            }
            let i = (py * w + px) * 4;
            over(&mut rgba[i..i + 4], colour::GOLD, a);
        }
    }
}

/// `c` at `a` over the pixel `p` (straight alpha), as compositing does: the pixel takes `c`'s
/// share of what covers it.
fn over(p: &mut [u8], c: Rgb, a: f32) {
    let da = p[3] as f32 / 255.0;
    let out = a + da * (1.0 - a);
    if out <= 0.0 {
        return;
    }
    for k in 0..3 {
        let v = (c[k] as f32 * a + p[k] as f32 * da * (1.0 - a)) / out;
        p[k] = v.round().clamp(0.0, 255.0) as u8;
    }
    p[3] = (out * 255.0).round() as u8;
}

/// A line's width in units as [`Canvas::text`] draws it at `scale`: at its size in whole pixels.
fn width(f: &Fonts, s: &str, (size, bold): (f32, bool), scale: f32) -> f32 {
    f.width(s, (size * scale).round(), bold) / scale
}

/// `s` cut to `room` units as drawn at `scale`, ending in an ellipsis when it had to be cut.
fn fit(f: &Fonts, s: &str, style: (f32, bool), room: f32, scale: f32) -> String {
    if width(f, s, style, scale) <= room {
        return s.to_owned();
    }
    let mut out: String = s.to_owned();
    while !out.is_empty() && width(f, &format!("{out}\u{2026}"), style, scale) > room {
        out.pop();
    }
    format!("{}\u{2026}", out.trim_end())
}

/// The panel, `w` x `h` units: its colour inside a thin edge, the corners rounded, and its shape
/// in the picture's alpha (a little see-through).
fn panel(c: &mut Canvas, w: f32, h: f32) {
    for p in c.px.chunks_exact_mut(4) {
        p[..3].copy_from_slice(&colour::PANEL);
    }
    let all = (0.0, 0.0, w, h);
    let inner = (EDGE, EDGE, w - 2.0 * EDGE, h - 2.0 * EDGE);
    let outside = |x, y| edge(x, y, all, RADIUS);
    c.cover(all, outside, |p, a| {
        p[3] = (a * OPACITY * 255.0).round() as u8
    });
    // The edge's share of what the panel covers of a pixel: alpha is straight, so the colour
    // is the edge's wherever the panel there is all edge, however little of the pixel it is.
    let rim = |x, y| outside(x, y).max(-edge(x, y, inner, RADIUS - EDGE));
    c.cover(all, rim, |p, a| {
        let covered = p[3] as f32 / (OPACITY * 255.0).round();
        mix(&mut p[..3], colour::EDGE, a / covered.max(a));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(c: &Canvas, x: usize, y: usize) -> u8 {
        c.px[(y * c.w + x) * 4 + 3]
    }

    fn rgb(c: &Canvas, x: usize, y: usize) -> [u8; 3] {
        let i = (y * c.w + x) * 4;
        [c.px[i], c.px[i + 1], c.px[i + 2]]
    }

    fn panel_alpha() -> u8 {
        (OPACITY * 255.0).round() as u8
    }

    #[test]
    fn an_unlock_with_a_badge_and_points() {
        let f = Fonts::new();
        let badge = [200u8, 200, 200, 255].repeat(8 * 8);
        let t = draw(
            &f,
            &Said::unlock("First Blood", "Win a battle", 25).badge(&badge, 8, 8),
            1.0,
        );
        let c = &t.canvas;
        assert_eq!(c.h, (PAD * 2.0 + 26.0 + 32.0 + 28.0 - 4.0).ceil() as usize);
        assert!(c.w >= LEAST as usize && c.w as f32 <= MOST);
        // Corners clear, the middle all but opaque, the badge drawn at the left.
        assert_eq!(alpha(c, 0, 0), 0);
        assert_eq!(alpha(c, c.w / 2, c.h / 2), panel_alpha());
        assert_eq!(
            rgb(c, PAD as usize + 10, PAD as usize + 10),
            [200, 200, 200]
        );
        // The edge in its colour, the inside in the panel's.
        assert_eq!(rgb(c, c.w / 2, 0), colour::EDGE);
        assert_eq!(rgb(c, c.w / 2, 3), colour::PANEL);
        // Twice the scale, twice the size; no badge, narrower.
        let twice = draw(&f, &Said::unlock("First Blood", "Win a battle", 25), 2.0);
        assert!(twice.height().abs_diff(c.h * 2) <= 2);
        assert!(twice.width() < c.w * 2);
    }

    #[test]
    fn a_badge_shows_the_panel_through_where_it_is_clear() {
        let f = Fonts::new();
        let mut badge = [255u8, 0, 0, 255].repeat(8 * 8);
        // Its left half clear.
        for y in 0..8 {
            for x in 0..4 {
                badge[(y * 8 + x) * 4 + 3] = 0;
            }
        }
        let t = draw(
            &f,
            &Said::unlock("First Blood", "", 0).badge(&badge, 8, 8),
            1.0,
        );
        let (side, y) = (t.height() as f32 - PAD * 2.0, PAD as usize + 4);
        let x = |k: f32| (PAD + side * k) as usize;
        assert_eq!(rgb(&t.canvas, x(0.2), y), colour::PANEL);
        assert_eq!(rgb(&t.canvas, x(0.8), y), [255, 0, 0]);
    }

    #[test]
    fn one_point_is_a_point() {
        let f = Fonts::new();
        let wide = |n| draw(&f, &Said::unlock("A", "", n), 1.0).width();
        // "1 point" is narrower than "2 points" by the s; widths step by whole units, so at
        // least not wider.
        assert!(wide(1) <= wide(2));
        let one = width(&f, "1 point", (POINTS, true), 1.0);
        let two = width(&f, "2 points", (POINTS, true), 1.0);
        assert!(one < two);
    }

    #[test]
    fn long_lines_are_cut_to_the_width() {
        let f = Fonts::new();
        let long = "A very long achievement description that goes on and on past any toast";
        let cut = fit(&f, long, (20.0, false), 200.0, 1.0);
        assert!(cut.ends_with('\u{2026}'));
        assert!(f.width(&cut, 20.0, false) <= 200.0);
        assert_eq!(fit(&f, "Short", (20.0, false), 200.0, 1.0), "Short");
        let t = draw(&f, &Said::new(Kind::Info, long), 1.0);
        assert_eq!(t.width(), MOST.ceil() as usize);
        assert_eq!(t.height(), (PAD * 2.0 + 32.0 - 4.0).ceil() as usize);
    }

    /// At a scale where the face's whole-pixel size is larger than the scale says (20 units at
    /// 0.53 is 10.6 pixels, drawn at 11), a cut line still ends inside the padding.
    #[test]
    fn a_cut_line_stays_inside_at_any_scale() {
        let f = Fonts::new();
        let long = "Defeat every boss in the castle without taking a single hit, twice over";
        for scale in [0.53, 0.75, 1.0, 1.29, 1.5] {
            let t = draw(&f, &Said::new(Kind::Info, "Title").detail(long), scale);
            let c = &t.canvas;
            let (w, h) = (c.w as f32 / scale, c.h as f32 / scale);
            // Right of the room, inside the edge, between the panel's straight sides.
            let x0 = ((w - PAD) * scale).ceil() as usize + 1;
            let x1 = ((w - EDGE) * scale).floor() as usize - 1;
            let (y0, y1) = ((RADIUS * scale) as usize, ((h - RADIUS) * scale) as usize);
            for y in y0..y1 {
                for x in x0..x1 {
                    assert_eq!(rgb(c, x, y), colour::PANEL, "ink at ({x}, {y}) at {scale}");
                }
            }
        }
    }

    #[test]
    fn a_short_toast_is_never_narrower_than_least() {
        let f = Fonts::new();
        assert_eq!(
            draw(&f, &Said::new(Kind::Info, "Hi"), 1.0).width(),
            LEAST as usize
        );
    }

    #[test]
    fn a_scale_that_isnt_one_draws_at_one_and_a_tiny_one_at_the_least() {
        let f = Fonts::new();
        let one = draw(&f, &Said::new(Kind::Info, "Hi"), 1.0);
        for bad in [f32::NAN, f32::INFINITY] {
            assert_eq!(
                draw(&f, &Said::new(Kind::Info, "Hi"), bad).width(),
                one.width()
            );
        }
        let least = draw(&f, &Said::new(Kind::Info, "Hi"), 0.0);
        assert_eq!(least.scale(), super::super::LEAST_SCALE);
        assert!(least.width() > 0);
    }

    #[test]
    fn laid_at_the_bottom_right_premultiplied() {
        let f = Fonts::new();
        let t = draw(&f, &Said::unlock("First Blood", "Win a battle", 25), 1.0);
        let (tw, th) = (t.width(), t.height());
        let mut screen = Canvas::new(1.0);
        screen.px.fill(0);
        lay(&mut screen, &t);
        // Untouched outside it; its corner clear, its middle the panel premultiplied.
        assert_eq!(alpha(&screen, 0, 0), 0);
        assert_eq!(alpha(&screen, screen.w - 16 - tw, screen.h - 16 - th), 0);
        assert_eq!(alpha(&screen, screen.w - 1, screen.h - 1), 0);
        let (x, y) = (screen.w - 16 - tw / 2, screen.h - 16 - th / 2);
        let a = panel_alpha() as u32;
        assert_eq!(alpha(&screen, x, y), a as u8);
        let want = (colour::PANEL[0] as u32 * a + 127) / 255;
        assert_eq!(screen.px[(y * screen.w + x) * 4] as u32, want);
        // At twice the scale, the margin is twice the pixels.
        let big = draw(&f, &Said::unlock("First Blood", "Win a battle", 25), 2.0);
        let mut screen = Canvas::new(2.0);
        screen.px.fill(0);
        lay(&mut screen, &big);
        let (x, y) = (
            screen.w - 32 - big.width() / 2,
            screen.h - 32 - big.height() / 2,
        );
        assert_eq!(alpha(&screen, x, y), a as u8);
        assert_eq!(alpha(&screen, screen.w - 31, screen.h - 31), 0);
    }

    #[test]
    fn placed_where_lay_puts_it() {
        let f = Fonts::new();
        let t = draw(&f, &Said::unlock("First Blood", "Win a battle", 25), 1.0);
        let (tw, th) = (t.width(), t.height());
        let mut screen = Canvas::new(1.0);
        screen.px.fill(0);
        lay(&mut screen, &t);
        let (x, y) = t.place((screen.w, screen.h));
        assert_eq!((x, y), (screen.w - 16 - tw, screen.h - 16 - th));
        // The panel's middle is there; just past its bottom right corner is clear.
        assert_ne!(alpha(&screen, x + tw / 2, y + th / 2), 0);
        assert_eq!(alpha(&screen, x + tw, y + th), 0);
        // Twice the scale, twice the margin; too big for the screen, the corner.
        assert_eq!(
            place((2048, 1536), 2.0, (100, 50)),
            (2048 - 32 - 100, 1536 - 32 - 50)
        );
        assert_eq!(place((80, 40), 1.0, (100, 50)), (0, 0));
    }

    #[test]
    fn a_toast_bigger_than_the_screen_is_cut_not_overrun() {
        let f = Fonts::new();
        let t = draw(&f, &Said::unlock("First Blood", "Win a battle", 25), 1.0);
        let mut screen = Canvas::blank(t.width() / 2, t.height() / 2, 1.0);
        lay(&mut screen, &t);
        assert_eq!(alpha(&screen, screen.w / 2, screen.h / 2), panel_alpha());
    }

    #[test]
    fn a_notice_says_what_it_was_given() {
        let n = Notice::unlock("First Blood", "Win a battle", 25).badge(vec![0; 4], 1, 1);
        let s = n.said();
        assert_eq!(
            (s.kind, s.header, s.title, s.detail, s.points),
            (Kind::Unlock, UNLOCKED, "First Blood", "Win a battle", 25)
        );
        assert_eq!(s.badge, Some((&[0u8; 4][..], 1, 1)));
        assert_eq!(n.shows, Kind::Unlock.shows());
        let plain = Notice::new(Kind::Problem, "No network").shows(Duration::from_secs(9));
        assert_eq!(plain.said().header, "");
        assert_eq!(plain.shows, Duration::from_secs(9));
    }

    #[test]
    fn toasts_pushed_together_show_in_turn_each_for_its_time() {
        let mut q = Queue::new();
        let t0 = Instant::now();
        let a = q.push(Notice::unlock("First Blood", "", 5));
        let b = q.push(Notice::new(Kind::Info, "Saved to slot 1"));
        let first = q.tick(t0).unwrap();
        assert_eq!((first.id(), first.said().title), (a, "First Blood"));
        let serial = first.serial();
        assert_eq!(q.tick(t0 + Duration::from_millis(3999)).unwrap().id(), a);
        let second = q.tick(t0 + Kind::Unlock.shows()).unwrap();
        assert_eq!(second.id(), b);
        assert_ne!(second.serial(), serial);
        let gone = t0 + Kind::Unlock.shows() + Kind::Info.shows();
        assert!(q.tick(gone).is_none());
        assert!(q.is_empty());
    }

    #[test]
    fn an_unlock_waits_a_moment_for_its_badge() {
        let mut q = Queue::new();
        let t0 = Instant::now();
        q.push(Notice::unlock("First Blood", "", 5).awaits_badge("123"));
        assert!(q.tick(t0).is_none());
        assert!(q.tick(t0 + Duration::from_millis(500)).is_none());
        q.badge("456", vec![1; 4], 1, 1);
        assert!(
            q.tick(t0 + Duration::from_millis(600)).is_none(),
            "not its badge"
        );
        q.badge("123", vec![9; 4], 1, 1);
        let s = q.tick(t0 + Duration::from_millis(700)).unwrap();
        assert_eq!(s.said().badge, Some((&[9u8; 4][..], 1, 1)));
        assert_eq!(
            s.until(),
            t0 + Duration::from_millis(700) + Kind::Unlock.shows()
        );
    }

    #[test]
    fn without_its_badge_an_unlock_shows_after_the_wait_and_takes_it_late() {
        let mut q = Queue::new();
        let t0 = Instant::now();
        q.push(Notice::unlock("First Blood", "", 5).awaits_badge("123"));
        assert!(q.tick(t0).is_none());
        assert!(q.tick(t0 + BADGE_WAIT - Duration::from_millis(1)).is_none());
        let s = q.tick(t0 + BADGE_WAIT).unwrap();
        assert_eq!(s.said().badge, None);
        let serial = s.serial();
        q.badge("123", vec![9; 4], 1, 1);
        let s = q.tick(t0 + BADGE_WAIT).unwrap();
        assert!(s.said().badge.is_some());
        assert_ne!(s.serial(), serial, "drawn again with it");
    }

    #[test]
    fn the_picture_is_drawn_once_per_toast_and_scale() {
        let f = Fonts::new();
        let mut q = Queue::new();
        assert!(q.draw(&f, 1.0).is_none());
        let t0 = Instant::now();
        q.push(Notice::new(Kind::Info, "Saved to slot 1"));
        q.tick(t0);
        let first = q.draw(&f, 1.0).unwrap().rgba().as_ptr();
        assert_eq!(q.draw(&f, 1.0).unwrap().rgba().as_ptr(), first);
        let big = q.draw(&f, 2.0).unwrap();
        assert_eq!(big.scale(), 2.0);
        let want = draw(&f, &Said::new(Kind::Info, "Saved to slot 1"), 2.0);
        assert_eq!(big.rgba(), want.rgba());
        q.tick(t0 + Kind::Info.shows());
        assert!(q.draw(&f, 2.0).is_none());
    }

    #[test]
    fn clearing_drops_every_toast() {
        let mut q = Queue::new();
        let t0 = Instant::now();
        q.push(Notice::new(Kind::Info, "One"));
        q.push(Notice::new(Kind::Info, "Two"));
        let serial = q.tick(t0).unwrap().serial();
        q.clear();
        assert!(q.is_empty() && q.tick(t0).is_none());
        q.push(Notice::new(Kind::Info, "Three"));
        assert!(
            q.tick(t0).unwrap().serial() > serial,
            "serials never repeat"
        );
    }

    #[test]
    fn a_badge_is_never_under_the_least_and_its_square_is_known() {
        let f = Fonts::new();
        let badge = [200u8, 90, 40, 255].repeat(8 * 8);
        // Two lines (54 units) beside a 56-unit badge, centred on it.
        let t = draw(&f, &Said::challenge("No Damage").badge(&badge, 8, 8), 1.0);
        assert_eq!(t.height(), (PAD * 2.0 + BADGE_LEAST) as usize);
        assert_eq!(t.badge_rect(), Some((16, 16, 56, 56)));
        let (x, y, w, h) = t.badge_rect().unwrap();
        assert_eq!(rgb(&t.canvas, x + w / 2, y + h / 2), [200, 90, 40]);
        assert_eq!(rgb(&t.canvas, x + w + 2, y + h / 2), colour::PANEL);
        // Three lines: as tall as they are. Twice the scale, twice the pixels.
        let big = draw(
            &f,
            &Said::progress("Collector", "42/100").badge(&badge, 8, 8),
            2.0,
        );
        assert_eq!(big.badge_rect(), Some((32, 32, 164, 164)));
        assert_eq!(
            draw(&f, &Said::challenge("No Damage"), 1.0).badge_rect(),
            None
        );
    }

    #[test]
    fn the_indicators_kinds_are_headed_and_coloured() {
        let p = Said::progress("Collector", "42/100");
        assert_eq!(
            (p.kind, p.header, p.detail),
            (Kind::Progress, PROGRESS, "42/100")
        );
        let c = Notice::challenge("No Damage");
        assert_eq!(
            (c.said().kind, c.said().header),
            (Kind::Challenge, CHALLENGE)
        );
        assert_eq!(Kind::Challenge.colour(), colour::GOLD);
        assert_eq!(Kind::Progress.colour(), colour::DIM);
    }

    #[test]
    fn a_pulse_at_nothing_changes_nothing() {
        let f = Fonts::new();
        let badge = [200u8, 90, 40, 255].repeat(8 * 8);
        let t = draw(&f, &Said::challenge("No Damage").badge(&badge, 8, 8), 1.0);
        let mut px = t.rgba().to_vec();
        let size = (t.width(), t.height());
        for none in [0.0, -1.0, f32::NAN] {
            pulse(&mut px, size, t.badge_rect().unwrap(), 1.0, none);
            assert_eq!(px, t.rgba());
        }
        // Too few pixels for the size: left alone.
        pulse(&mut px[..8], size, t.badge_rect().unwrap(), 1.0, 1.0);
    }

    #[test]
    fn the_ring_is_gold_round_the_badge_at_full_strength() {
        let f = Fonts::new();
        let badge = [200u8, 90, 40, 255].repeat(8 * 8);
        for scale in [1.0, 1.5] {
            let t = draw(&f, &Said::challenge("No Damage").badge(&badge, 8, 8), scale);
            let (x, y, w, h) = t.badge_rect().unwrap();
            let mut px = t.rgba().to_vec();
            let size = (t.width(), t.height());
            pulse(&mut px, size, (x, y, w, h), scale, 1.0);
            let at = |px: &[u8], x: usize, y: usize| {
                let i = (y * size.0 + x) * 4;
                [px[i], px[i + 1], px[i + 2], px[i + 3]]
            };
            // In the middle of the ring's width, left, right, above and below: gold, opaque.
            let mid = ((RING_GAP + RING_WIDTH / 2.0) * scale) as usize + 1;
            let gold = [colour::GOLD[0], colour::GOLD[1], colour::GOLD[2], 255];
            for (rx, ry) in [
                (x - mid, y + h / 2),
                (x + w + mid - 1, y + h / 2),
                (x + w / 2, y - mid),
                (x + w / 2, y + h + mid - 1),
            ] {
                assert_eq!(at(&px, rx, ry), gold, "({rx}, {ry}) at {scale}");
            }
            // The badge and the panel past the ring untouched.
            assert_eq!(
                at(&px, x + w / 2, y + h / 2),
                at(t.rgba(), x + w / 2, y + h / 2)
            );
            let past = ((RING_GAP + RING_WIDTH + 1.5) * scale) as usize;
            assert_eq!(
                at(&px, x + w + past, y + h / 2),
                at(t.rgba(), x + w + past, y + h / 2)
            );
            // Half strength: half way from the panel to gold.
            let mut half = t.rgba().to_vec();
            pulse(&mut half, size, (x, y, w, h), scale, 0.5);
            let p = at(&half, x - mid, y + h / 2);
            let panel = at(t.rgba(), x - mid, y + h / 2);
            assert!(p[0] > panel[0] && p[0] < colour::GOLD[0]);
        }
    }

    #[test]
    fn the_pulse_is_slow_and_smooth() {
        assert!((pulse_strength(Duration::ZERO) - 0.2).abs() < 1e-6);
        assert!((pulse_strength(PULSE / 2) - 1.0).abs() < 1e-6);
        assert!((pulse_strength(PULSE) - 0.2).abs() < 1e-4);
        let quarter = pulse_strength(PULSE / 4);
        assert!(quarter > 0.5 && quarter < 0.7, "{quarter}");
    }

    #[test]
    fn indicators_stack_at_the_bottom_left() {
        // Three panels: the top one first, the newest at the bottom, the margin in.
        let at = stack((1024, 768), 1.0, &[(300, 114), (280, 88), (290, 88)], 1);
        let bottom = 768 - 16 - 88;
        let middle = bottom - 8 - 88;
        assert_eq!(
            at,
            [
                Some((16, middle - 8 - 114)),
                Some((16, middle)),
                Some((16, bottom))
            ]
        );
        // Twice the scale: twice the margin and gap.
        let at = stack((2048, 1536), 2.0, &[(600, 176), (560, 176)], 0);
        assert_eq!(
            at,
            [
                Some((32, 1536 - 32 - 176 - 16 - 176)),
                Some((32, 1536 - 32 - 176))
            ]
        );
        // Too many for the screen: the pinned top one kept, then the newest; the oldest under
        // the top one left out.
        let sizes = [(300, 200), (300, 200), (300, 200), (300, 200)];
        let at = stack((1024, 640), 1.0, &sizes, 1);
        let bottom = 640 - 16 - 200;
        assert_eq!(
            at,
            [Some((16, bottom - 8 - 200)), None, None, Some((16, bottom))]
        );
        // Without one pinned, the oldest goes first.
        let at = stack((1024, 640), 1.0, &sizes, 0);
        assert_eq!(at.iter().filter(|a| a.is_some()).count(), 2);
        assert_eq!(at[0], None);
        // Wider than the frame: from its left edge; nothing: nothing.
        assert_eq!(
            stack((200, 768), 1.0, &[(300, 50)], 0),
            [Some((0, 768 - 16 - 50))]
        );
        assert!(stack((1024, 768), 1.0, &[], 0).is_empty());
    }
}
