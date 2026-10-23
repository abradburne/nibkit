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

/// How wide text is drawn without the font to hand: an estimate, as [`super::layout::menu`]
/// makes. Apps that draw with nibkit's fonts pass their real width to the `_measured` calls.
fn estimate(s: &str, size: f32, _bold: bool) -> f32 {
    s.chars().count() as f32 * size * 0.45
}

/// The distance from one line's top to the next's, for text of `size`.
fn advance(size: f32) -> f32 {
    size * 1.1
}

/// `text` on lines no wider than `max`, broken between words (a word too wide for a line alone
/// is broken between characters).
fn wrap(
    text: &str,
    size: f32,
    bold: bool,
    max: f32,
    width: &dyn Fn(&str, f32, bool) -> f32,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let joined = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if width(&joined, size, bold) <= max {
            line = joined;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        // The word starts a line; one too wide for any line is cut between characters.
        for c in word.chars() {
            let longer = format!("{line}{c}");
            if !line.is_empty() && width(&longer, size, bold) > max {
                lines.push(std::mem::replace(&mut line, c.to_string()));
            } else {
                line = longer;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `text` fitted into `lines` lines no wider than `max`: wrapped at the first of `sizes`
/// (largest first) where it fits; at the last, what doesn't fit is cut between characters and
/// ends in an ellipsis. Gives the lines and the size they are drawn at.
pub fn fit(
    text: &str,
    sizes: &[f32],
    bold: bool,
    max: f32,
    lines: usize,
    width: &dyn Fn(&str, f32, bool) -> f32,
) -> (Vec<String>, f32) {
    let lines = lines.max(1);
    let mut size = 0.0;
    let mut wrapped = Vec::new();
    for &s in sizes {
        size = s;
        wrapped = wrap(text, s, bold, max, width);
        if wrapped.len() <= lines {
            return (wrapped, s);
        }
    }
    // Still too long at the smallest size: the last line takes the rest, cut to fit "…".
    let rest = wrapped.split_off(lines - 1).join(" ");
    let mut chars: Vec<char> = rest.chars().collect();
    let cut = |chars: &[char]| {
        let kept: String = chars.iter().collect();
        format!(
            "{}\u{2026}",
            kept.trim_end_matches(|c: char| c.is_whitespace() || ",.;:-".contains(c))
        )
    };
    while !chars.is_empty() && width(&cut(&chars), size, bold) > max {
        chars.pop();
    }
    wrapped.push(cut(&chars));
    (wrapped, size)
}

/// Fitted text's draws, centred on `x` (`anchor` as in [`Draw::Text`]) from `top` down; gives
/// them and where the last line ends.
fn lines(
    fitted: &(Vec<String>, f32),
    bold: bool,
    (x, top): (f32, f32),
    anchor: f32,
    colour: colour::Rgb,
) -> (Vec<Draw>, f32) {
    let (lines, size) = fitted;
    let draws = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            text(
                l,
                (*size, bold),
                (x, top + i as f32 * advance(*size)),
                anchor,
                colour,
            )
        })
        .collect();
    (
        draws,
        top + *size + (lines.len().max(1) - 1) as f32 * advance(*size),
    )
}

/// How high fitted text stands.
fn height((lines, size): &(Vec<String>, f32)) -> f32 {
    size + (lines.len().max(1) - 1) as f32 * advance(*size)
}

/// The paused screen's widest line.
const PAUSED_WIDTH: f32 = W - 96.0;

/// The words over the paused game: `title` large ("Paused"), `subtitle` under it (the game's
/// name and today's play time). The app dims the game under them (to [`DIM`]). Widths are
/// estimated: an app drawing with nibkit's fonts calls [`paused_measured`].
pub fn paused(title: &str, subtitle: &str) -> Vec<Draw> {
    paused_measured(title, subtitle, estimate)
}

/// [`paused`], with `width(text, size, bold)` saying how wide text is drawn ([`Fonts::width`]
/// with nibkit's raster). A long title (a disc game's full name) goes onto two lines, then a
/// size smaller, then ends in an ellipsis; the subtitle likewise, smaller. The two stay centred
/// where one line of each would be.
///
/// [`Fonts::width`]: super::raster::Fonts::width
pub fn paused_measured(
    title: &str,
    subtitle: &str,
    width: impl Fn(&str, f32, bool) -> f32,
) -> Vec<Draw> {
    let title = fit(title, &[64.0, 52.0], true, PAUSED_WIDTH, 2, &width);
    let subtitle = fit(subtitle, &[30.0, 26.0], false, PAUSED_WIDTH, 2, &width);
    // One line of each spans 300 to 414: keep the middle there.
    let gap = 20.0;
    let top = 357.0 - (height(&title) + gap + height(&subtitle)) / 2.0;
    let (mut out, bottom) = lines(&title, true, (W / 2.0, top), 0.5, colour::WHITE);
    out.extend(lines(&subtitle, false, (W / 2.0, bottom + gap), 0.5, colour::DIM).0);
    out
}

/// The text column beside a save state's picture: from here to the screen's edge, less a margin.
const STATE_X: f32 = 660.0;
const STATE_WIDTH: f32 = W - STATE_X - 40.0;

/// A save state: its picture (the app's, by index; `None` for an empty slot) in [`PICTURE`], its
/// name large beside it, a line under the name ("Quick slot", or empty), then label / value
/// lines ("Saved", "Today 4:41 PM"). Widths are estimated: an app drawing with nibkit's fonts
/// calls [`state_measured`].
pub fn state(name: &str, under: &str, lines: &[(&str, &str)], picture: Option<usize>) -> Vec<Draw> {
    state_measured(name, under, lines, picture, estimate)
}

/// [`state`], with `width(text, size, bold)` saying how wide text is drawn. A long name goes onto
/// two lines, then a size smaller, then ends in an ellipsis; the lines under it move down to
/// make room, and each keeps to the column.
pub fn state_measured(
    name: &str,
    under: &str,
    rows: &[(&str, &str)],
    picture: Option<usize>,
    width: impl Fn(&str, f32, bool) -> f32,
) -> Vec<Draw> {
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
    let name = fit(name, &[60.0, 48.0], true, STATE_WIDTH, 2, &width);
    let (draws, mut bottom) = lines(&name, true, (STATE_X, 170.0), 0.0, colour::WHITE);
    out.extend(draws);
    if !under.is_empty() {
        let under = fit(under, &[30.0, 26.0], false, STATE_WIDTH, 1, &width);
        let (draws, end) = lines(&under, false, (STATE_X, bottom + 16.0), 0.0, colour::DIM);
        out.extend(draws);
        bottom = end;
    }
    // One line of name and of the line under it end at 276; the rows start at 340 below that.
    let first = 340.0f32.max(bottom + 64.0);
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = first + k as f32 * 40.0;
        let value = fit(value, &[26.0], true, W - 780.0 - 40.0, 1, &width)
            .0
            .join("");
        out.push(text(label, (26.0, false), (STATE_X, y), 0.0, colour::DIM));
        out.push(text(&value, (26.0, true), (780.0, y), 0.0, colour::WHITE));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roboto Condensed's width, near enough: half the size a character.
    fn half(s: &str, size: f32, _bold: bool) -> f32 {
        s.chars().count() as f32 * size * 0.5
    }

    fn texts(draws: &[Draw]) -> Vec<(String, f32, f32)> {
        draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text { text, size, y, .. } => Some((text.clone(), *size, *y)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_short_title_is_one_line_where_it_always_was() {
        let t = texts(&paused_measured("Tetris", "Paused", half));
        assert_eq!(
            t,
            [
                ("Tetris".into(), 64.0, 300.0),
                ("Paused".into(), 30.0, 384.0)
            ]
        );
    }

    #[test]
    fn a_long_title_wraps_then_shrinks_then_ends_in_an_ellipsis() {
        let max = PAUSED_WIDTH;
        // 29 characters a line at 64: two lines, full size.
        let two = "Gran Turismo 2 Arcade Mode USA Rev 1 and a little more";
        let (l, size) = fit(two, &[64.0, 52.0], true, max, 2, &half);
        assert_eq!((l.len(), size), (2, 64.0));
        assert!(l.iter().all(|l| half(l, 64.0, true) <= max));
        assert_eq!(l.join(" "), two);
        // Longer: a size smaller, still whole.
        let three = format!("{two} more words");
        let (l, size) = fit(&three, &[64.0, 52.0], true, max, 2, &half);
        assert_eq!((l.len(), size), (2, 52.0));
        assert_eq!(l.join(" "), three);
        // Longer still: an ellipsis, on whole characters, within the width.
        let long = "word ".repeat(60);
        let (l, size) = fit(&long, &[64.0, 52.0], true, max, 2, &half);
        assert_eq!((l.len(), size), (2, 52.0));
        assert!(l[1].ends_with('\u{2026}') && !l[1].ends_with(" \u{2026}"));
        assert!(l.iter().all(|l| half(l, 52.0, true) <= max));
        // One word longer than a line is cut between characters.
        let (l, _) = fit(&"x".repeat(200), &[64.0], true, max, 2, &half);
        assert!(l.iter().all(|l| half(l, 64.0, true) <= max));
        assert!(l[1].ends_with('\u{2026}'));
        // Non-ASCII is cut between characters, never inside one.
        let (l, _) = fit(&"ドラゴンクエスト".repeat(20), &[64.0], true, max, 1, &half);
        assert!(l[0].ends_with('\u{2026}'));
    }

    #[test]
    fn two_line_titles_stay_centred_and_the_subtitle_moves_down() {
        let t = texts(&paused_measured(&"Long name ".repeat(6), "Paused", half));
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].2 + advance(64.0), t[1].2);
        assert!(t[0].2 < 300.0 && t[2].2 > 384.0);
        let middle = (t[0].2 + t[2].2 + 30.0) / 2.0;
        assert!((middle - 357.0).abs() < 0.5);
    }

    #[test]
    fn a_long_state_name_pushes_its_lines_down() {
        let s = state_measured(
            "Gran Turismo 2 (Simulation Mode)",
            "Quick slot",
            &[("Saved", "Today")],
            Some(0),
            half,
        );
        let t = texts(&s);
        // Too long for two lines at 60 in the narrow column: two at 48, the second cut short.
        assert_eq!(t[0].1, 48.0);
        assert!(t[1].0.ends_with('\u{2026}'));
        assert!(t.iter().all(|(l, size, _)| l == "Saved"
            || l == "Today"
            || half(l, *size, true) <= STATE_WIDTH));
        let saved = t.iter().find(|x| x.0 == "Saved").unwrap();
        assert!(saved.2 >= 340.0);
    }

    /// With the real font: a very long disc name, kept inside the screen.
    #[cfg(feature = "menu-raster")]
    #[test]
    fn real_font_long_names_stay_on_screen() {
        let fonts = super::super::raster::Fonts::new();
        let width = |s: &str, size: f32, bold: bool| fonts.width(s, size, bold);
        for name in [
            "Gran Turismo 2 (USA) (Simulation Mode) (Rev 2)",
            "Final Fantasy VII - International Edition (Japan) (Disc 3) (Rev 1) [Hack by Someone]",
            &"Wide ".repeat(80),
        ] {
            let draws = paused_measured(name, "Paused", width);
            for (line, size, _) in texts(&draws) {
                assert!(
                    fonts.width(&line, size, true) <= PAUSED_WIDTH,
                    "{line} at {size}"
                );
            }
            let draws = state_measured(name, "Quick slot", &[], Some(0), width);
            for (line, size, _) in texts(&draws) {
                assert!(
                    fonts.width(&line, size, true) <= STATE_WIDTH,
                    "{line} at {size}"
                );
            }
        }
    }

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
