//! Where everything goes: DSlot's menu, measured off its code (ui/draw.rs, ui/menu.rs), on a
//! 1024 × 768 screen. Text is placed by the top of its line box (the font's ascent plus 1 unit
//! over it, as DSlot renders a line) and an anchor along it.

use super::colour::{self, Rgb};
use super::model::{Menu, Shelf, Slot, Status, Value};
use super::{H, W};

/// The first row's centre, in screen units.
pub const TOP: f32 = 156.0;
/// How far apart rows are, in screen units.
pub const ROW_STEP: f32 = 46.0;
/// How many rows fit on the screen.
pub const VISIBLE: usize = 10;
/// The tabs: the first one's corner, and each one's size.
pub const TABS: (f32, f32, f32, f32) = (28.0, 10.0, 130.0, 40.0);
/// Where labels start, in screen units.
pub const LEFT: f32 = 150.0;
/// Where values end, in screen units.
pub const RIGHT: f32 = W - 150.0;
/// The height of the bar along the bottom, in screen units.
pub const BAR_H: f32 = 96.0;
/// The top of the hint line above the bar, in screen units.
pub const HINT_Y: f32 = 636.0;
/// A star's size, in screen units.
pub const ICON: f32 = 26.0;
/// A row's pictures' height (`Value::Status`), in screen units: the text's capitals.
pub const GLYPH: f32 = 26.0;
/// The battery icon's height, in screen units.
pub const BATTERY: f32 = 22.0;

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A filled rectangle.
    Rect {
        /// Its left edge.
        x: f32,
        /// Its top edge.
        y: f32,
        /// Its width.
        w: f32,
        /// Its height.
        h: f32,
        /// Its colour.
        colour: Rgb,
    },
    /// A filled rectangle with its corners rounded by `radius`.
    Round {
        /// Its left edge.
        x: f32,
        /// Its top edge.
        y: f32,
        /// Its width.
        w: f32,
        /// Its height.
        h: f32,
        /// How far the corners are rounded.
        radius: f32,
        /// Its colour.
        colour: Rgb,
    },
    /// A rectangle's outline, `width` thick inside it, corners rounded by `radius`.
    Frame {
        /// Its left edge.
        x: f32,
        /// Its top edge.
        y: f32,
        /// Its width.
        w: f32,
        /// Its height.
        h: f32,
        /// How far the corners are rounded.
        radius: f32,
        /// How thick the outline is, inside the rectangle.
        width: f32,
        /// Its colour.
        colour: Rgb,
    },
    /// One line of text, `size` units high, the top of its line box at `y`; `anchor` is where
    /// `x` falls along it: 0 its left end, 0.5 its middle, 1 its right end.
    Text {
        /// The text.
        text: String,
        /// How high it is, in screen units.
        size: f32,
        /// Whether it is in the bold face.
        bold: bool,
        /// Where `anchor` falls along the line.
        x: f32,
        /// The top of its line box.
        y: f32,
        /// Where `x` falls along the line: 0 its left end, 0.5 its middle, 1 its right end.
        anchor: f32,
        /// Its colour.
        colour: Rgb,
    },
    /// `rating` of 5 stars from `x`, each `ICON` square, 4 apart; the rest faint.
    Stars {
        /// The first star's left edge.
        x: f32,
        /// The stars' top edge.
        y: f32,
        /// How many of the 5 are lit.
        rating: u8,
    },
    /// The battery icon, `h` tall and 2.1 times as wide.
    Battery {
        /// Its left edge.
        x: f32,
        /// Its top edge.
        y: f32,
        /// Its height.
        h: f32,
        /// The charge, 0-100: red at 15 or under unless charging.
        percent: u8,
        /// Whether it is charging: a bolt over it.
        charging: bool,
    },
    /// The bar, the slot cut into its top edge.
    Bar {
        /// Its top edge.
        y: f32,
        /// The slot cut into it.
        slot: Slot,
    },
    /// A small picture on a row (a signal, a lock, a check), `h` high, its left edge at `x`:
    /// [`super::glyphs::render`] draws it.
    Glyph {
        /// Which.
        glyph: super::glyphs::Glyph,
        /// Its left edge.
        x: f32,
        /// Its top edge.
        y: f32,
        /// Its height.
        h: f32,
    },
    /// One of the app's own pictures (a save state's), fitted into the box: as large as it goes
    /// with its own proportions kept, centred, the box's background showing round it. A
    /// one-screen picture (GB, GBA) and a DS state's two screens stacked (256 x 384) both fit.
    /// `faded` dims it.
    Picture {
        /// Which of the app's pictures.
        index: usize,
        /// The box's left edge.
        x: f32,
        /// The box's top edge.
        y: f32,
        /// The box's width.
        w: f32,
        /// The box's height.
        h: f32,
        /// Whether it is dimmed.
        faded: bool,
    },
}

