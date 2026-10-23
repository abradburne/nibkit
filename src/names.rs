//! A game's name for showing, from its file's name, as DSlot shows it.

/// The name to show for a game whose file is called `stem` (without its extension): No-Intro's
/// tags in brackets dropped ("(USA)", "(Rev 1)", "[!]"), spaces tidied, and an article No-Intro
/// puts last put back in front ("Legend of Zelda, The - A Link to the Past" is "The Legend of
/// Zelda - A Link to the Past"). A name that is nothing but brackets stays as it is.
pub fn display_name(stem: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in stem.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.is_empty() {
        return stem.to_owned();
    }
    let (head, tail) = out.split_once(" - ").unwrap_or((out.as_str(), ""));
    for article in ["The", "A", "An"] {
        if let Some(name) = head.strip_suffix(&format!(", {article}")) {
            let name = format!("{article} {name}");
            return if tail.is_empty() {
                name
            } else {
                format!("{name} - {tail}")
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_intro_names() {
        assert_eq!(
            display_name("Gran Turismo 2 (USA) (Arcade Mode) (Rev 1)"),
            "Gran Turismo 2"
        );
        assert_eq!(
            display_name("Legend of Zelda, The - A Link to the Past (USA)"),
            "The Legend of Zelda - A Link to the Past"
        );
        assert_eq!(
            display_name("Urbz, The (Europe) (En,Fr,De) [!]"),
            "The Urbz"
        );
        assert_eq!(display_name("Sims, An Odd Name"), "Sims, An Odd Name");
        assert_eq!(display_name("(Test Cart)"), "(Test Cart)");
        assert_eq!(display_name("  Tetris   DX  "), "Tetris DX");
    }
}
