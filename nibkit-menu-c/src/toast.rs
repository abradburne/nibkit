//! Toasts in DSlot's style, for a game that is playing: a small rounded panel with a heading,
//! a line and a line under it, an optional picture (an achievement's badge) at its left, drawn
//! with DSlot's face into a picture of its own size with alpha, for the app to lay over its
//! frame at the corner nibkit gives. The drawing and the queue are nibkit's `menu::toast`, the
//! one every app's toasts are; this is their C face.
//!
//! A `NibToast` is filled a call at a time (begin, then the parts), as a `NibMenu` is: drawn on
//! its own (`nib_toast_render`), or handed to a `NibToasts` queue (`nib_toasts_push`), which
//! shows one at a time, in turn, each for its kind's time, an unlock waiting a moment for its
//! badge; or to `NibIndicators` (`nib_indicators_progress`, `_challenge`), RetroAchievements'
//! indicators, nibkit's `menu::indicators`: up while they last at the bottom left, the
//! progress a panel, each challenge its badge alone in a pulsing gold ring.

use nibkit::menu::indicators::Indicators;
use nibkit::menu::raster::Fonts;
use nibkit::menu::scale_for;
use nibkit::menu::toast::{self, Kind, Notice, Queue, Toast};
use std::ffi::{c_char, c_int};
use std::time::{Duration, Instant};

/// What the toast is about: the heading's colour says.
pub const INFO: c_int = 0;
/// An achievement earned: gold.
pub const UNLOCK: c_int = 1;
/// Something went wrong: red.
pub const PROBLEM: c_int = 2;
/// An achievement's measured progress: headed "Progress".
pub const PROGRESS: c_int = 3;
/// An achievement that can be earned right now: an icon in the indicators; headed "Challenge"
/// in gold where it shares the progress panel.
pub const CHALLENGE: c_int = 4;

/// A toast being filled, behind the C header's opaque `NibToast`: what it says, the fonts
/// (loaded when it is first drawn), and the last picture, which the caller reads until its next
/// call.
#[derive(Default)]
pub struct NibToast {
    notice: Notice,
    fonts: Option<Fonts>,
    picture: Option<Toast>,
}

/// The toasts an app has to show, behind the C header's opaque `NibToasts`: nibkit's queue, the
/// fonts (loaded when a toast is first drawn), and the caller's clock as nibkit's.
#[derive(Default)]
pub struct NibToasts {
    queue: Queue,
    fonts: Option<Fonts>,
    /// The caller's first time, in milliseconds, and the instant it was.
    clock: Option<(u64, Instant)>,
}

/// RetroAchievements' indicators, behind the C header's opaque `NibIndicators`: nibkit's, the
/// fonts (loaded when they are first drawn), and the caller's clock as nibkit's.
#[derive(Default)]
pub struct NibIndicators {
    up: Indicators,
    fonts: Option<Fonts>,
    /// The caller's first time, in milliseconds, and the instant it was.
    clock: Option<(u64, Instant)>,
}

/// The C header's kind as nibkit's; an unknown one is information.
fn kind_from(k: c_int) -> Kind {
    match k {
        UNLOCK => Kind::Unlock,
        PROBLEM => Kind::Problem,
        PROGRESS => Kind::Progress,
        CHALLENGE => Kind::Challenge,
        INFO => Kind::Info,
        _ => Kind::Info,
    }
}

/// Runs `f` on what `p` points at, if it isn't NULL.
///
/// # Safety
/// `p` is NULL or points at a live `T` not used elsewhere during the call.
unsafe fn with<T, R>(p: *mut T, or: R, f: impl FnOnce(&mut T) -> R) -> R {
    // SAFETY: the caller's promise.
    match unsafe { p.as_mut() } {
        Some(t) => f(t),
        None => or,
    }
}

/// Changes the toast `t` is filling with `f`.
///
/// # Safety
/// As [`with`].
unsafe fn fill(t: *mut NibToast, f: impl FnOnce(Notice) -> Notice) {
    // SAFETY: the caller's promise.
    unsafe { with(t, (), |t| t.notice = f(std::mem::take(&mut t.notice))) }
}

/// A frame's size from C: negative is 0.
fn dim(v: c_int) -> usize {
    v.max(0) as usize
}

/// Writes `v` where `out` points, if it isn't NULL.
///
/// # Safety
/// `out` is NULL or writable.
unsafe fn put(out: *mut c_int, v: usize) {
    // SAFETY: the caller's promise.
    if let Some(o) = unsafe { out.as_mut() } {
        *o = v as c_int;
    }
}