/// A row's centre on screen, by its place among those on screen.
pub fn row_y(place: usize) -> f32 {
    TOP + place as f32 * ROW_STEP
}

/// The row on screen under a touch, counting from the first on screen.
pub fn row_hit(y: f32) -> Option<usize> {
    (0..VISIBLE).find(|&r| (y - row_y(r)).abs() < ROW_STEP / 2.0)
}

/// The tab under a touch, of `n`.
pub fn tab_hit(x: f32, y: f32, n: usize) -> Option<usize> {
    let (tx, ty, w, h) = TABS;
    let inside = n > 0
        && (ty - 10.0..ty + h + 14.0).contains(&y)
        && (tx..tx + n as f32 * w).contains(&x)
        && x < W;
    inside.then(|| (((x - tx) / w) as usize).min(n - 1))
}

fn text(s: &str, (size, bold): (f32, bool), (x, y): (f32, f32), anchor: f32, colour: Rgb) -> Draw {
    Draw::Text {
        text: s.to_owned(),
        size,
        bold,
        x,
        y,
        anchor,
        colour,
    }
}

fn tabs(out: &mut Vec<Draw>, tabs: &[(String, bool)]) {
    let (x, y, w, h) = TABS;
    for (i, (label, on)) in tabs.iter().enumerate() {
        let at = x + i as f32 * w;
        out.push(Draw::Rect {
            x: at,
            y,
            w: w - 2.0,
            h,
            colour: if *on { colour::WHITE } else { colour::PANEL },
        });
        let ink = if *on { colour::TAB_INK } else { colour::DIM };
        out.push(text(
            label,
            (22.0, true),
            (at + (w - 2.0) / 2.0, y + 7.0),
            0.5,
            ink,
        ));
    }
}

/// The hint line and the bar with the battery and the clock.
fn footer(out: &mut Vec<Draw>, hint: &str, status: &Status) {
    out.push(text(
        hint,
        (18.0, false),
        (W / 2.0, HINT_Y),
        0.5,
        colour::FAINT,
    ));
    out.push(Draw::Bar {
        y: H - BAR_H,
        slot: status.slot,
    });
    if let Some((percent, charging)) = status.battery {
        let w = BATTERY * 2.1;
        out.push(Draw::Battery {
            x: 40.0,
            y: H - 59.0,
            h: BATTERY,
            percent,
            charging,
        });
        out.push(text(
            &format!("{percent}%"),
            (24.0, true),
            (40.0 + w + 10.0, H - 62.0),
            0.0,
            colour::DIM,
        ));
    }
    out.push(text(
        &status.clock,
        (24.0, true),
        (W - 40.0, H - 62.0),
        1.0,
        colour::DIM,
    ));
}

/// A page of rows, as DSlot's menu draws one. A row's note goes left of its value by an
/// estimate of the value's width; [`menu_measured`] places it exactly.
pub fn menu(m: &Menu, status: &Status) -> Vec<Draw> {
    menu_measured(m, status, |s, size, _| {
        s.chars().count() as f32 * size * 0.45
    })
}

