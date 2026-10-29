//! Text cleanup for untrusted metadata, sort keys and matching keys.

/// Cleans a metadata string from an untrusted source: removes control
/// characters, collapses whitespace, trims, and truncates to `max_chars`.
/// Returns `None` if nothing is left.
pub fn clean(text: &str, max_chars: usize) -> Option<String> {
    let mut out = String::with_capacity(text.len().min(max_chars * 4));
    let mut count = 0;
    let mut pending_space = false;
    for c in text.chars() {
        if c.is_whitespace() || c.is_control() {
            pending_space = !out.is_empty();
            continue;
        }
        if count >= max_chars {
            break;
        }
        if pending_space {
            if count + 1 >= max_chars {
                break;
            }
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        out.push(c);
        count += 1;
    }
    (!out.is_empty()).then_some(out)
}

/// Like [`clean`] but keeps line breaks (for descriptions).
pub fn clean_multiline(text: &str, max_chars: usize) -> Option<String> {
    let lines: Vec<String> = text
        .lines()
        .map(|line| clean(line, max_chars).unwrap_or_default())
        .collect();
    let joined = lines.join("\n");
    let collapsed = collapse_blank_lines(joined.trim());
    let truncated: String = collapsed.chars().take(max_chars).collect();
    let truncated = truncated.trim_end().to_owned();
    (!truncated.is_empty()).then_some(truncated)
}

fn collapse_blank_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0;
    for line in text.split('\n') {
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// Sort key for titles and names: lowercase, leading English article removed.
pub fn sort_key(text: &str) -> String {
    let lower = text.trim().to_lowercase();
    for article in ["the ", "a ", "an "] {
        if let Some(rest) = lower.strip_prefix(article)
            && !rest.trim().is_empty()
        {
            return rest.trim_start().to_owned();
        }
    }
    lower
}

/// Matching key: lowercase alphanumerics separated by single spaces.
/// Used for grouping by album and for fingerprints.
pub fn match_key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        } else {
            pending_space = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_strips_controls_and_collapses_space() {
        assert_eq!(
            clean("  The\u{0}  Book\t\n Title ", 100).as_deref(),
            Some("The Book Title")
        );
        assert_eq!(clean(" \u{1}\n ", 100), None);
        assert_eq!(clean("", 100), None);
    }

    #[test]
    fn clean_truncates_on_char_boundaries() {
        assert_eq!(clean("本本本本", 2).as_deref(), Some("本本"));
        assert_eq!(clean("ab cd", 3).as_deref(), Some("ab"));
        let long = "x".repeat(10_000);
        assert_eq!(clean(&long, 512).map(|s| s.chars().count()), Some(512));
    }

    #[test]
    fn multiline_keeps_paragraphs() {
        assert_eq!(
            clean_multiline("First  line\r\n\n\n\nSecond\u{7}", 100).as_deref(),
            Some("First line\n\nSecond")
        );
    }

    #[test]
    fn sort_and_match_keys() {
        assert_eq!(sort_key("The Hobbit"), "hobbit");
        assert_eq!(sort_key("The"), "the");
        assert_eq!(sort_key("An Echo"), "echo");
        assert_eq!(match_key("The Hobbit: Part 1!"), "the hobbit part 1");
        assert_eq!(match_key("  Livre d’été 本 "), "livre d été 本");
    }
}
