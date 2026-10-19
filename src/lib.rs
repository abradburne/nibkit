//! nibkit's menu for C and C++ apps (include/nibkit_menu.h): DSlot's menu, filled in a call at a
//! time, answering the buttons and touches, drawn into an RGBA picture.

use nibkit::menu::raster::{Canvas, Fonts};
use nibkit::menu::{layout, top, Card, Draw, Effect, Key, Menu, Row, Shelf, Slot, Status, Value};
use std::ffi::{c_char, c_float, c_int, CStr};

/// A menu's state, behind the C header's opaque `NibMenu`.
pub struct NibMenu {
    menu: Menu,
    shelf: Shelf,
    pictures: Vec<(Vec<u8>, usize, usize)>,
    status: Status,
    fonts: Fonts,
    canvas: Canvas,
    /// The top screen's picture, while the menu is open.
    top: Canvas,
}

/// `NibEffect`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NibEffect {
    pub kind: c_int,
    pub index: c_int,
    pub step: c_int,
    pub key: c_int,
}

const NONE: NibEffect = NibEffect {
    kind: 0,
    index: 0,
    step: 0,
    key: 0,
};

fn key_of(k: c_int) -> Option<Key> {
    Some(match k {
        0 => Key::Up,
        1 => Key::Down,
        2 => Key::Left,
        3 => Key::Right,
        4 => Key::A,
        5 => Key::B,
        6 => Key::X,
        7 => Key::Y,
        8 => Key::L,
        9 => Key::R,
        _ => return None,
    })
}

fn key_code(k: Key) -> c_int {
    match k {
        Key::Up => 0,
        Key::Down => 1,
        Key::Left => 2,
        Key::Right => 3,
        Key::A => 4,
        Key::B => 5,
        Key::X => 6,
        Key::Y => 7,
        Key::L => 8,
        Key::R => 9,
    }
}

fn effect(e: Option<Effect>) -> NibEffect {
    let mk = |kind, index: usize, step, key| NibEffect {
        kind,
        index: index as c_int,
        step,
        key,
    };
    match e {
        None => NONE,
        Some(Effect::Change { row, step }) => mk(1, row, step, 0),
        Some(Effect::Choose(row)) => mk(2, row, 0, 0),
        Some(Effect::Use(row)) => mk(3, row, 0, 0),
        Some(Effect::Page(tab)) => mk(4, tab, 0, 0),
        Some(Effect::Back) => mk(5, 0, 0, 0),
        Some(Effect::Card { at, key }) => mk(6, at, 0, key_code(key)),
    }
}

/// A C string as text: empty for NULL or bad UTF-8 (lossy).
///
/// # Safety
/// `s` is NULL or points at a NUL-terminated string.
unsafe fn text(s: *const c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: the caller's promise.
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

impl NibMenu {
    fn shelf_mode(&self) -> bool {
        !self.shelf.cards.is_empty()
    }

    /// Keeps the highlight on a row (or card) that exists, in view.
    fn settle(&mut self) {
        if self.menu.row >= self.menu.rows.len() {
            self.menu.row = self.menu.rows.len().saturating_sub(1);
        }
        self.menu.settle();
        if self.shelf.at >= self.shelf.cards.len() {
            self.shelf.at = self.shelf.cards.len().saturating_sub(1);
        }
    }
}

/// # Safety
/// Free the result with `nib_menu_free`, once.
#[no_mangle]
pub extern "C" fn nib_menu_new() -> *mut NibMenu {
    Box::into_raw(Box::new(NibMenu {
        menu: Menu::default(),
        shelf: Shelf::default(),
        pictures: Vec::new(),
        status: Status::default(),
        fonts: Fonts::new(),
        canvas: Canvas::new(1.0),
        top: Canvas::new(1.0),
    }))
}

/// # Safety
/// `m` came from `nib_menu_new` and isn't used again (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn nib_menu_free(m: *mut NibMenu) {
    if !m.is_null() {
        // SAFETY: the caller's promise: it is ours, from Box::into_raw.
        drop(unsafe { Box::from_raw(m) });
    }
}