/// [`menu`], with `width(text, size, bold)` saying how wide text is drawn, in screen units:
/// a row's note goes just left of its value.
pub fn menu_measured(
    m: &Menu,
    status: &Status,
    width: impl Fn(&str, f32, bool) -> f32,
) -> Vec<Draw> {
    let mut out = Vec::new();
    tabs(&mut out, &m.tabs);
    out.push(text(
        &m.title,
        (22.0, true),
        (LEFT, 72.0),
        0.0,
        colour::FAINT,
    ));
    let shown = m.rows.iter().enumerate().skip(m.scroll).take(VISIBLE);
    for (place, (i, row)) in shown.enumerate() {
        let y = row_y(place);
        if row.heading {
            out.push(text(
                &row.label,
                (20.0, true),
                (LEFT, y - 6.0),
                0.0,
                colour::FAINT,
            ));
            continue;
        }
        let lit = i == m.row;
        if lit {
            out.push(Draw::Rect {
                x: 120.0,
                y: y - ROW_STEP / 2.0 + 2.0,
                w: W - 240.0,
                h: ROW_STEP - 4.0,
                colour: colour::PANEL,
            });
        }
        let ink = if lit { colour::WHITE } else { colour::DIM };
        out.push(text(&row.label, (26.0, false), (LEFT, y - 16.0), 0.0, ink));
        let value = (26.0, true);
        // How wide the value is drawn, so that its note goes just left of it.
        let drawn = match &row.value {
            Value::None => 0.0,
            Value::Text(t) | Value::Inherited(t) => {
                let ink = match row.value {
                    Value::Inherited(_) => colour::DIM,
                    _ => colour::WHITE,
                };
                let t = if lit {
                    format!("\u{2039}   {t}   \u{203a}")
                } else {
                    t.clone()
                };
                out.push(text(&t, value, (RIGHT, y - 16.0), 1.0, ink));
                width(&t, 26.0, true)
            }
            Value::More(t) => {
                let t = format!("{t}   \u{203a}");
                out.push(text(&t, value, (RIGHT, y - 16.0), 1.0, colour::WHITE));
                width(&t, 26.0, true)
            }
            Value::Prompt { text: t, warning } => {
                let ink = if *warning { colour::RED } else { colour::WHITE };
                out.push(text(t, value, (RIGHT, y - 16.0), 1.0, ink));
                width(t, 26.0, true)
            }
            Value::Status { text: t, glyphs } => {
                // The pictures at the right edge, right to left, the words left of them.
                let (h, gap) = (GLYPH, 12.0);
                let mut x = RIGHT;
                for &g in glyphs.iter().rev() {
                    let w = g.width(h);
                    x -= w;
                    out.push(match g {
                        super::glyphs::Glyph::Battery(percent) => Draw::Battery {
                            x,
                            y: y - h * 0.42,
                            h: h * 0.84,
                            percent,
                            charging: false,
                        },
                        _ => Draw::Glyph {
                            glyph: g,
                            x,
                            y: y - h / 2.0,
                            h,
                        },
                    });
                    x -= gap;
                }
                if !t.is_empty() {
                    let ink = if lit { colour::WHITE } else { colour::DIM };
                    out.push(text(t, value, (x, y - 16.0), 1.0, ink));
                    x -= width(t, 26.0, true);
                }
                RIGHT - x
            }
            Value::Stars(rating) => {
                let w = 5.0 * (ICON + 4.0) - 4.0;
                out.push(Draw::Stars {
                    x: RIGHT - w,
                    y: y - ICON / 2.0,
                    rating: *rating,
                });
                w
            }
            Value::Slider(percent) => {
                out.push(text(
                    &format!("{percent}%"),
                    value,
                    (RIGHT, y - 16.0),
                    1.0,
                    colour::WHITE,
                ));
                let (bar, gap) = (180.0, 76.0);
                let x = RIGHT - gap - bar;
                let track = [
                    colour::PANEL[0] + 30,
                    colour::PANEL[1] + 30,
                    colour::PANEL[2] + 30,
                ];
                out.push(Draw::Rect {
                    x,
                    y: y - 3.0,
                    w: bar,
                    h: 6.0,
                    colour: track,
                });
                let filled = bar * (*percent as f32 / 100.0).clamp(0.0, 1.0);
                out.push(Draw::Rect {
                    x,
                    y: y - 3.0,
                    w: filled.max(1.0),
                    h: 6.0,
                    colour: colour::WHITE,
                });
                gap + bar
            }
        };
        if let Some(note) = &row.note {
            out.push(text(
                note,
                (18.0, false),
                (RIGHT - drawn - 22.0, y - 10.0),
                1.0,
                colour::FAINT,
            ));
        }
    }
    // More rows above or below: an ellipsis (the face has one; it has no arrows).
    if m.scroll > 0 {
        let y = row_y(0) - ROW_STEP / 2.0 - 30.0;
        out.push(text(
            "\u{2026}",
            (28.0, true),
            (RIGHT, y),
            1.0,
            colour::FAINT,
        ));
    }
    let on_screen = m.rows.len().saturating_sub(m.scroll).min(VISIBLE);
    if m.scroll + on_screen < m.rows.len() {
        let y = row_y(on_screen) - ROW_STEP / 2.0 - 4.0;
        out.push(text(
            "\u{2026}",
            (28.0, true),
            (RIGHT, y),
            1.0,
            colour::FAINT,
        ));
    }
    footer(&mut out, &m.hint, status);
    out
}