/// Hands `t` to C: its pixels, and its size and place on a `frame` where asked.
///
/// # Safety
/// Each out pointer is NULL or writable.
unsafe fn hand_over(
    t: &Toast,
    at: (usize, usize),
    (width, height, x, y): (*mut c_int, *mut c_int, *mut c_int, *mut c_int),
) -> *const u8 {
    // SAFETY: the caller's promise.
    unsafe {
        put(width, t.width());
        put(height, t.height());
        put(x, at.0);
        put(y, at.1);
    }
    t.rgba().as_ptr()
}

/// The caller's `now_ms` on a clock that started at its first time (`clock`) as an instant.
fn instant(clock: &mut Option<(u64, Instant)>, now_ms: u64) -> Instant {
    let (first, then) = *clock.get_or_insert((now_ms, Instant::now()));
    then + Duration::from_millis(now_ms.saturating_sub(first))
}

/// The version of the library: major × 10000 + minor × 100 + patch, as `NIBKIT_MENU_VERSION`
/// in the header it was built with.
pub const VERSION: c_int = 201;

#[no_mangle]
pub extern "C" fn nib_menu_version() -> c_int {
    VERSION
}

#[no_mangle]
pub extern "C" fn nib_menu_scale_for(width: c_int, height: c_int) -> f32 {
    scale_for(dim(width), dim(height))
}

#[no_mangle]
pub extern "C" fn nib_toast_new() -> *mut NibToast {
    Box::into_raw(Box::default())
}

/// # Safety
/// `t` is NULL or came from `nib_toast_new` and isn't used after.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_free(t: *mut NibToast) {
    if !t.is_null() {
        // SAFETY: the caller's promise.
        drop(unsafe { Box::from_raw(t) });
    }
}

/// # Safety
/// `t` is NULL or a live `NibToast`; `title` is NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_begin(t: *mut NibToast, kind: c_int, title: *const c_char) {
    // SAFETY: the caller's promises, passed on.
    let title = unsafe { crate::text(title) };
    unsafe { fill(t, |_| Notice::new(kind_from(kind), title)) }
}

/// # Safety
/// As `nib_toast_begin`, for both strings.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_unlock(
    t: *mut NibToast,
    title: *const c_char,
    detail: *const c_char,
    points: c_int,
) {
    // SAFETY: the caller's promises, passed on.
    let (title, detail) = unsafe { (crate::text(title), crate::text(detail)) };
    unsafe { fill(t, |_| Notice::unlock(title, detail, points.max(0) as u32)) }
}

/// # Safety
/// As `nib_toast_begin`, for both strings.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_progress(
    t: *mut NibToast,
    title: *const c_char,
    measured: *const c_char,
) {
    // SAFETY: the caller's promises, passed on.
    let (title, measured) = unsafe { (crate::text(title), crate::text(measured)) };
    unsafe { fill(t, |_| Notice::progress(title, measured)) }
}

/// # Safety
/// As `nib_toast_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_challenge(t: *mut NibToast, title: *const c_char) {
    // SAFETY: the caller's promises, passed on.
    let title = unsafe { crate::text(title) };
    unsafe { fill(t, |_| Notice::challenge(title)) }
}

/// # Safety
/// As `nib_toast_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_header(t: *mut NibToast, header: *const c_char) {
    // SAFETY: the caller's promises, passed on.
    let header = unsafe { crate::text(header) };
    unsafe { fill(t, |n| n.header(header)) }
}

/// # Safety
/// As `nib_toast_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_detail(t: *mut NibToast, detail: *const c_char) {
    // SAFETY: the caller's promises, passed on.
    let detail = unsafe { crate::text(detail) };
    unsafe { fill(t, |n| n.detail(detail)) }
}

/// # Safety
/// `t` is NULL or a live `NibToast`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_points(t: *mut NibToast, points: c_int) {
    // SAFETY: the caller's promise.
    unsafe { fill(t, |n| n.points(points.max(0) as u32)) }
}

