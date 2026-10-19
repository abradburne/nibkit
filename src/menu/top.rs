//! The top screen while an app's menu is open, as inkdot draws it: the game paused (dimmed, with
//! its name under "Paused"), or, on a shelf of save states, the highlighted one's picture large
//! with its name and details beside it. Laid out on the same 1024 × 768 screen as the menu.

use super::colour;
use super::layout::Draw;
use super::W;

/// Where a save state's picture goes on the top screen (a Game Boy's 160 × 144 at 3.5×).
pub const PICTURE: (f32, f32, f32, f32) = (62.0, 104.0, 560.0, 504.0);
/// How much of the game shows through while paused: the rest is black.
pub const DIM: f32 = 0.45;

fn text(
    s: &str,
    (size, bold): (f32, bool),
    (x, y): (f32, f32),
    anchor: f32,
    colour: colour::Rgb,
) -> Draw {
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

/// The words over the paused game: `title` large ("Paused"), `subtitle` under it (the game's
/// name and today's play time). The app dims the game under them (to [`DIM`]).
pub fn paused(title: &str, subtitle: &str) -> Vec<Draw> {
    vec![
        text(title, (64.0, true), (W / 2.0, 300.0), 0.5, colour::WHITE),
        text(subtitle, (30.0, false), (W / 2.0, 384.0), 0.5, colour::DIM),
    ]
}

/// A save state: its picture (the app's, by index; `None` for an empty slot) in [`PICTURE`], its
/// name large beside it, a line under the name ("Quick slot", or empty), then label / value
/// lines ("Saved", "Today 4:41 PM").
pub fn state(name: &str, under: &str, lines: &[(&str, &str)], picture: Option<usize>) -> Vec<Draw> {
    let (x, y, w, h) = PICTURE;
    let mut out = vec![match picture {
        Some(index) => Draw::Picture {
            index,
            x,
            y,
            w,
            h,
            faded: false,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [21, 24, 30],
        },
    }];
    out.push(text(name, (60.0, true), (660.0, 170.0), 0.0, colour::WHITE));
    if !under.is_empty() {
        out.push(text(under, (30.0, false), (660.0, 246.0), 0.0, colour::DIM));
    }
    for (k, (label, value)) in lines.iter().enumerate() {
        let y = 340.0 + k as f32 * 40.0;
        out.push(text(label, (26.0, false), (660.0, y), 0.0, colour::DIM));
        out.push(text(value, (26.0, true), (780.0, y), 0.0, colour::WHITE));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_and_state_screens() {
        let p = paused("Paused", "Tetris \u{b7} 12 min today");
        assert_eq!(p.len(), 2);
        let s = state(
            "Slot 2",
            "Quick slot",
            &[("Saved", "Today"), ("Played", "1 h")],
            Some(0),
        );
        assert!(matches!(s[0], Draw::Picture { index: 0, .. }));
        assert_eq!(s.len(), 1 + 2 + 4);
        let empty = state("Slot 4", "", &[("", "Empty")], None);
        assert!(matches!(empty[0], Draw::Rect { .. }));
    }
}
