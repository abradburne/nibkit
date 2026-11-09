//! The Help page, ready-made for any app's menu: **What's new** in the running release and the
//! system's help topics ([`crate::guide`]), listed, and each read as a page of text that scrolls
//! with the D-pad or a finger ([`Menu::reading`]). It looks and answers as DSlot's Help section
//! does, so a game's pause menu can offer the same words.
//!
//! An app keeps a [`Help`] while its menu is open and, each time it fills the page, has
//! [`Help::fill`] put the title, rows, hint and reading mode on its [`Menu`] (after the tabs).
//! A's [`Effect::Choose`] goes to [`Help::choose`], and B's [`Effect::Back`] to [`Help::back`],
//! which says whether it went back to the list (else the app goes back as it would):
//!
//! ```
//! use nibkit::menu::{help::Help, Effect, Key, Menu};
//! let mut help = Help::from_topics(None, vec![nibkit::guide::parse("controls", "Controls\nA plays.").unwrap()]);
//! let mut menu = Menu::default();
//! help.fill(&mut menu);
//! assert_eq!(menu.rows[0].label, "Controls");
//! if let Some(Effect::Choose(row)) = menu.key(Key::A) {
//!     help.choose(row);
//! }
//! help.fill(&mut menu);
//! assert!(menu.reading);
//! assert_eq!(menu.rows[0].label, "A plays.");
//! assert_eq!(menu.key(Key::B), Some(Effect::Back));
//! assert!(help.back(), "back to the list");
//! ```

use super::model::{Menu, Row, Value};
use crate::guide::{self, Line, Topic};

/// The list's hint line.
pub const HINT: &str = "A  read   \u{b7}   L / R  page   \u{b7}   B  back";
/// The hint line while a topic is read.
pub const READING_HINT: &str =
    "Up / down  scroll   \u{b7}   left / right  a screen   \u{b7}   B  back";
/// The list's title.
pub const TITLE: &str = "Help";
/// What's new's row in the list.
pub const WHATS_NEW: &str = "What's new";

/// The page while the menu is open: the topics, and the one being read.
#[derive(Clone, Debug, Default)]
pub struct Help {
    whats_new: Option<Topic>,
    topics: Vec<Topic>,
    /// The topic being read, by its row in the list.
    open: Option<usize>,
    /// The list's highlighted row, kept while a topic is read.
    list_row: usize,
    /// The page changed (a topic opened or closed): put it at its top when next filled.
    fresh: bool,
}

impl Help {
    /// The page, with the system's files as they are now ([`guide::whats_new`], [`guide::help`]).
    pub fn load() -> Help {
        Help::from_topics(guide::whats_new(), guide::help())
    }

    /// The page with these topics: What's new first, where there is one.
    pub fn from_topics(whats_new: Option<Topic>, topics: Vec<Topic>) -> Help {
        Help {
            whats_new,
            topics,
            fresh: true,
            ..Help::default()
        }
    }

    /// Whether there is anything to show (no files: no page).
    pub fn is_empty(&self) -> bool {
        self.whats_new.is_none() && self.topics.is_empty()
    }

    /// The list's topics, in its order: What's new's (named [`WHATS_NEW`]) first.
    fn listed(&self) -> impl Iterator<Item = (&str, &Topic)> {
        let first = self.whats_new.iter().map(|t| (WHATS_NEW, t));
        first.chain(self.topics.iter().map(|t| (t.title.as_str(), t)))
    }

    /// The topic being read, if one is.
    pub fn reading(&self) -> Option<&Topic> {
        self.listed().nth(self.open?).map(|(_, t)| t)
    }

    /// The page's title: the topic's while one is read ("What's new in nibOS 0.1.0-beta3",
    /// "Controls"), else [`TITLE`].
    pub fn title(&self) -> String {
        match self.reading() {
            Some(t) => t.title.clone(),
            None => TITLE.into(),
        }
    }