/// # Safety
/// `t` is NULL or a live `NibToast`; `rgba` is NULL or `width` × `height` × 4 bytes, read
/// during the call only (it is copied).
#[no_mangle]
pub unsafe extern "C" fn nib_toast_badge(
    t: *mut NibToast,
    rgba: *const u8,
    width: c_int,
    height: c_int,
) {
    // SAFETY: the caller's promise.
    let badge =
        unsafe { crate::picture(rgba, width, height) }.map(|(px, w, h)| (px.to_vec(), w, h));
    unsafe {
        fill(t, |n| match badge {
            Some((px, w, h)) => n.badge(px, w, h),
            None => n,
        })
    }
}

/// # Safety
/// As `nib_toast_begin`, for `name`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_await_badge(t: *mut NibToast, name: *const c_char) {
    // SAFETY: the caller's promises, passed on.
    let name = unsafe { crate::text(name) };
    unsafe { fill(t, |n| n.awaits_badge(name)) }
}

/// # Safety
/// `t` is NULL or a live `NibToast`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_shows(t: *mut NibToast, ms: c_int) {
    let time = Duration::from_millis(ms.max(0) as u64);
    // SAFETY: the caller's promise.
    unsafe { fill(t, |n| n.shows(time)) }
}

/// The toast drawn for a `frame_w` x `frame_h` frame, at that frame's scale, and where on it it
/// goes.
///
/// # Safety
/// `t` is NULL or a live `NibToast`; the out pointers are NULL or writable. The result stays
/// valid until the next call on `t`.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_render(
    t: *mut NibToast,
    frame_w: c_int,
    frame_h: c_int,
    width: *mut c_int,
    height: *mut c_int,
    x: *mut c_int,
    y: *mut c_int,
) -> *const u8 {
    let frame = (dim(frame_w), dim(frame_h));
    // SAFETY: the caller's promise.
    unsafe {
        with(t, std::ptr::null(), |t| {
            let fonts = t.fonts.get_or_insert_with(Fonts::new);
            let drawn = toast::draw(fonts, &t.notice.said(), scale_for(frame.0, frame.1));
            let drawn = t.picture.insert(drawn);
            // SAFETY: the caller's promise that these are NULL or writable.
            hand_over(drawn, drawn.place(frame), (width, height, x, y))
        })
    }
}

/// The badge's square in the last picture `nib_toast_render` gave: 1 and its place, or 0 (the
/// outs untouched) without one.
///
/// # Safety
/// `t` is NULL or a live `NibToast`; the out pointers are NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_badge_rect(
    t: *const NibToast,
    x: *mut c_int,
    y: *mut c_int,
    width: *mut c_int,
    height: *mut c_int,
) -> c_int {
    // SAFETY: the caller's promise.
    let rect = unsafe { t.as_ref() }
        .and_then(|t| t.picture.as_ref())
        .and_then(Toast::badge_rect);
    let Some((rx, ry, rw, rh)) = rect else {
        return 0;
    };
    // SAFETY: the caller's promise.
    unsafe {
        put(x, rx);
        put(y, ry);
        put(width, rw);
        put(height, rh);
    }
    1
}

/// # Safety
/// `rgba` is NULL or `width` x `height` x 4 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_pulse(
    rgba: *mut u8,
    width: c_int,
    height: c_int,
    x: c_int,
    y: c_int,
    w: c_int,
    h: c_int,
    scale: f32,
    strength: f32,
) {
    let (width, height) = (dim(width), dim(height));
    if rgba.is_null() || width == 0 || height == 0 {
        return;
    }
    // SAFETY: the caller's promise.
    let px = unsafe { std::slice::from_raw_parts_mut(rgba, width * height * 4) };
    let badge = (dim(x), dim(y), dim(w), dim(h));
    toast::pulse(px, (width, height), badge, scale, strength);
}

#[no_mangle]
pub extern "C" fn nib_toast_pulse_strength(ms: u64) -> f32 {
    toast::pulse_strength(Duration::from_millis(ms))
}

/// # Safety
/// `sizes` is NULL or `n` pairs (width, height); `places` is NULL or `n` writable pairs.
#[no_mangle]
pub unsafe extern "C" fn nib_toast_stack(
    frame_w: c_int,
    frame_h: c_int,
    n: c_int,
    sizes: *const c_int,
    pinned: c_int,
    places: *mut c_int,
) -> c_int {
    let n = dim(n);
    if sizes.is_null() || places.is_null() || n == 0 {
        return 0;
    }
    // SAFETY: the caller's promise.
    let (sizes, places) = unsafe {
        (
            std::slice::from_raw_parts(sizes, n * 2),
            std::slice::from_raw_parts_mut(places, n * 2),
        )
    };
    let sizes: Vec<(usize, usize)> = sizes
        .chunks_exact(2)
        .map(|s| (dim(s[0]), dim(s[1])))
        .collect();
    let frame = (dim(frame_w), dim(frame_h));
    let at = toast::stack(frame, scale_for(frame.0, frame.1), &sizes, dim(pinned));
    let mut kept = 0;
    for (out, a) in places.chunks_exact_mut(2).zip(at) {
        let (x, y) = a.map_or((-1, -1), |(x, y)| {
            kept += 1;
            (x as c_int, y as c_int)
        });
        (out[0], out[1]) = (x, y);
    }
    kept
}

