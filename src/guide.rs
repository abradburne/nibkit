//! The words the system keeps for its menus to show: **What's new** in the release that is
//! running, and short **help** topics. They are plain text files, so that every app's menu shows
//! the same words and a release changes them without changing any app:
//!
//! - `/usr/share/nibos/whats-new.txt`: the running release's notes, which nibOS's image build
//!   writes from its changelog's section for the version.
//! - `/usr/share/nibos/help/*.txt`: one topic each, shown in the order of their file names
//!   (`10-controls.txt`, `20-menu.txt`...).
//!
//! `NIBOS_GUIDE` names another folder laid out the same way (to try topics out, or on a
//! computer). On the stock OS and a computer without it there are none: [`whats_new`] is
//! `None` and [`help`] empty.
//!
//! A file's first line is its title; the rest is its text, written to be read on a small
//! screen and broken into lines there by [`lines`]:
//!
//! - lines next to each other are one paragraph, joined (so a file can be broken anywhere);
//! - an empty line ends a paragraph;
//! - a line starting `# ` is a heading;
//! - a line starting `- ` is an item of a list, continued by the indented lines after it.
//!
//! ```
//! use nibkit::guide::{lines, Line};
//! let text = "# Sleep\nClose the lid,\nor press power.\n\n- Hold power for the power menu.";
//! assert_eq!(
//!     lines(text, 40),
//!     vec![
//!         Line::Heading("Sleep".into()),
//!         Line::Text("Close the lid, or press power.".into()),
//!         Line::Blank,
//!         Line::Text("\u{2022} Hold power for the power menu.".into()),
//!     ]
//! );
//! ```

use std::path::{Path, PathBuf};

/// Where the system keeps the files, on nibOS.
pub const DIR: &str = "/usr/share/nibos";
/// What's new's file, in [`dir`].
pub const WHATS_NEW: &str = "whats-new.txt";
/// The folder of help topics, in [`dir`].
pub const HELP: &str = "help";
/// The characters to a line in DSlot's menu, at its text's size, for [`lines`].
pub const WIDTH: usize = 64;

/// A file: its title, and its text as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topic {
    /// The file's name without `.txt` ("10-controls"), to tell topics apart.
    pub name: String,
    /// The first line.
    pub title: String,
    /// The rest, for [`lines`].
    pub text: String,
}

/// One line on the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    /// A line of a paragraph or a list.
    Text(String),
    /// A heading over what follows it.
    Heading(String),
    /// The space between two paragraphs.
    Blank,
}

/// The folder the files are in: `NIBOS_GUIDE` where it is set, else [`DIR`] on nibOS; `None` on
/// the stock OS or a computer without it.
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("NIBOS_GUIDE").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    cfg!(nibkit_nibos).then(|| PathBuf::from(DIR))
}

/// What's new in the running release, if the system says.
pub fn whats_new() -> Option<Topic> {
    read(&dir()?.join(WHATS_NEW))
}

/// The help topics, in the order of their file names.
pub fn help() -> Vec<Topic> {
    dir().map(|d| help_in(&d.join(HELP))).unwrap_or_default()
}

/// The topics in the folder `dir`: its `.txt` files, by name.
pub fn help_in(dir: &Path) -> Vec<Topic> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    paths.sort();
    paths.iter().filter_map(|p| read(p)).collect()
}

/// A file as a topic; `None` if it can't be read or has no title.
pub fn read(path: &Path) -> Option<Topic> {
    let text = std::fs::read_to_string(path).ok()?;
    let name = path.file_stem()?.to_string_lossy().into_owned();
    parse(&name, &text)
}

/// A topic from its text: the first line that isn't empty is the title.
pub fn parse(name: &str, text: &str) -> Option<Topic> {
    let text = text.trim_start_matches('\u{feff}');
    let mut it = text.lines();
    let title = it.by_ref().map(str::trim).find(|l| !l.is_empty())?;
    let rest: Vec<&str> = it.collect();
    Some(Topic {
        name: name.to_owned(),
        title: title.to_owned(),
        text: rest.join("\n").trim_matches('\n').to_owned(),
    })
}