/// The cards' boxes: the highlighted one in the middle, its neighbours either side, smaller.
pub const CARD: (f32, f32) = (300.0, 310.0);
/// Its neighbours' size.
pub const SIDE_CARD: (f32, f32) = (250.0, 260.0);
/// How far apart the cards' centres are.
pub const CARD_STEP: f32 = 330.0;
/// The highlighted card's top.
pub const CARD_Y: f32 = 190.0;

/// The space between the resume point and the numbered slots, beyond the cards' usual step.
pub const RESUME_GAP: f32 = 48.0;

/// The box of card `i` on the shelf, if it is on screen: the highlighted one in the middle,
/// its neighbours either side, smaller; the resume point a little further from the slots.
pub fn card_box(s: &Shelf, i: usize) -> Option<(f32, f32, f32, f32)> {
    let offset = i as i32 - s.at as i32;
    if offset.abs() > 1 || i >= s.cards.len() {
        return None;
    }
    let resume = |k: usize| s.cards.get(k).is_some_and(|c| c.resume);
    let gap = match (resume(i), resume(s.at)) {
        (true, false) => -RESUME_GAP,
        (false, true) => RESUME_GAP,
        _ => 0.0,
    };
    let (w, h) = if offset == 0 { CARD } else { SIDE_CARD };
    let cx = W / 2.0 + offset as f32 * CARD_STEP + gap;
    let cy = CARD_Y + CARD.1 / 2.0;
    Some((cx - w / 2.0, cy - h / 2.0, w, h))
}

/// The card under a touch, by its place on the shelf.
pub fn card_hit(s: &Shelf, x: f32, y: f32) -> Option<usize> {
    (0..s.cards.len()).find(|&i| {
        card_box(s, i).is_some_and(|(bx, by, bw, bh)| {
            (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y)
        })
    })
}