#[no_mangle]
pub extern "C" fn nib_toasts_new() -> *mut NibToasts {
    Box::into_raw(Box::default())
}

/// # Safety
/// `q` is NULL or came from `nib_toasts_new` and isn't used after.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_free(q: *mut NibToasts) {
    if !q.is_null() {
        // SAFETY: the caller's promise.
        drop(unsafe { Box::from_raw(q) });
    }
}

/// Queues what `t` says (copied: `t` can be begun again at once); its id, or 0 if either is
/// NULL.
///
/// # Safety
/// `q` and `t` are NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_push(q: *mut NibToasts, t: *const NibToast) -> u64 {
    // SAFETY: the caller's promise.
    let Some(t) = (unsafe { t.as_ref() }) else {
        return 0;
    };
    // SAFETY: as above.
    unsafe { with(q, 0, |q| q.queue.push(t.notice.clone())) }
}

/// # Safety
/// `q` is NULL or live; `name` is NULL or NUL-terminated; `rgba` is NULL or `width` × `height`
/// × 4 bytes, read during the call only (it is copied).
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_badge(
    q: *mut NibToasts,
    name: *const c_char,
    rgba: *const u8,
    width: c_int,
    height: c_int,
) {
    // SAFETY: the caller's promises.
    let (name, badge) = unsafe { (crate::text(name), crate::picture(rgba, width, height)) };
    if let Some((px, w, h)) = badge {
        // SAFETY: as above.
        unsafe { with(q, (), |q| q.queue.badge(&name, px.to_vec(), w, h)) }
    }
}

/// # Safety
/// `q` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_tick(q: *mut NibToasts, now_ms: u64) -> u64 {
    // SAFETY: the caller's promise.
    unsafe {
        with(q, 0, |q| {
            let now = instant(&mut q.clock, now_ms);
            q.queue.tick(now).map_or(0, |s| s.serial())
        })
    }
}

/// # Safety
/// `q` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_shown_id(q: *const NibToasts) -> u64 {
    // SAFETY: the caller's promise.
    unsafe { q.as_ref() }
        .and_then(|q| q.queue.shown())
        .map_or(0, |s| s.id())
}

/// The shown toast drawn for a `frame_w` x `frame_h` frame (once, then kept), and where on it
/// it goes; NULL when none is shown.
///
/// # Safety
/// `q` is NULL or live; the out pointers are NULL or writable. The result stays valid until the
/// next call on `q`.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_render(
    q: *mut NibToasts,
    frame_w: c_int,
    frame_h: c_int,
    width: *mut c_int,
    height: *mut c_int,
    x: *mut c_int,
    y: *mut c_int,
) -> *const u8 {
    let frame = (dim(frame_w), dim(frame_h));
    // SAFETY: the caller's promise.
    unsafe {
        with(q, std::ptr::null(), |q| {
            let fonts = q.fonts.get_or_insert_with(Fonts::new);
            let Some(drawn) = q.queue.draw(fonts, scale_for(frame.0, frame.1)) else {
                return std::ptr::null();
            };
            // SAFETY: the caller's promise that these are NULL or writable.
            hand_over(drawn, drawn.place(frame), (width, height, x, y))
        })
    }
}

/// # Safety
/// `q` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_toasts_clear(q: *mut NibToasts) {
    // SAFETY: the caller's promise.
    unsafe { with(q, (), |q| q.queue.clear()) }
}

#[no_mangle]
pub extern "C" fn nib_indicators_new() -> *mut NibIndicators {
    Box::into_raw(Box::default())
}

/// # Safety
/// `i` is NULL or came from `nib_indicators_new` and isn't used after.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_free(i: *mut NibIndicators) {
    if !i.is_null() {
        // SAFETY: the caller's promise.
        drop(unsafe { Box::from_raw(i) });
    }
}