    /// The hint line: [`READING_HINT`] while a topic is read, else [`HINT`].
    pub fn hint(&self) -> &'static str {
        match self.open {
            Some(_) => READING_HINT,
            None => HINT,
        }
    }

    /// The rows: the list (a row for each topic, A to read it), or the topic's text as lines of
    /// [`guide::WIDTH`] characters (its headings as headings).
    pub fn rows(&self) -> Vec<Row> {
        match self.reading() {
            Some(t) => text_rows(&t.text, guide::WIDTH),
            None => self
                .listed()
                .map(|(label, _)| Row::new(label, Value::More(String::new())))
                .collect(),
        }
    }

    /// Puts the page on `menu` after its tabs: title, rows, hint and reading mode, at the top of
    /// a topic just opened (or on the list's row it was opened from, back on the list).
    pub fn fill(&mut self, menu: &mut Menu) {
        menu.title = self.title();
        menu.hint = self.hint().into();
        menu.rows = self.rows();
        menu.reading = self.open.is_some();
        if std::mem::take(&mut self.fresh) {
            (menu.row, menu.scroll) = match self.open {
                Some(_) => (0, 0),
                None => (self.list_row, 0),
            };
        }
        menu.settle();
    }

    /// A on the list's row `row`: reads that topic. Whether there was one.
    pub fn choose(&mut self, row: usize) -> bool {
        if self.open.is_some() || row >= self.listed().count() {
            return false;
        }
        (self.open, self.list_row, self.fresh) = (Some(row), row, true);
        true
    }

    /// B: back from a topic to the list. Whether it was reading one (if not, B is the app's:
    /// it leaves the page).
    pub fn back(&mut self) -> bool {
        self.fresh = self.open.is_some();
        self.open.take().is_some()
    }
}

/// `text` ([`guide::lines`]) as rows to read: a line each, headings as headings, a blank line
/// as an empty row.
pub fn text_rows(text: &str, width: usize) -> Vec<Row> {
    guide::lines(text, width)
        .into_iter()
        .map(|l| match l {
            Line::Text(t) => Row::new(t, Value::None),
            Line::Heading(h) => Row::heading(h),
            Line::Blank => Row::new("", Value::None),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Effect, Key};

    fn help() -> Help {
        let t = |name: &str, text: &str| guide::parse(name, text).unwrap();
        let long: String = (0..30).map(|i| format!("Line {i}.\n\n")).collect();
        Help::from_topics(
            Some(t("whats-new", "nibOS 0.1.0-beta3\n- The Help page.")),
            vec![
                t("10-controls", "Controls\n# The D-pad\nIt moves."),
                t("20-long", &format!("A long one\n{long}")),
            ],
        )
    }

    #[test]
    fn the_list_then_a_topic_then_the_list() {
        let mut h = help();
        let mut m = Menu {
            tabs: vec![("System".into(), true)],
            ..Menu::default()
        };
        h.fill(&mut m);
        let labels: Vec<_> = m.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["What's new", "Controls", "A long one"]);
        assert_eq!(
            (m.title.as_str(), m.hint.as_str(), m.reading),
            ("Help", HINT, false)
        );
        assert_eq!(m.tabs.len(), 1, "the tabs are the app's");

        m.key(Key::Down);
        m.key(Key::Down);
        let Some(Effect::Choose(row)) = m.key(Key::A) else {
            panic!("A chooses")
        };
        assert!(h.choose(row));
        h.fill(&mut m);
        assert!(m.reading);
        assert_eq!(m.title, "A long one");
        assert_eq!(m.hint, READING_HINT);
        assert_eq!((m.row, m.scroll), (0, 0));
        m.key(Key::Right);
        h.fill(&mut m);
        assert_eq!(m.scroll, VISIBLE_LESS_ONE, "filling again keeps the place");
        assert!(!h.choose(0), "nothing to choose while reading");

        assert!(h.back());
        h.fill(&mut m);
        assert!(!m.reading);
        assert_eq!(
            (m.row, m.scroll),
            (2, 0),
            "back on the row it was opened from"
        );
        assert!(!h.back(), "B on the list is the app's");
    }

    const VISIBLE_LESS_ONE: usize = crate::menu::layout::VISIBLE - 1;

    #[test]
    fn a_topic_is_lines_and_headings() {
        let mut h = help();
        h.choose(1);
        let rows = h.rows();
        assert!(rows[0].heading && rows[0].label == "The D-pad");
        assert_eq!(rows[1].label, "It moves.");
        h.back();
        h.choose(0);
        assert_eq!(h.title(), "nibOS 0.1.0-beta3");
        assert_eq!(h.rows()[0].label, "\u{2022} The Help page.");
    }

    #[test]
    fn no_files_no_page() {
        let h = Help::from_topics(None, Vec::new());
        assert!(h.is_empty());
        assert!(h.rows().is_empty());
        assert!(!help().is_empty());
    }
}