/// A shelf of cards: the highlighted one big in the middle, its neighbours either side.
pub fn shelf(s: &Shelf, status: &Status) -> Vec<Draw> {
    let mut out = Vec::new();
    tabs(&mut out, &s.tabs);
    // Which slot of how many (the resume point is not one of them; its tag says what it is).
    let slots = s.cards.iter().filter(|c| !c.resume).count();
    let counter = match s.cards.get(s.at) {
        Some(card) if card.resume => String::new(),
        Some(_) => {
            let before = s.cards[..s.at].iter().filter(|c| c.resume).count();
            format!("{} / {slots}", s.at - before + 1)
        }
        None => String::new(),
    };
    if !counter.is_empty() {
        out.push(text(
            &counter,
            (22.0, false),
            (W / 2.0, 120.0),
            0.5,
            colour::DIM,
        ));
    }
    for (i, card) in s.cards.iter().enumerate() {
        let Some((x, y, w, h)) = card_box(s, i) else {
            continue;
        };
        let lit = i == s.at;
        let pad = if lit { 16.0 } else { 14.0 };
        let label_h = if lit { 54.0 } else { 46.0 };
        if card.resume {
            // The resume point: an outline round a dark card, not a filled one.
            out.push(Draw::Round {
                x,
                y,
                w,
                h,
                radius: 14.0,
                colour: [24, 27, 33],
            });
            out.push(Draw::Frame {
                x,
                y,
                w,
                h,
                radius: 14.0,
                width: 2.0,
                colour: if lit { colour::DIM } else { colour::FAINT },
            });
            out.push(text(
                "RESUME",
                (18.0, true),
                (x + w / 2.0, y - 30.0),
                0.5,
                if lit { colour::DIM } else { colour::FAINT },
            ));
        } else {
            out.push(Draw::Round {
                x,
                y,
                w,
                h,
                radius: 14.0,
                colour: if lit { [59, 63, 71] } else { [43, 47, 54] },
            });
        }
        let (px, py, pw, ph) = (x + pad, y + pad, w - 2.0 * pad, h - pad - label_h);
        match card.picture {
            Some(index) => out.push(Draw::Picture {
                index,
                x: px,
                y: py,
                w: pw,
                h: ph,
                faded: !lit,
            }),
            None => {
                out.push(Draw::Rect {
                    x: px,
                    y: py,
                    w: pw,
                    h: ph,
                    colour: [21, 24, 30],
                });
                out.push(text(
                    "Empty",
                    (26.0, false),
                    (px + pw / 2.0, py + ph / 2.0 - 16.0),
                    0.5,
                    colour::FAINT,
                ));
            }
        }
        let ink = if lit { colour::WHITE } else { colour::DIM };
        let size = if lit { (28.0, true) } else { (24.0, false) };
        out.push(text(
            &card.label,
            size,
            (x + w / 2.0, y + h - label_h + 8.0),
            0.5,
            ink,
        ));
        if lit {
            out.push(Draw::Frame {
                x: x - 2.0,
                y: y - 2.0,
                w: w + 4.0,
                h: h + 4.0,
                radius: 16.0,
                width: 4.0,
                colour: colour::GOLD,
            });
        }
    }
    footer(&mut out, &s.hint, status);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Card, Row};

    fn texts(d: &[Draw]) -> Vec<(String, f32, f32, Rgb)> {
        d.iter()
            .filter_map(|d| match d {
                Draw::Text {
                    text, x, y, colour, ..
                } => Some((text.clone(), *x, *y, *colour)),
                _ => None,
            })
            .collect()
    }

    fn page(rows: Vec<Row>, row: usize) -> Menu {
        Menu {
            tabs: vec![("Play".into(), true), ("Game".into(), false)],
            title: "TETRIS".into(),
            rows,
            row,
            scroll: 0,
            hint: "A  choose".into(),
        }
    }

    /// The numbers are DSlot's: the first row at 156, 46 apart, labels at 150, values ending
    /// at 874, the highlight band 784 wide from 120.
    #[test]
    fn rows_sit_where_dslot_puts_them() {
        let m = page(
            vec![
                Row::new("Resume", Value::None),
                Row::new("Timing", Value::Inherited("Use default (Smooth)".into())),
                Row::new("Fast-forward", Value::Text("6x".into())),
            ],
            2,
        );
        let d = menu(&m, &Status::default());
        let t = texts(&d);
        assert!(t.contains(&("Resume".into(), 150.0, 140.0, colour::DIM)));
        assert!(t.contains(&("Use default (Smooth)".into(), RIGHT, 186.0, colour::DIM)));
        assert!(t.contains(&(
            "\u{2039}   6x   \u{203a}".into(),
            874.0,
            232.0,
            colour::WHITE
        )));
        assert!(t.contains(&("Fast-forward".into(), 150.0, 232.0, colour::WHITE)));
        assert!(d.contains(&Draw::Rect {
            x: 120.0,
            y: 227.0,
            w: 784.0,
            h: 42.0,
            colour: colour::PANEL
        }));
        // The selected tab is white with dark ink; the hint and the clock bar at the foot.
        assert!(d.contains(&Draw::Rect {
            x: 28.0,
            y: 10.0,
            w: 128.0,
            h: 40.0,
            colour: colour::WHITE
        }));
        assert!(t
            .iter()
            .any(|(s, _, y, _)| s == "A  choose" && *y == HINT_Y));
        assert!(d
            .iter()
            .any(|d| matches!(d, Draw::Bar { y, .. } if *y == 672.0)));
    }

    #[test]
    fn a_long_page_scrolls_with_ellipses() {
        let rows = (0..14)
            .map(|i| Row::new(format!("Row {i}"), Value::None))
            .collect();
        let mut m = page(rows, 0);
        let marks = |m: &Menu| {
            texts(&menu(m, &Status::default()))
                .iter()
                .filter(|t| t.0 == "\u{2026}")
                .count()
        };
        assert_eq!(marks(&m), 1, "more below");
        m.scroll = 4;
        assert_eq!(marks(&m), 1, "more above, none below");
        m.scroll = 2;
        assert_eq!(marks(&m), 2);
    }

    #[test]
    fn touches_find_rows_tabs_and_cards() {
        assert_eq!(row_hit(156.0), Some(0));
        assert_eq!(row_hit(156.0 + 46.0 * 3.0 + 10.0), Some(3));
        assert_eq!(row_hit(100.0), None);
        assert_eq!(tab_hit(30.0, 20.0, 5), Some(0));
        assert_eq!(tab_hit(300.0, 20.0, 5), Some(2));
        assert_eq!(tab_hit(300.0, 20.0, 2), None);
        let s = Shelf {
            cards: vec![
                Card::resume(None, "r"),
                Card::slot(None, "1"),
                Card::slot(None, "2"),
            ],
            at: 1,
            ..Shelf::default()
        };
        assert_eq!(card_hit(&s, 512.0, 300.0), Some(1));
        assert_eq!(card_hit(&s, 512.0 + CARD_STEP, 300.0), Some(2));
        assert_eq!(card_hit(&s, 512.0 - CARD_STEP - RESUME_GAP, 300.0), Some(0));
        assert_eq!(card_hit(&s, 512.0, 600.0), None);
    }

    #[test]
    fn the_resume_point_stands_apart() {
        let mut s = Shelf {
            cards: vec![
                Card::resume(Some(0), "Today 4:41 PM"),
                Card::slot(None, "1"),
            ],
            ..Shelf::default()
        };
        s.at = s.first_slot();
        assert_eq!(s.at, 1);
        let (rx, ..) = card_box(&s, 0).unwrap();
        let (sx, ..) = card_box(&s, 1).unwrap();
        assert_eq!(
            sx - rx,
            CARD_STEP + RESUME_GAP - (CARD.0 - SIDE_CARD.0) / 2.0
        );
        let labels = |s: &Shelf| -> Vec<String> {
            texts(&shelf(s, &Status::default()))
                .into_iter()
                .map(|t| t.0)
                .collect()
        };
        assert!(labels(&s).contains(&"1 / 1".to_owned()));
        assert!(labels(&s).contains(&"RESUME".to_owned()));
        s.at = 0;
        assert!(!labels(&s).iter().any(|l| l.contains(" / ")));
    }

    #[test]
    fn a_shelf_shows_the_highlighted_card_and_its_neighbours() {
        let s = Shelf {
            tabs: vec![],
            cards: (0..7)
                .map(|i| Card::slot((i < 3).then_some(i), format!("{}", i + 1)))
                .collect(),
            at: 3,
            hint: String::new(),
        };
        let d = shelf(&s, &Status::default());
        let labels: Vec<_> = texts(&d).into_iter().map(|t| t.0).collect();
        assert!(labels.contains(&"4 / 7".to_owned()));
        assert!(labels.contains(&"3".to_owned()) && labels.contains(&"5".to_owned()));
        assert!(!labels.contains(&"2".to_owned()));
        assert!(d.iter().any(|d| matches!(
            d,
            Draw::Picture {
                index: 2,
                faded: true,
                ..
            }
        )));
        assert!(labels.contains(&"Empty".to_owned()));
    }
}