/// # Safety
/// `i` and `t` are NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_progress(
    i: *mut NibIndicators,
    id: u32,
    t: *const NibToast,
) {
    // SAFETY: the caller's promise.
    let Some(t) = (unsafe { t.as_ref() }) else {
        return;
    };
    // SAFETY: as above.
    unsafe { with(i, (), |i| i.up.progress(id, t.notice.clone())) }
}

/// # Safety
/// `i` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_hide_progress(i: *mut NibIndicators) {
    // SAFETY: the caller's promise.
    unsafe { with(i, (), |i| i.up.hide_progress()) }
}

/// # Safety
/// `i` and `t` are NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_challenge(
    i: *mut NibIndicators,
    id: u32,
    t: *const NibToast,
) {
    // SAFETY: the caller's promise.
    let Some(t) = (unsafe { t.as_ref() }) else {
        return;
    };
    // SAFETY: as above.
    unsafe { with(i, (), |i| i.up.challenge(id, t.notice.clone())) }
}

/// # Safety
/// `i` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_end_challenge(i: *mut NibIndicators, id: u32) {
    // SAFETY: the caller's promise.
    unsafe { with(i, (), |i| i.up.end_challenge(id)) }
}

/// # Safety
/// `i` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_clear(i: *mut NibIndicators) {
    // SAFETY: the caller's promise.
    unsafe { with(i, (), |i| i.up.clear()) }
}

/// # Safety
/// As `nib_toasts_badge`.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_badge(
    i: *mut NibIndicators,
    name: *const c_char,
    rgba: *const u8,
    width: c_int,
    height: c_int,
) {
    // SAFETY: the caller's promises.
    let (name, badge) = unsafe { (crate::text(name), crate::picture(rgba, width, height)) };
    if let Some((px, w, h)) = badge {
        // SAFETY: as above.
        unsafe { with(i, (), |i| i.up.badge(&name, px.to_vec(), w, h)) }
    }
}

/// # Safety
/// `i` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_tick(i: *mut NibIndicators, now_ms: u64) -> u64 {
    // SAFETY: the caller's promise.
    unsafe {
        with(i, 0, |i| {
            let now = instant(&mut i.clock, now_ms);
            i.up.tick(now)
        })
    }
}

/// # Safety
/// `i` is NULL or live.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_pulsing(i: *const NibIndicators) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { i.as_ref() }.map_or(0, |i| c_int::from(i.up.pulsing()))
}