/// The text as lines of at most `width` characters (a word longer than that is cut), the way
/// the module's notes say: paragraphs joined and broken between words, headings, the items of
/// a list with a bullet and their following lines indented under it, one blank line between
/// paragraphs (never at the start or end, never two).
pub fn lines(text: &str, width: usize) -> Vec<Line> {
    let width = width.max(8);
    let mut out = Vec::new();
    // The paragraph or item being gathered: its words, and whether it is an item.
    let mut words: Vec<&str> = Vec::new();
    let mut item = false;
    let blank = |out: &mut Vec<Line>| {
        if !matches!(out.last(), None | Some(Line::Blank)) {
            out.push(Line::Blank);
        }
    };
    let flush = |out: &mut Vec<Line>, words: &mut Vec<&str>, item: bool| {
        if words.is_empty() {
            return;
        }
        let (first, then) = if item { ("\u{2022} ", "  ") } else { ("", "") };
        let mut line = String::from(first);
        let mut fresh = true;
        for w in words.drain(..) {
            let mut w = w;
            loop {
                let room = width.saturating_sub(line.chars().count() + usize::from(!fresh));
                let n = w.chars().count();
                if n <= room {
                    if !fresh {
                        line.push(' ');
                    }
                    line.push_str(w);
                    fresh = false;
                    break;
                }
                if !fresh {
                    // The word on the next line.
                    out.push(Line::Text(std::mem::replace(&mut line, then.to_owned())));
                    fresh = true;
                    continue;
                }
                // Longer than a whole line: as much as fits, and the rest on the next.
                let room = width.saturating_sub(line.chars().count()).max(1);
                let cut = w.char_indices().nth(room).map_or(w.len(), |(i, _)| i);
                line.push_str(&w[..cut]);
                out.push(Line::Text(std::mem::replace(&mut line, then.to_owned())));
                w = &w[cut..];
                if w.is_empty() {
                    break;
                }
            }
        }
        if !fresh {
            out.push(Line::Text(line));
        }
    };
    for raw in text.lines() {
        let l = raw.trim_end();
        let t = l.trim_start();
        if t.is_empty() {
            flush(&mut out, &mut words, item);
            item = false;
            blank(&mut out);
        } else if let Some(h) = t.strip_prefix("# ") {
            flush(&mut out, &mut words, item);
            item = false;
            blank(&mut out);
            out.push(Line::Heading(h.trim().to_owned()));
        } else if let Some(rest) = t.strip_prefix("- ") {
            flush(&mut out, &mut words, item);
            item = true;
            words.extend(rest.split_whitespace());
        } else if item && l.len() > t.len() {
            // Indented under an item: more of it.
            words.extend(t.split_whitespace());
        } else {
            if item {
                flush(&mut out, &mut words, item);
                item = false;
            }
            words.extend(t.split_whitespace());
        }
    }
    flush(&mut out, &mut words, item);
    while out.last() == Some(&Line::Blank) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &[Line]) -> Vec<String> {
        l.iter()
            .map(|l| match l {
                Line::Text(t) => t.clone(),
                Line::Heading(h) => format!("# {h}"),
                Line::Blank => String::new(),
            })
            .collect()
    }

    #[test]
    fn paragraphs_are_joined_and_broken_between_words() {
        let got = lines("one two three\nfour five six seven\n\n\n\nnext", 12);
        assert_eq!(
            text(&got),
            ["one two", "three four", "five six", "seven", "", "next"]
        );
        assert!(text(&got).iter().all(|l| l.chars().count() <= 12));
    }

    #[test]
    fn items_hang_under_their_bullet() {
        let got = lines("Before:\n- first item here\n  goes on\n- second\nafter", 14);
        assert_eq!(
            text(&got),
            [
                "Before:",
                "\u{2022} first item",
                "  here goes on",
                "\u{2022} second",
                "after"
            ]
        );
    }

    #[test]
    fn headings_stand_apart_and_no_blank_ends() {
        let got = lines("\n\n# Top\nsome words\n# Next\n\n", 40);
        assert_eq!(text(&got), ["# Top", "some words", "", "# Next"]);
    }

    #[test]
    fn a_long_word_is_cut() {
        let got = lines("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(text(&got), ["abcdefghij", "klmnopqrst", "uvwxyz"]);
    }

    #[test]
    fn a_topic_is_its_title_and_the_rest() {
        let t = parse("10-controls", "\u{feff}\nControls\n\nThe D-pad moves.\n").unwrap();
        assert_eq!(t.title, "Controls");
        assert_eq!(t.text, "The D-pad moves.");
        assert_eq!(parse("x", "\n  \n"), None);
    }

    #[test]
    fn topics_come_in_file_name_order() {
        let d = std::env::temp_dir().join(format!("nibkit-guide-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("20-menu.txt"), "The menu\nBack opens it.").unwrap();
        std::fs::write(d.join("10-controls.txt"), "Controls\nThe D-pad moves.").unwrap();
        std::fs::write(d.join("notes.md"), "Not a topic").unwrap();
        std::fs::write(d.join("30-empty.txt"), "").unwrap();
        let names: Vec<String> = help_in(&d).into_iter().map(|t| t.title).collect();
        assert_eq!(names, ["Controls", "The menu"]);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
