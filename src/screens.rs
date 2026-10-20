//! The two panels. Each is 1024 x 768; the bottom one has the touch panel. How they appear to
//! SDL (or any library that lists displays) depends on the system:
//!
//! - **nibOS**: a display for each panel, side by side; the bottom panel (DSI-1) is the
//!   left-hand one, SDL's display 0. A window for each screen.
//! - **stock OS**: Weston shows one display as wide as both; the bottom panel is its right
//!   half. One window across it.
//!
//! [`place`] works out where each screen is from the displays' bounds, so an app opens its
//! windows without knowing which system it is on:
//!
//! ```
//! use nibkit::screens::{place, Rect, Screen};
//! // What SDL reports on nibOS: two 1024x768 displays side by side.
//! let displays = [Rect::new(0, 0, 1024, 768), Rect::new(1024, 0, 1024, 768)];
//! if let Some(placement) = place(&displays, false) {
//!     let bottom = placement.region(Screen::Bottom);
//!     println!("bottom screen: display {} at {:?}", bottom.display, bottom.rect);
//! }
//! ```

use crate::platform;

/// A screen's size, in pixels.
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 768;

/// The panels' refresh rate, in Hz: 59.94 measured on nibOS.
pub const REFRESH_HZ: f64 = platform::REFRESH_HZ;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Screen {
    Top,
    /// The one with the touch panel.
    Bottom,
}

/// How the panels appear as displays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layout {
    /// A display for each panel, the bottom one on the left.
    PerPanel,
    /// One display as wide as both, the bottom one on the right.
    Spanned,
}

/// This system's layout.
pub const LAYOUT: Layout = platform::LAYOUT;

/// A rectangle in pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    fn union(self, o: Rect) -> Rect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        let right = (self.x + self.w as i32).max(o.x + o.w as i32);
        let bottom = (self.y + self.h as i32).max(o.y + o.h as i32);
        Rect::new(x, y, (right - x) as u32, (bottom - y) as u32)
    }
}

/// Where a screen is: which display, and the part of it the screen covers (in the display
/// list's coordinates).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Region {
    pub display: usize,
    pub rect: Rect,
}

/// Where both screens are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Placement {
    pub top: Region,
    pub bottom: Region,
    /// Both on one display: one window across it is enough.
    pub shared: bool,
}

impl Placement {
    pub fn region(&self, screen: Screen) -> Region {
        match screen {
            Screen::Top => self.top,
            Screen::Bottom => self.bottom,
        }
    }
}

/// Where the top and bottom screens are among `displays` (SDL's display bounds, in its order),
/// the other way round if `swap`. `None` when the displays don't look like the two panels
/// (a desktop, where an app shows both screens in a window of its own).
pub fn place(displays: &[Rect], swap: bool) -> Option<Placement> {
    // On a desktop there are no panels, however the monitors are arranged.
    if cfg!(nibkit_desktop) {
        return None;
    }
    let first = *displays.first()?;
    let union = displays.iter().fold(first, |a, &b| a.union(b));
    // Both panels side by side, as against a desktop monitor.
    if (union.w as f32) / (union.h as f32) < 2.2 {
        return None;
    }
    let (left, right) = match LAYOUT {
        Layout::PerPanel => {
            let mut by_x: Vec<usize> = (0..displays.len()).collect();
            by_x.sort_by_key(|&i| displays[i].x);
            let (l, r) = (*by_x.first()?, *by_x.get(1)?);
            (
                Region {
                    display: l,
                    rect: displays[l],
                },
                Region {
                    display: r,
                    rect: displays[r],
                },
            )
        }
        Layout::Spanned => {
            let half = union.w / 2;
            let left = Rect::new(union.x, union.y, half, union.h);
            let right = Rect::new(union.x + half as i32, union.y, union.w - half, union.h);
            (
                Region {
                    display: 0,
                    rect: left,
                },
                Region {
                    display: 0,
                    rect: right,
                },
            )
        }
    };
    // The bottom panel is the left-hand display on nibOS, the right-hand half on the stock OS.
    let (top, bottom) = match LAYOUT {
        Layout::PerPanel => (right, left),
        Layout::Spanned => (left, right),
    };
    let (top, bottom) = if swap { (bottom, top) } else { (top, bottom) };
    Some(Placement {
        top,
        bottom,
        shared: top.display == bottom.display,
    })
}

/// How an app shows both screens on a desktop, in windows of its own (where [`place`] finds
/// no panels).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Windows {
    /// A window for each screen, both this size.
    Two { size: (u32, u32) },
    /// One window this size with both screens in it, the top one above the bottom one and a
    /// gap between, as the handheld holds them; each screen's place in the window.
    Stacked {
        size: (u32, u32),
        top: Rect,
        bottom: Rect,
    },
}