/// Runs `f` on the menu behind `m`, if it isn't NULL.
///
/// # Safety
/// `m` is NULL or a live `NibMenu` not used elsewhere during the call.
unsafe fn with<T>(m: *mut NibMenu, or: T, f: impl FnOnce(&mut NibMenu) -> T) -> T {
    // SAFETY: the caller's promise.
    match unsafe { m.as_mut() } {
        Some(m) => f(m),
        None => or,
    }
}

/// # Safety
/// `m` is a live `NibMenu`; the strings are NULL or NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_begin(
    m: *mut NibMenu,
    title: *const c_char,
    hint: *const c_char,
) {
    // SAFETY: the caller's promise, passed on.
    let (title, hint) = unsafe { (text(title), text(hint)) };
    // SAFETY: as above.
    unsafe {
        with(m, (), |m| {
            m.menu.tabs.clear();
            m.menu.rows.clear();
            m.menu.title = title;
            m.menu.hint = hint.clone();
            m.shelf.tabs.clear();
            m.shelf.cards.clear();
            m.shelf.hint = hint;
            m.pictures.clear();
        })
    }
}

/// # Safety
/// As `nib_menu_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_tab(m: *mut NibMenu, label: *const c_char, selected: c_int) {
    // SAFETY: the caller's promise.
    let label = unsafe { text(label) };
    // SAFETY: as above.
    unsafe {
        with(m, (), |m| {
            m.menu.tabs.push((label.clone(), selected != 0));
            m.shelf.tabs.push((label, selected != 0));
        })
    }
}

/// # Safety
/// As `nib_menu_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_heading(m: *mut NibMenu, label: *const c_char) {
    // SAFETY: the caller's promise.
    let label = unsafe { text(label) };
    // SAFETY: as above.
    unsafe { with(m, (), |m| m.menu.rows.push(Row::heading(label))) }
}

/// # Safety
/// As `nib_menu_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_row(
    m: *mut NibMenu,
    label: *const c_char,
    kind: c_int,
    value: *const c_char,
    number: c_int,
) {
    // SAFETY: the caller's promise.
    let (label, value) = unsafe { (text(label), text(value)) };
    let n = number.clamp(0, 100) as u8;
    let value = match kind {
        1 => Value::Text(value),
        2 => Value::Inherited(value),
        3 => Value::More(value),
        4 => Value::Stars(n.min(5)),
        5 => Value::Slider(n),
        6 => Value::Prompt {
            text: value,
            warning: false,
        },
        7 => Value::Prompt {
            text: value,
            warning: true,
        },
        _ => Value::None,
    };
    // SAFETY: as above.
    unsafe { with(m, (), |m| m.menu.rows.push(Row::new(label, value))) }
}

/// # Safety
/// As `nib_menu_begin`; `rgba` is NULL or `width` × `height` × 4 bytes.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_card(
    m: *mut NibMenu,
    label: *const c_char,
    rgba: *const u8,
    width: c_int,
    height: c_int,
) {
    // SAFETY: the caller's promise.
    let label = unsafe { text(label) };
    let (w, h) = (width.max(0) as usize, height.max(0) as usize);
    let picture = (!rgba.is_null() && w > 0 && h > 0)
        // SAFETY: the caller's promise about rgba's size.
        .then(|| unsafe { std::slice::from_raw_parts(rgba, w * h * 4) }.to_vec());
    // SAFETY: as above.
    unsafe {
        with(m, (), |m| {
            let index = picture.map(|p| {
                m.pictures.push((p, w, h));
                m.pictures.len() - 1
            });
            m.shelf.cards.push(Card {
                picture: index,
                label,
            });
        })
    }
}