/// The indicators as the last tick found them, drawn for a `frame_w` x `frame_h` frame and
/// stacked in one picture, and where on it it goes; NULL for none.
///
/// # Safety
/// `i` is NULL or live; the out pointers are NULL or writable. The result stays valid until the
/// next call on `i`.
#[no_mangle]
pub unsafe extern "C" fn nib_indicators_render(
    i: *mut NibIndicators,
    frame_w: c_int,
    frame_h: c_int,
    width: *mut c_int,
    height: *mut c_int,
    x: *mut c_int,
    y: *mut c_int,
) -> *const u8 {
    let frame = (dim(frame_w), dim(frame_h));
    // SAFETY: the caller's promise.
    unsafe {
        with(i, std::ptr::null(), |i| {
            let fonts = i.fonts.get_or_insert_with(Fonts::new);
            let Some(s) = i.up.draw(fonts, frame) else {
                return std::ptr::null();
            };
            let (sx, sy) = s.place();
            // SAFETY: the caller's promise that these are NULL or writable.
            put(width, s.width());
            put(height, s.height());
            put(x, sx);
            put(y, sy);
            s.rgba().as_ptr()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nibkit::menu::toast::Said;
    use std::ffi::CString;
    use std::ptr::null_mut;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    /// The toast C fills and draws is byte for byte the one nibkit draws from the same words,
    /// at the frame's scale, where nibkit places it.
    #[test]
    fn render_hands_over_nibkits_picture() {
        let t = nib_toast_new();
        let (head, title, detail) = (
            c("Achievement unlocked"),
            c("First Blood"),
            c("Win a battle"),
        );
        let badge = [200u8, 90, 40, 255].repeat(8 * 8);
        let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
        // SAFETY: a live toast, NUL-terminated strings, an 8 x 8 badge, writable outs.
        let px = unsafe {
            nib_toast_begin(t, UNLOCK, title.as_ptr());
            nib_toast_header(t, head.as_ptr());
            nib_toast_detail(t, detail.as_ptr());
            nib_toast_points(t, 25);
            nib_toast_badge(t, badge.as_ptr(), 8, 8);
            nib_toast_render(t, 2048, 1536, &mut w, &mut h, &mut x, &mut y)
        };
        let f = Fonts::new();
        let said = Said::unlock("First Blood", "Win a battle", 25).badge(&badge, 8, 8);
        let want = toast::draw(&f, &said, 2.0);
        assert_eq!((w as usize, h as usize), (want.width(), want.height()));
        assert_eq!((x as usize, y as usize), want.place((2048, 1536)));
        // SAFETY: the picture is w x h x 4 bytes until the next call on `t`.
        let got = unsafe { std::slice::from_raw_parts(px, want.rgba().len()) };
        assert_eq!(got, want.rgba());
        // nib_toast_unlock is the same toast; the outs may be NULL.
        let px = unsafe {
            nib_toast_unlock(t, title.as_ptr(), detail.as_ptr(), 25);
            nib_toast_badge(t, badge.as_ptr(), 8, 8);
            nib_toast_render(
                t,
                2048,
                1536,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        // SAFETY: as above.
        let got = unsafe { std::slice::from_raw_parts(px, want.rgba().len()) };
        assert_eq!(got, want.rgba());
        // SAFETY: from nib_toast_new, not used after.
        unsafe { nib_toast_free(t) };
    }

    #[test]
    fn the_queue_shows_toasts_in_turn_by_the_callers_clock() {
        let q = nib_toasts_new();
        let t = nib_toast_new();
        let (one, two) = (c("One"), c("Two"));
        // SAFETY: live handles, NUL-terminated strings, writable outs.
        unsafe {
            nib_toast_begin(t, INFO, one.as_ptr());
            let a = nib_toasts_push(q, t);
            nib_toast_begin(t, PROBLEM, two.as_ptr());
            nib_toast_shows(t, 1000);
            let b = nib_toasts_push(q, t);
            assert!(a != 0 && b != a);
            // The caller's clock starts anywhere.
            let start = 50_000;
            let first = nib_toasts_tick(q, start);
            assert_ne!(first, 0);
            assert_eq!(nib_toasts_shown_id(q), a);
            assert_eq!(nib_toasts_tick(q, start + 2499), first);
            let second = nib_toasts_tick(q, start + 2500);
            assert!(second != 0 && second != first);
            assert_eq!(nib_toasts_shown_id(q), b);
            let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
            let px = nib_toasts_render(q, 1024, 768, &mut w, &mut h, &mut x, &mut y);
            let want = toast::draw(&Fonts::new(), &Said::new(Kind::Problem, "Two"), 1.0);
            assert_eq!(
                std::slice::from_raw_parts(px, want.rgba().len()),
                want.rgba()
            );
            assert_eq!((x as usize, y as usize), want.place((1024, 768)));
            assert_eq!(nib_toasts_tick(q, start + 3500), 0);
            assert!(nib_toasts_render(q, 1024, 768, &mut w, &mut h, &mut x, &mut y).is_null());
            assert_eq!(nib_toasts_shown_id(q), 0);
            nib_toast_free(t);
            nib_toasts_free(q);
        }
    }

    #[test]
    fn an_unlock_in_the_queue_waits_for_its_badge_by_name() {
        let q = nib_toasts_new();
        let t = nib_toast_new();
        let (title, name) = (c("First Blood"), c("12345"));
        let badge = [9u8; 4 * 4 * 4];
        // SAFETY: live handles, NUL-terminated strings, a 4 x 4 badge.
        unsafe {
            nib_toast_unlock(t, title.as_ptr(), std::ptr::null(), 10);
            nib_toast_await_badge(t, name.as_ptr());
            nib_toasts_push(q, t);
            assert_eq!(nib_toasts_tick(q, 0), 0, "waiting for the badge");
            nib_toasts_badge(q, name.as_ptr(), badge.as_ptr(), 4, 4);
            assert_ne!(nib_toasts_tick(q, 100), 0);
            nib_toasts_clear(q);
            assert_eq!(nib_toasts_tick(q, 200), 0);
            nib_toast_free(t);
            nib_toasts_free(q);
        }
    }

    #[test]
    fn null_handles_do_nothing() {
        // SAFETY: NULL is allowed everywhere.
        unsafe {
            nib_toast_begin(null_mut(), INFO, std::ptr::null());
            nib_toast_points(null_mut(), 3);
            assert!(nib_toast_render(
                null_mut(),
                1024,
                768,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut()
            )
            .is_null());
            assert_eq!(nib_toasts_push(null_mut(), std::ptr::null()), 0);
            assert_eq!(nib_toasts_tick(null_mut(), 0), 0);
            assert_eq!(nib_toasts_shown_id(std::ptr::null()), 0);
            nib_toasts_clear(null_mut());
            nib_toast_free(null_mut());
            nib_toasts_free(null_mut());
        }
    }

    #[test]
    fn kinds_from_c() {
        assert_eq!(kind_from(INFO), Kind::Info);
        assert_eq!(kind_from(UNLOCK), Kind::Unlock);
        assert_eq!(kind_from(PROBLEM), Kind::Problem);
        assert_eq!(kind_from(PROGRESS), Kind::Progress);
        assert_eq!(kind_from(CHALLENGE), Kind::Challenge);
        assert_eq!(kind_from(7), Kind::Info);
    }

    /// The header's version and kinds are the library's.
    #[test]
    fn the_header_says_what_the_library_is() {
        let header = include_str!("../include/nibkit_menu.h");
        let define = |name: &str| {
            let line = header
                .lines()
                .find(|l| l.split_whitespace().nth(1) == Some(name))
                .unwrap_or_else(|| panic!("{name} in the header"));
            line.split_whitespace().nth(2).unwrap().to_owned()
        };
        assert_eq!(define("NIBKIT_MENU_VERSION"), VERSION.to_string());
        assert_eq!(nib_menu_version(), VERSION);
        for (name, v) in [
            ("NIB_TOAST_INFO", INFO),
            ("NIB_TOAST_UNLOCK", UNLOCK),
            ("NIB_TOAST_PROBLEM", PROBLEM),
            ("NIB_TOAST_PROGRESS", PROGRESS),
            ("NIB_TOAST_CHALLENGE", CHALLENGE),
        ] {
            assert!(header.contains(&format!("{name} = {v},")), "{name}");
        }
    }

    #[test]
    fn a_frames_scale_from_c() {
        assert_eq!(nib_menu_scale_for(1024, 768), 1.0);
        assert_eq!(nib_menu_scale_for(512, 384), 0.5);
        assert_eq!(nib_menu_scale_for(-5, 768), scale_for(0, 768));
    }

    /// A challenge drawn from C, its badge's square, and its ring painted from C: byte for byte
    /// nibkit's.
    #[test]
    fn a_challenge_and_its_ring_are_nibkits() {
        let t = nib_toast_new();
        let title = c("No Damage");
        let badge = [200u8, 90, 40, 255].repeat(8 * 8);
        let (mut w, mut h) = (0, 0);
        let (mut bx, mut by, mut bw, mut bh) = (0, 0, 0, 0);
        // SAFETY: a live toast, a NUL-terminated string, an 8 x 8 badge, writable outs.
        let px = unsafe {
            nib_toast_challenge(t, title.as_ptr());
            nib_toast_badge(t, badge.as_ptr(), 8, 8);
            let px = nib_toast_render(t, 1536, 1152, &mut w, &mut h, null_mut(), null_mut());
            assert_eq!(
                nib_toast_badge_rect(t, &mut bx, &mut by, &mut bw, &mut bh),
                1
            );
            px
        };
        let f = Fonts::new();
        let want = toast::draw(&f, &Said::challenge("No Damage").badge(&badge, 8, 8), 1.5);
        let rect = want.badge_rect().unwrap();
        assert_eq!(
            (bx, by, bw, bh),
            (
                rect.0 as c_int,
                rect.1 as c_int,
                rect.2 as c_int,
                rect.3 as c_int
            )
        );
        // SAFETY: the picture is w x h x 4 bytes until the next call on `t`.
        let mut got = unsafe { std::slice::from_raw_parts(px, want.rgba().len()) }.to_vec();
        assert_eq!(got, want.rgba());
        let mut ring = want.rgba().to_vec();
        toast::pulse(&mut ring, (want.width(), want.height()), rect, 1.5, 0.6);
        // SAFETY: `got` is w x h x 4 bytes.
        unsafe { nib_toast_pulse(got.as_mut_ptr(), w, h, bx, by, bw, bh, 1.5, 0.6) };
        assert_eq!(got, ring);
        assert_ne!(got, want.rgba());
        assert_eq!(
            nib_toast_pulse_strength(900),
            toast::pulse_strength(Duration::from_millis(900))
        );
        // Without a badge, no square.
        // SAFETY: as above; NULL outs.
        unsafe {
            nib_toast_begin(t, CHALLENGE, title.as_ptr());
            nib_toast_render(t, 1024, 768, null_mut(), null_mut(), null_mut(), null_mut());
            assert_eq!(
                nib_toast_badge_rect(t, null_mut(), null_mut(), null_mut(), null_mut()),
                0
            );
            nib_toast_free(t);
        }
    }

    #[test]
    fn the_stack_from_c_is_nibkits() {
        let sizes = [300, 250, 300, 250, 300, 250, 300, 250];
        let mut places = [7; 8];
        // SAFETY: four pairs each.
        let kept = unsafe { nib_toast_stack(1024, 768, 4, sizes.as_ptr(), 1, places.as_mut_ptr()) };
        assert_eq!(kept, 2);
        let want = toast::stack((1024, 768), 1.0, &[(300, 250); 4], 1);
        let want: Vec<c_int> = want
            .iter()
            .flat_map(|a| a.map_or([-1, -1], |(x, y)| [x as c_int, y as c_int]))
            .collect();
        assert_eq!(places.to_vec(), want);
        // SAFETY: NULL is allowed.
        assert_eq!(
            unsafe { nib_toast_stack(1024, 768, 2, std::ptr::null(), 0, places.as_mut_ptr()) },
            0
        );
    }

    /// The indicators C keeps and draws are nibkit's, byte for byte, at the frame's scale and
    /// where nibkit stacks them.
    #[test]
    fn the_indicators_from_c_are_nibkits() {
        let i = nib_indicators_new();
        let t = nib_toast_new();
        let (title, measured, other, name) =
            (c("Collector"), c("42/100"), c("No Damage"), c("555"));
        let badge = [9u8, 99, 199, 255].repeat(4 * 4);
        let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
        let mut rust = Indicators::new();
        let f = Fonts::new();
        let then = Instant::now();
        // SAFETY: live handles, NUL-terminated strings, a 4 x 4 badge, writable outs.
        unsafe {
            nib_toast_progress(t, title.as_ptr(), measured.as_ptr());
            nib_toast_badge(t, badge.as_ptr(), 4, 4);
            nib_indicators_progress(i, 1, t);
            nib_toast_challenge(t, other.as_ptr());
            nib_toast_await_badge(t, name.as_ptr());
            nib_indicators_challenge(i, 2, t);
            nib_indicators_badge(i, name.as_ptr(), badge.as_ptr(), 4, 4);
            rust.progress(
                1,
                Notice::progress("Collector", "42/100").badge(badge.clone(), 4, 4),
            );
            rust.challenge(2, Notice::challenge("No Damage").awaits_badge("555"));
            rust.badge("555", badge.clone(), 4, 4);
            // The same step of the ring on both clocks.
            assert_ne!(nib_indicators_tick(i, 10_000), 0);
            assert_eq!(nib_indicators_pulsing(i), 1);
            assert_ne!(rust.tick(then), 0);
            let px = nib_indicators_render(i, 1024, 768, &mut w, &mut h, &mut x, &mut y);
            let want = rust.draw(&f, (1024, 768)).unwrap();
            assert_eq!((w as usize, h as usize), (want.width(), want.height()));
            assert_eq!((x as usize, y as usize), want.place());
            assert_eq!(
                std::slice::from_raw_parts(px, want.rgba().len()),
                want.rgba()
            );
            nib_indicators_end_challenge(i, 2);
            nib_indicators_hide_progress(i);
            assert_eq!(nib_indicators_tick(i, 10_100), 0);
            assert!(nib_indicators_render(i, 1024, 768, &mut w, &mut h, &mut x, &mut y).is_null());
            nib_toast_challenge(t, other.as_ptr());
            nib_indicators_challenge(i, 3, t);
            nib_indicators_clear(i);
            assert_eq!(nib_indicators_tick(i, 10_200), 0);
            // NULL handles do nothing.
            nib_indicators_progress(null_mut(), 1, t);
            nib_indicators_challenge(i, 1, std::ptr::null());
            assert_eq!(nib_indicators_tick(null_mut(), 0), 0);
            assert_eq!(nib_indicators_pulsing(std::ptr::null()), 0);
            nib_indicators_free(null_mut());
            nib_toast_free(t);
            nib_indicators_free(i);
        }
    }
}