/// The gap between the screens in a [`Windows::Stacked`] window, at scale 1: the hinge.
pub const HINGE: u32 = 32;

/// Windows for both screens on a desktop at `scale` times the panels' size (1.0 is 1024 x
/// 768 each; 0.5 suits a laptop): one each, or one with both stacked.
///
/// ```
/// use nibkit::screens::{desktop_windows, Rect, Windows};
/// match desktop_windows(true, 0.5) {
///     Windows::Stacked { size, top, bottom } => {
///         assert_eq!(size, (512, 768 + 16));
///         assert_eq!((top, bottom), (Rect::new(0, 0, 512, 384), Rect::new(0, 400, 512, 384)));
///     }
///     Windows::Two { .. } => unreachable!(),
/// }
/// ```
pub fn desktop_windows(stacked: bool, scale: f32) -> Windows {
    let px = |v: u32| ((v as f32 * scale).round() as u32).max(1);
    let (w, h) = (px(WIDTH), px(HEIGHT));
    if !stacked {
        return Windows::Two { size: (w, h) };
    }
    let gap = px(HINGE);
    Windows::Stacked {
        size: (w, 2 * h + gap),
        top: Rect::new(0, 0, w, h),
        bottom: Rect::new(0, (h + gap) as i32, w, h),
    }
}

impl Rect {
    /// A point in the window's pixels as a point on the panel this rectangle shows (1024 x
    /// 768 units), or `None` outside it: the mouse on a desktop's bottom screen, as a touch.
    ///
    /// ```
    /// use nibkit::screens::Rect;
    /// let bottom = Rect::new(0, 400, 512, 384); // half size, under the top screen
    /// assert_eq!(bottom.panel_point(256, 592), Some((512.0, 384.0)));
    /// assert_eq!(bottom.panel_point(256, 100), None);
    /// ```
    pub fn panel_point(&self, x: i32, y: i32) -> Option<(f32, f32)> {
        let inside = (self.x..self.x + self.w as i32).contains(&x)
            && (self.y..self.y + self.h as i32).contains(&y);
        inside.then(|| {
            (
                (x - self.x) as f32 * WIDTH as f32 / self.w as f32,
                (y - self.y) as f32 * HEIGHT as f32 / self.h as f32,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop() {
        assert_eq!(
            desktop_windows(false, 1.0),
            Windows::Two { size: (1024, 768) }
        );
        let Windows::Stacked { size, top, bottom } = desktop_windows(true, 1.0) else {
            panic!("stacked");
        };
        assert_eq!(size, (1024, 768 * 2 + HINGE));
        assert_eq!(top.panel_point(0, 0), Some((0.0, 0.0)));
        assert_eq!(
            bottom.panel_point(1023, 767 + 768 + HINGE as i32),
            Some((1023.0, 767.0))
        );
        // The hinge is neither screen.
        assert_eq!(bottom.panel_point(10, 770), None);
        assert_eq!(top.panel_point(10, 770), None);
    }

    #[test]
    fn panels() {
        // A desktop: no placement, the app draws both screens itself.
        assert_eq!(place(&[Rect::new(0, 0, 1920, 1080)], false), None);
        assert_eq!(place(&[], false), None);
        if cfg!(nibkit_desktop) {
            let two = [Rect::new(0, 0, 1024, 768), Rect::new(1024, 0, 1024, 768)];
            assert_eq!(place(&two, false), None);
            return;
        }
        match LAYOUT {
            Layout::PerPanel => {
                // SDL's order on nibOS: display 0 is DSI-1, the bottom panel, on the left.
                let displays = [Rect::new(0, 0, 1024, 768), Rect::new(1024, 0, 1024, 768)];
                let p = place(&displays, false).unwrap();
                assert_eq!((p.bottom.display, p.top.display, p.shared), (0, 1, false));
                // Listed the other way round, still by position.
                let p = place(&[displays[1], displays[0]], false).unwrap();
                assert_eq!((p.bottom.display, p.top.display), (1, 0));
                let p = place(&displays, true).unwrap();
                assert_eq!((p.bottom.display, p.top.display), (1, 0));
            }
            Layout::Spanned => {
                let p = place(&[Rect::new(0, 0, 2048, 768)], false).unwrap();
                assert!(p.shared);
                assert_eq!(p.top.rect, Rect::new(0, 0, 1024, 768));
                assert_eq!(p.bottom.rect, Rect::new(1024, 0, 1024, 768));
            }
        }
    }
}