/// # Safety
/// As `nib_menu_begin`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_status(
    m: *mut NibMenu,
    clock: *const c_char,
    battery: c_int,
    charging: c_int,
    slot: c_int,
    cart_width: c_float,
) {
    // SAFETY: the caller's promise.
    let clock = unsafe { text(clock) };
    // SAFETY: as above.
    unsafe {
        with(m, (), |m| {
            m.status = Status {
                clock,
                battery: (battery >= 0).then(|| (battery.min(100) as u8, charging != 0)),
                slot: if slot == 1 {
                    Slot::One
                } else {
                    Slot::Two(if cart_width > 0.0 { cart_width } else { 560.0 })
                },
            }
        })
    }
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_row_at(m: *const NibMenu) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { m.as_ref() }.map_or(0, |m| m.menu.row as c_int)
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_set_row(m: *mut NibMenu, row: c_int) {
    // SAFETY: the caller's promise.
    unsafe {
        with(m, (), |m| {
            m.menu.row = row.max(0) as usize;
            m.settle();
        })
    }
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_card_at(m: *const NibMenu) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { m.as_ref() }.map_or(0, |m| m.shelf.at as c_int)
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_set_card(m: *mut NibMenu, card: c_int) {
    // SAFETY: the caller's promise.
    unsafe {
        with(m, (), |m| {
            m.shelf.at = card.max(0) as usize;
            m.settle();
        })
    }
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_key(m: *mut NibMenu, key: c_int) -> NibEffect {
    let Some(key) = key_of(key) else { return NONE };
    // SAFETY: the caller's promise.
    unsafe {
        with(m, NONE, |m| {
            m.settle();
            if m.shelf_mode() {
                effect(m.shelf.key(key))
            } else {
                effect(m.menu.key(key))
            }
        })
    }
}

/// # Safety
/// `m` is a live `NibMenu`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_touch(m: *mut NibMenu, x: c_float, y: c_float) -> NibEffect {
    // SAFETY: the caller's promise.
    unsafe {
        with(m, NONE, |m| {
            m.settle();
            if m.shelf_mode() {
                effect(m.shelf.touch(x, y))
            } else {
                effect(m.menu.touch(x, y))
            }
        })
    }
}

/// # Safety
/// `m` is a live `NibMenu`; `width` and `height` are NULL or writable. The result stays valid
/// until the next call on `m`.
#[no_mangle]
pub unsafe extern "C" fn nib_menu_render(
    m: *mut NibMenu,
    width: *mut c_int,
    height: *mut c_int,
) -> *const u8 {
    // SAFETY: the caller's promise.
    let Some(m) = (unsafe { m.as_mut() }) else {
        return std::ptr::null();
    };
    m.settle();
    let draws = if m.shelf_mode() {
        layout::shelf(&m.shelf, &m.status)
    } else {
        layout::menu(&m.menu, &m.status)
    };
    m.canvas.clear(nibkit::menu::colour::BLACK);
    let pictures = &m.pictures;
    m.canvas.draw(&m.fonts, &draws, |i| {
        pictures.get(i).map(|(p, w, h)| (&p[..], *w, *h))
    });
    // SAFETY: the caller's promise that they are NULL or writable.
    unsafe {
        if let Some(w) = width.as_mut() {
            *w = m.canvas.w as c_int;
        }
        if let Some(h) = height.as_mut() {
            *h = m.canvas.h as c_int;
        }
    }
    m.canvas.px.as_ptr()
}

/// A picture handed over from C, if it is there: its bytes, width and height.
///
/// # Safety
/// `rgba` is NULL or `w` × `h` × 4 bytes.
unsafe fn picture<'a>(rgba: *const u8, w: c_int, h: c_int) -> Option<(&'a [u8], usize, usize)> {
    let (w, h) = (w.max(0) as usize, h.max(0) as usize);
    // SAFETY: the caller's promise.
    (!rgba.is_null() && w > 0 && h > 0)
        .then(|| (unsafe { std::slice::from_raw_parts(rgba, w * h * 4) }, w, h))
}

fn size_out(c: &Canvas, width: *mut c_int, height: *mut c_int) {
    // SAFETY: the callers pass on their callers' promise that these are NULL or writable.
    unsafe {
        if let Some(w) = width.as_mut() {
            *w = c.w as c_int;
        }
        if let Some(h) = height.as_mut() {
            *h = c.h as c_int;
        }
    }
}

/// The paused top screen: `game` scaled (nearest pixel, keeping its shape) to fill the screen,
/// dimmed, with `title` large and `subtitle` under it.
///
/// # Safety
/// `m` is a live `NibMenu`; `game` is NULL or `gw` × `gh` × 4 bytes; the strings are NULL or
/// NUL-terminated; `width` and `height` are NULL or writable. The result stays valid until the
/// next call on `m`.
#[no_mangle]
pub unsafe extern "C" fn nib_top_paused(
    m: *mut NibMenu,
    game: *const u8,
    gw: c_int,
    gh: c_int,
    title: *const c_char,
    subtitle: *const c_char,
    width: *mut c_int,
    height: *mut c_int,
) -> *const u8 {
    // SAFETY: the caller's promises, passed on.
    let (Some(m), game, title, subtitle) = (
        unsafe { m.as_mut() },
        unsafe { picture(game, gw, gh) },
        unsafe { text(title) },
        unsafe { text(subtitle) },
    ) else {
        return std::ptr::null();
    };
    let c = &mut m.top;
    c.clear(nibkit::menu::colour::BLACK);
    if let Some((px, w, h)) = game {
        let k = (nibkit::menu::W / w as f32).min(nibkit::menu::H / h as f32);
        let (dw, dh) = (w as f32 * k, h as f32 * k);
        let place = Draw::Picture {
            index: 0,
            x: (nibkit::menu::W - dw) / 2.0,
            y: (nibkit::menu::H - dh) / 2.0,
            w: dw,
            h: dh,
            faded: false,
        };
        c.draw(&m.fonts, &[place], |_| Some((px, w, h)));
        for p in c.px.chunks_exact_mut(4) {
            for v in &mut p[..3] {
                *v = (*v as f32 * top::DIM) as u8;
            }
        }
    }
    c.draw(&m.fonts, &top::paused(&title, &subtitle), |_| None);
    size_out(c, width, height);
    c.px.as_ptr()
}

/// The top screen on a shelf of save states: the highlighted one's `picture` large (NULL for an
/// empty slot), its `name` beside it, `under` that ("Quick slot", or NULL), then `n` lines of
/// `labels[i]` and `values[i]` ("Saved", "Today 4:41 PM").
///
/// # Safety
/// As `nib_top_paused`; `labels` and `values` are NULL or hold `n` strings each.
#[no_mangle]
pub unsafe extern "C" fn nib_top_state(
    m: *mut NibMenu,
    picture_rgba: *const u8,
    pw: c_int,
    ph: c_int,
    name: *const c_char,
    under: *const c_char,
    labels: *const *const c_char,
    values: *const *const c_char,
    n: c_int,
    width: *mut c_int,
    height: *mut c_int,
) -> *const u8 {
    // SAFETY: the caller's promises, passed on.
    let Some(m) = (unsafe { m.as_mut() }) else {
        return std::ptr::null();
    };
    // SAFETY: as above.
    let (pic, name, under) = unsafe { (picture(picture_rgba, pw, ph), text(name), text(under)) };
    let n = if labels.is_null() || values.is_null() {
        0
    } else {
        n.max(0) as usize
    };
    // SAFETY: the caller's promise that each array holds n strings.
    let lines: Vec<(String, String)> = (0..n)
        .map(|i| unsafe { (text(*labels.add(i)), text(*values.add(i))) })
        .collect();
    let lines: Vec<(&str, &str)> = lines
        .iter()
        .map(|(l, v)| (l.as_str(), v.as_str()))
        .collect();
    let c = &mut m.top;
    c.clear(nibkit::menu::colour::BLACK);
    c.draw(
        &m.fonts,
        &top::state(&name, &under, &lines, pic.map(|_| 0)),
        |_| pic,
    );
    size_out(c, width, height);
    c.px.as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    /// The C calls, as an app makes them: a page filled, the highlight moved and kept across a
    /// refill, a value stepped, a tab turned, drawn.
    #[test]
    fn a_page_through_the_c_calls() {
        let m = nib_menu_new();
        let fill = |fast: &str| unsafe {
            nib_menu_begin(m, c("MOLE PATROL").as_ptr(), c("A  choose").as_ptr());
            nib_menu_tab(m, c("Play").as_ptr(), 1);
            nib_menu_tab(m, c("States").as_ptr(), 0);
            nib_menu_heading(m, c("PLAY").as_ptr());
            nib_menu_row(m, c("Resume").as_ptr(), 0, std::ptr::null(), 0);
            nib_menu_row(m, c("Fast-forward").as_ptr(), 1, c(fast).as_ptr(), 0);
            nib_menu_row(m, c("Volume").as_ptr(), 5, std::ptr::null(), 70);
            nib_menu_status(m, c("5:05 PM").as_ptr(), 76, 0, 1, 0.0);
        };
        fill("Off");
        unsafe {
            assert_eq!(nib_menu_key(m, 1).kind, 0);
            assert_eq!(nib_menu_row_at(m), 2, "past the heading, one down");
            assert_eq!(
                nib_menu_key(m, 3),
                NibEffect {
                    kind: 1,
                    index: 2,
                    step: 1,
                    key: 0
                }
            );
        }
        fill("On");
        unsafe {
            assert_eq!(nib_menu_row_at(m), 2, "kept across the refill");
            assert_eq!(
                nib_menu_key(m, 9),
                NibEffect {
                    kind: 4,
                    index: 1,
                    step: 0,
                    key: 0
                }
            );
            let (mut w, mut h) = (0, 0);
            let px = nib_menu_render(m, &mut w, &mut h);
            assert_eq!((w, h), (1024, 768));
            let px = std::slice::from_raw_parts(px, (w * h * 4) as usize);
            assert_eq!(
                &px[(12 * 1024 + 30) * 4..][..3],
                &nibkit::menu::colour::WHITE,
                "the selected tab"
            );
            nib_menu_free(m);
        }
    }

    #[test]
    fn the_top_screens_through_the_c_calls() {
        let m = nib_menu_new();
        let game = vec![255u8; 256 * 192 * 4];
        let (mut w, mut h) = (0, 0);
        unsafe {
            let px = nib_top_paused(
                m,
                game.as_ptr(),
                256,
                192,
                c("Paused").as_ptr(),
                c("Mole Patrol").as_ptr(),
                &mut w,
                &mut h,
            );
            let px = std::slice::from_raw_parts(px, (w * h * 4) as usize);
            assert_eq!((w, h), (1024, 768));
            // A DS screen fills it, dimmed: white shows as 45% grey away from the words.
            assert_eq!(px[(700 * 1024 + 20) * 4], (255.0 * top::DIM) as u8);
            let labels = [c("Saved"), c("Played")];
            let values = [c("Today 4:41 PM"), c("1 h 23 min")];
            let lp: Vec<_> = labels.iter().map(|s| s.as_ptr()).collect();
            let vp: Vec<_> = values.iter().map(|s| s.as_ptr()).collect();
            let shot = vec![90u8; 160 * 144 * 4];
            let px = nib_top_state(
                m,
                shot.as_ptr(),
                160,
                144,
                c("Slot 2").as_ptr(),
                c("Quick slot").as_ptr(),
                lp.as_ptr(),
                vp.as_ptr(),
                2,
                &mut w,
                &mut h,
            );
            let px = std::slice::from_raw_parts(px, (w * h * 4) as usize);
            assert_eq!(px[(300 * 1024 + 300) * 4], 90, "the picture, large");
            assert!(!nib_top_state(
                m,
                std::ptr::null(),
                0,
                0,
                c("Slot 4").as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            )
            .is_null());
            nib_menu_free(m);
        }
    }

    #[test]
    fn a_shelf_through_the_c_calls() {
        let m = nib_menu_new();
        let picture = vec![200u8; 160 * 144 * 4];
        unsafe {
            nib_menu_begin(m, std::ptr::null(), c("A  load").as_ptr());
            nib_menu_card(m, c("1").as_ptr(), picture.as_ptr(), 160, 144);
            nib_menu_card(m, c("2").as_ptr(), std::ptr::null(), 0, 0);
            assert_eq!(nib_menu_key(m, 3).kind, 0);
            assert_eq!(nib_menu_card_at(m), 1);
            assert_eq!(
                nib_menu_key(m, 6),
                NibEffect {
                    kind: 6,
                    index: 1,
                    step: 0,
                    key: 6
                }
            );
            let px = nib_menu_render(m, std::ptr::null_mut(), std::ptr::null_mut());
            assert!(!px.is_null());
            nib_menu_free(m);
            nib_menu_free(std::ptr::null_mut());
        }
    }
}
